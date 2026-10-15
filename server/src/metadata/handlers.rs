use axum::Json;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::services::ServeFile;
use tracing::info;

use super::{
    ApplyMetadata, BACKDROP_FILE, ContentChanges, MetadataCandidate, POSTER_FILE, TMDB_KEY,
    TmdbKey, TmdbSetting, content_folder, exists, repo, still_file, tmdb, tmdb_kind,
};
use crate::app::AppState;
use crate::contents::{self, Content};
use crate::error::{AppError, AppResult};
use crate::manifest;
use crate::settings;

/// GET /api/admin/settings/tmdb
pub async fn tmdb_setting(State(state): State<AppState>) -> AppResult<Json<TmdbSetting>> {
    let configured = settings::repo::get(&state.db, TMDB_KEY).await?.is_some();
    Ok(Json(TmdbSetting { configured }))
}

/// PUT /api/admin/settings/tmdb
/// Guarda la clave después de comprobar que TMDB la acepta.
pub async fn set_tmdb_key(
    State(state): State<AppState>,
    Json(body): Json<TmdbKey>,
) -> AppResult<Json<TmdbSetting>> {
    let key = body.key.trim();
    if key.is_empty() {
        return Err(AppError::BadRequest("falta la clave".to_string()));
    }
    tmdb::Tmdb::new(key).check().await?;
    settings::repo::set(&state.db, TMDB_KEY, key).await?;
    info!("TMDB key saved");
    Ok(Json(TmdbSetting { configured: true }))
}

/// DELETE /api/admin/settings/tmdb
pub async fn remove_tmdb_key(State(state): State<AppState>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM settings WHERE key = ?")
        .bind(TMDB_KEY)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SearchParams {
    q: String,
    year: Option<i64>,
}

/// GET /api/admin/contents/{id}/metadata/search?q=frieren&year=2023
/// Busca en TMDB películas (o series, según el contenido) con ese título.
pub async fn search(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<SearchParams>,
) -> AppResult<Json<Vec<MetadataCandidate>>> {
    let content = find(&state, id).await?;
    let kind = tmdb_kind(&content)?;
    let query = params.q.trim();
    if query.is_empty() {
        return Err(AppError::BadRequest("escribí qué buscar".to_string()));
    }
    let tmdb = super::client(&state.db).await?;
    let mut results = tmdb.search(kind, query, params.year).await?;
    // Con el año puede no aparecer (TMDB a veces tiene otra fecha): se prueba sin él.
    if results.is_empty() && params.year.is_some() {
        results = tmdb.search(kind, query, None).await?;
    }
    let candidates = results
        .into_iter()
        .map(|r| {
            let title = r.preferred_title().to_string();
            // Debajo va el otro título (el traducido, o el original en su alfabeto).
            let other = if title == r.title {
                r.original_title.clone()
            } else {
                Some(r.title.clone())
            };
            MetadataCandidate {
                tmdb_id: r.id,
                year: tmdb::year_of(&r.release_date),
                original_title: other.filter(|other| *other != title),
                title,
                overview: r.overview.filter(|o| !o.trim().is_empty()),
                thumbnail: r.poster_path.as_deref().map(tmdb::thumbnail_url),
            }
        })
        .collect();
    Ok(Json(candidates))
}

/// POST /api/admin/contents/{id}/metadata
/// Usa la película o serie de TMDB elegida (o vuelve a pedir la que ya tenía).
pub async fn apply(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<ApplyMetadata>,
) -> AppResult<StatusCode> {
    let content = find(&state, id).await?;
    super::apply(&state.db, &state.settings, &content, body.tmdb_id).await?;
    info!(
        "metadata of \"{}\" taken from TMDB {}",
        content.title, body.tmdb_id
    );
    Ok(StatusCode::NO_CONTENT)
}

/// PATCH /api/admin/contents/{id}
/// Corrige a mano el título, el año o la sinopsis.
pub async fn edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(changes): Json<ContentChanges>,
) -> AppResult<StatusCode> {
    find(&state, id).await?;
    let title = changes.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest(
            "el título no puede quedar vacío".to_string(),
        ));
    }
    if let Some(year) = changes.year
        && !(1870..=2200).contains(&year)
    {
        return Err(AppError::BadRequest(format!("{year} no parece un año")));
    }
    let overview = changes
        .overview
        .as_deref()
        .map(str::trim)
        .filter(|o| !o.is_empty());
    repo::edit(&state.db, id, title, changes.year, overview).await?;

    if let (Some(media_dir), Some(top)) = (
        state.settings.media_dir(),
        repo::top_of(&state.db, id).await?,
    ) {
        manifest::refresh(&state.db, &media_dir, top).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/contents/{id}/poster
pub async fn poster(
    state: State<AppState>,
    id: Path<i64>,
    request: Request,
) -> AppResult<Response> {
    image(state, id, request, POSTER_FILE).await
}

/// GET /api/contents/{id}/backdrop
pub async fn backdrop(
    state: State<AppState>,
    id: Path<i64>,
    request: Request,
) -> AppResult<Response> {
    image(state, id, request, BACKDROP_FILE).await
}

/// GET /api/contents/{id}/still
/// El cuadro de un episodio, que vive en la carpeta de su serie.
pub async fn still(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    request: Request,
) -> AppResult<Response> {
    let place = repo::episode_place(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("el contenido {id} no es un episodio")))?;
    let show = find(&state, place.show_id).await?;
    let folder = content_folder(&state.db, &state.settings, &show).await?;
    serve(
        folder.join(still_file(place.season, place.episode)),
        request,
    )
    .await
}

/// Una imagen de la carpeta del contenido. La URL lleva `?v=` con la fecha en que se guardó:
/// si cambia la imagen cambia la URL, así que el navegador puede guardarla sin preguntar.
async fn image(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    request: Request,
    file: &str,
) -> AppResult<Response> {
    let content = find(&state, id).await?;
    let folder = content_folder(&state.db, &state.settings, &content).await?;
    if !exists(&folder, file) {
        return Err(AppError::NotFound(format!(
            "\"{}\" no tiene {file}",
            content.title
        )));
    }
    serve(folder.join(file), request).await
}

/// Entrega una imagen. La URL lleva `?v=`: si la imagen cambia, cambia la URL, así que el
/// navegador puede guardarla sin volver a preguntar.
async fn serve(path: std::path::PathBuf, request: Request) -> AppResult<Response> {
    if !path.is_file() {
        return Err(AppError::NotFound(format!(
            "no está la imagen {}",
            path.display()
        )));
    }
    let mut response = ServeFile::new(path)
        .oneshot(request)
        .await?
        .map(axum::body::Body::new);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    Ok(response)
}

async fn find(state: &AppState, id: i64) -> AppResult<Content> {
    contents::repo::find(&mut *state.db.acquire().await?, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el contenido {id}")))
}
