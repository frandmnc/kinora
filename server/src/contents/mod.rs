//! Catálogo: películas, series, temporadas y episodios, y los archivos que los reproducen.

mod handlers;
pub mod repo;

use axum::Router;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::AppState;

/// Se guarda como texto en la columna `kind`: "movie", "show", "season", "episode".
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, sqlx::Type, TS)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
#[ts(export)]
pub enum ContentKind {
    Movie,
    Show,
    Season,
    Episode,
}

#[derive(Debug, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct Content {
    pub id: i64,
    pub library_id: i64,
    pub kind: ContentKind,
    pub parent_id: Option<i64>,
    pub number: Option<i64>,
    pub title: String,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub dir: Option<String>,
    pub created_at: i64,
}

/// Un archivo reproducible. No incluye `probe_json` porque es grande.
#[derive(Debug, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct MediaFile {
    pub id: i64,
    pub content_id: i64,
    pub rel_path: String,
    pub size: i64,
    pub duration: Option<f64>,
    pub video_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub source_path: Option<String>,
    pub imported_at: i64,
}

/// Un contenido con sus archivos y sus hijos (una serie con temporadas, y estas con episodios).
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ContentDetail {
    pub content: Content,
    pub files: Vec<MediaFile>,
    pub children: Vec<ContentDetail>,
}

/// Rutas del Player. Se montan bajo /api.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/contents", get(handlers::list))
        .route("/contents/{id}", get(handlers::detail))
}
