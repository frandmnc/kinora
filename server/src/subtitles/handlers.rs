use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::services::ServeFile;
use tracing::{info, warn};

use super::extract::{self, SUBTITLE_EXTENSIONS};
use super::{SubtitleTrack, repo};
use crate::app::AppState;
use crate::contents;
use crate::error::{AppError, AppResult};

/// GET /api/subtitles/{id}
/// Entrega el .vtt. El navegador lo pide desde `<track src>`.
pub async fn serve(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    request: Request,
) -> AppResult<Response> {
    let (library_dir, rel_path) = repo::location(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el subtítulo {id}")))?;
    let path = state
        .settings
        .require_media_dir()?
        .join(library_dir)
        .join(rel_path);
    if !path.is_file() {
        return Err(AppError::NotFound(format!(
            "el subtítulo {id} no está en el disco"
        )));
    }

    let response = ServeFile::new(path).oneshot(request).await?;
    let mut response = response.map(Body::new);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/vtt; charset=utf-8"),
    );
    Ok(response)
}

#[derive(Deserialize)]
pub struct AddParams {
    /// Nombre del archivo subido: de ahí sale el formato (.srt, .ass, .ssa, .vtt).
    name: String,
    /// Código ISO 639 ("spa", "en"…). Sin idioma queda como "Subtítulos".
    language: Option<String>,
    #[serde(default)]
    forced: bool,
}

/// POST /api/admin/files/{id}/subtitles?name=Episodio.srt&language=spa&forced=false
/// El cuerpo es el archivo tal cual. Se convierte a WebVTT al lado del video.
pub async fn add(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    Query(params): Query<AddParams>,
    body: Bytes,
) -> AppResult<(StatusCode, Json<SubtitleTrack>)> {
    let extension = std::path::Path::new(&params.name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !SUBTITLE_EXTENSIONS.contains(&extension.as_str()) {
        return Err(AppError::BadRequest(format!(
            "{} no es un subtítulo (se aceptan SRT, ASS, SSA y VTT)",
            params.name
        )));
    }
    let language = params
        .language
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty() && l != "und");
    if let Some(l) = &language
        && !((2..=3).contains(&l.len()) && l.chars().all(|c| c.is_ascii_alphabetic()))
    {
        return Err(AppError::BadRequest(format!(
            "\"{l}\" no es un código de idioma"
        )));
    }
    if body.is_empty() {
        return Err(AppError::BadRequest(format!("{} está vacío", params.name)));
    }

    let (library_dir, video_rel) = contents::repo::file_location(&state.db, file_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el archivo {file_id}")))?;
    let root = state.settings.require_media_dir()?.join(library_dir);
    if !root.join(&video_rel).is_file() {
        return Err(AppError::NotFound(format!(
            "{video_rel} no está en el disco"
        )));
    }

    // Se guarda en un temporal porque ffmpeg lee de un archivo; `attach_file` lo consume.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = std::env::temp_dir().join(format!("kinora-subtitle-{file_id}-{nanos}.{extension}"));
    tokio::fs::write(&temp, &body).await?;
    let result = extract::attach_file(
        &state.db,
        &root,
        file_id,
        &video_rel,
        temp.clone(),
        language,
        params.forced,
    )
    .await;
    // Si falló a mitad de camino, que no quede el temporal.
    let _ = tokio::fs::remove_file(&temp).await;
    // El detalle de ffmpeg va a la terminal; en la pantalla, algo que se entienda.
    let track = result.map_err(|err| {
        warn!("could not add subtitle {}: {err:#}", params.name);
        AppError::BadRequest(format!(
            "No se pudo leer {}: no parece un archivo de subtítulos válido.",
            params.name
        ))
    })?;
    info!("subtitle added to {video_rel}: {}", track.label);
    Ok((StatusCode::CREATED, Json(track)))
}

/// DELETE /api/admin/subtitles/{id}
/// Quita el subtítulo de la base y borra sus archivos (el .vtt y el .ass original, si hay).
pub async fn remove(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<StatusCode> {
    let files = repo::delete(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el subtítulo {id}")))?;
    let root = state.settings.require_media_dir()?.join(&files.library_dir);
    for rel in std::iter::once(&files.rel_path).chain(files.original_rel_path.as_ref()) {
        match tokio::fs::remove_file(root.join(rel)).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => warn!("could not delete {rel}: {err}"),
        }
    }
    info!("subtitle removed: {}", files.rel_path);
    Ok(StatusCode::NO_CONTENT)
}
