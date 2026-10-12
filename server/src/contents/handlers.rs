use std::collections::HashMap;
use std::net::SocketAddr;

use axum::Json;
use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::services::ServeFile;
use tracing::info;

use super::{Content, ContentDetail, ContentKind, MediaFile, PlaybackReport, Progress, repo};
use crate::app::AppState;
use crate::error::{AppError, AppResult};

#[derive(Deserialize)]
pub struct ListParams {
    library_id: Option<i64>,
    kind: Option<ContentKind>,
}

/// GET /api/contents?library_id=1&kind=show
/// Películas y series (los contenidos sin padre), por título.
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> AppResult<Json<Vec<Content>>> {
    let contents = repo::list_top_level(&state.db, params.library_id, params.kind).await?;
    Ok(Json(contents))
}

/// GET /api/contents/{id}
/// El contenido con todos sus descendientes (temporadas, episodios), sus archivos y su progreso.
pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<ContentDetail>> {
    let contents = repo::with_descendants(&state.db, id).await?;
    let ids: Vec<i64> = contents.iter().map(|c| c.id).collect();

    // Agrupar hijos por padre, archivos y progreso por contenido, y armar el árbol desde la raíz.
    let mut parts = TreeParts::default();
    for file in repo::files_of(&state.db, &ids).await? {
        parts.files.entry(file.content_id).or_default().push(file);
    }
    for progress in repo::progress_of(&state.db, &ids).await? {
        parts.progress.insert(progress.content_id, progress);
    }
    let mut root = None;
    for content in contents {
        match content.parent_id {
            Some(parent) if content.id != id => {
                parts.children.entry(parent).or_default().push(content)
            }
            _ => root = Some(content),
        }
    }

    let root = root.ok_or_else(|| AppError::NotFound(format!("no existe el contenido {id}")))?;
    Ok(Json(build_tree(root, &mut parts)))
}

/// Las piezas del árbol, agrupadas por id de contenido.
#[derive(Default)]
struct TreeParts {
    children: HashMap<i64, Vec<Content>>,
    files: HashMap<i64, Vec<MediaFile>>,
    progress: HashMap<i64, Progress>,
}

/// Arma el árbol de forma recursiva. Cada pieza se mueve (no se copia) a su lugar.
fn build_tree(content: Content, parts: &mut TreeParts) -> ContentDetail {
    let mut kids = parts.children.remove(&content.id).unwrap_or_default();
    kids.sort_by_key(|c| c.number);
    ContentDetail {
        files: parts.files.remove(&content.id).unwrap_or_default(),
        progress: parts.progress.remove(&content.id),
        children: kids
            .into_iter()
            .map(|child| build_tree(child, parts))
            .collect(),
        content,
    }
}

/// PUT /api/contents/{id}/progress
/// El Player informa dónde está y si reproduce. Se guarda el progreso y se actualiza
/// la lista "viendo ahora" del monitor.
pub async fn report_progress(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    Json(report): Json<PlaybackReport>,
) -> AppResult<StatusCode> {
    let title = repo::display_title(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el contenido {id}")))?;

    let position = report.position.max(0.0);
    let watched = report
        .duration
        .is_some_and(|duration| duration > 0.0 && position >= duration * WATCHED_AT);
    repo::save_progress(&state.db, id, position, report.duration, watched).await?;

    state.monitor.report_viewer(
        client.ip().to_string(),
        id,
        title,
        position,
        report.duration,
        report.state,
    );
    Ok(StatusCode::NO_CONTENT)
}

/// A partir del 90 % de la duración, el contenido se considera visto.
const WATCHED_AT: f64 = 0.9;

/// GET /api/files/{id}/stream
/// Entrega el archivo de video. `ServeFile` responde los pedidos parciales (HTTP Range):
/// el navegador pide solo el tramo que necesita, y así puede adelantar y retroceder.
pub async fn stream(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    request: Request,
) -> AppResult<Response> {
    let (library_dir, rel_path) = repo::file_location(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el archivo {id}")))?;

    let path = state
        .settings
        .require_media_dir()?
        .join(library_dir)
        .join(rel_path);
    if !path.is_file() {
        return Err(AppError::NotFound(format!(
            "el archivo {id} no está en el disco ({})",
            path.display()
        )));
    }

    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("whole file")
        .to_string();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    let response = ServeFile::new(&path).oneshot(request).await?;
    info!(
        "▶ {} requested {file_name} ({range}) → {}",
        client.ip(),
        response.status()
    );

    // Mientras el navegador descarga, se cuentan los bytes enviados. Cuando la descarga
    // termina o se corta, el cuerpo se descarta, y con él el `guard`: el stream deja de figurar.
    let guard = state
        .monitor
        .start_transfer(client.ip().to_string(), id, file_name);
    let (parts, body) = response.into_parts();
    let body = body.map_frame(move |frame| {
        if let Some(data) = frame.data_ref() {
            guard.add_bytes(data.len() as u64);
        }
        frame
    });
    Ok(Response::from_parts(parts, Body::new(body)))
}
