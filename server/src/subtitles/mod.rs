//! Subtítulos: al importar se extraen los de texto que vienen dentro del video, y después se
//! pueden agregar archivos (.srt, .ass, .vtt) a cada video desde Bibliotecas. Todos se
//! convierten a WebVTT (lo que entienden los navegadores y la TV) y se guardan al lado del
//! video. Los ASS conservan además su original, para dibujar sus estilos más adelante.

mod extract;
mod handlers;
pub mod repo;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};
use serde::Serialize;
use ts_rs::TS;

use crate::app::AppState;

pub use extract::{attach_subtitles, describe, find_tracks};

/// Una pista de subtítulos lista para el Player.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct SubtitleTrack {
    pub id: i64,
    pub media_file_id: i64,
    pub language: Option<String>,
    /// Código de 2 letras para `<track srclang>`.
    pub srclang: Option<String>,
    pub label: String,
    pub is_default: bool,
    pub forced: bool,
}

/// Rutas del Player. Se montan bajo /api.
pub fn routes() -> Router<AppState> {
    Router::new().route("/subtitles/{id}", get(handlers::serve))
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/files/{id}/subtitles",
            // Un subtítulo pesa unos cientos de KB; 20 MB alcanza y sobra.
            post(handlers::add).layer(DefaultBodyLimit::max(20 * 1024 * 1024)),
        )
        .route("/subtitles/{id}", delete(handlers::remove))
}
