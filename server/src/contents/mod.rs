//! Catálogo: películas, series, temporadas y episodios, y los archivos que los reproducen.

mod handlers;
pub mod repo;

use axum::Router;
use axum::routing::{delete, get, put};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::AppState;
use crate::playback::WatchStats;
use crate::probe::TechInfo;
use crate::subtitles::SubtitleTrack;

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
    /// De qué película o serie de TMDB salió la metadata.
    pub tmdb_id: Option<i64>,
    /// Cuándo se trajo de TMDB (segundos desde 1970).
    pub metadata_at: Option<i64>,
    /// Cuándo se guardó la carátula (`/api/contents/{id}/poster`); `null` si no tiene.
    pub poster_at: Option<i64>,
    /// Cuándo se guardó la imagen de fondo (`/api/contents/{id}/backdrop`); `null` si no tiene.
    pub backdrop_at: Option<i64>,
    /// Solo episodios: cuándo se guardó un cuadro del episodio (`/api/contents/{id}/still`).
    pub still_at: Option<i64>,
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
    /// Lo técnico (resolución, códecs, HDR, audio), sacado del análisis de ffprobe.
    /// Solo viene en el detalle de un contenido.
    #[sqlx(skip)]
    pub tech: Option<TechInfo>,
}

/// Un contenido con sus archivos y sus hijos (una serie con temporadas, y estas con episodios).
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ContentDetail {
    pub content: Content,
    pub files: Vec<MediaFile>,
    /// Solo en películas y episodios que ya se empezaron a ver.
    pub progress: Option<Progress>,
    /// Cuánto se vio en total y cuándo fue la última vez: solo en películas y episodios que el
    /// perfil ya vio alguna vez.
    pub watch: Option<WatchStats>,
    /// Los subtítulos de sus archivos.
    pub subtitles: Vec<SubtitleTrack>,
    pub children: Vec<ContentDetail>,
}

#[derive(Debug, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct Progress {
    pub content_id: i64,
    pub position: f64,
    pub duration: Option<f64>,
    pub watched: bool,
    pub updated_at: i64,
}

/// Una película o serie con algo agregado hace poco, para "Agregados recientemente".
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RecentItem {
    pub content: Content,
    /// Cuándo se agregó lo último (segundos desde 1970).
    pub added_at: i64,
    /// Qué se agregó: "T1 E5", "T2 · 12 episodios"; vacío en una película.
    pub note: String,
}

/// Lo que informa el Player cada 10 segundos y en cada cambio.
#[derive(Debug, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PlaybackReport {
    pub position: f64,
    pub duration: Option<f64>,
    pub state: PlaybackState,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PlaybackState {
    Playing,
    Paused,
    /// El Player se cerró: la persona dejó de mirar.
    Stopped,
}

/// Rutas del Player. Se montan bajo /api.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/contents", get(handlers::list))
        .route("/contents/{id}", get(handlers::detail))
        .route("/recent", get(handlers::recent))
        .route("/contents/{id}/progress", put(handlers::report_progress))
        .route("/files/{id}/stream", get(handlers::stream))
}

/// Una película o serie con sus números, para la pantalla de una biblioteca en el Admin.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ContentSummary {
    pub content: Content,
    pub seasons: i64,
    pub episodes: i64,
    /// Episodios que existen pero todavía no tienen archivo.
    pub missing: i64,
    pub files: i64,
    /// Bytes de todos sus archivos.
    pub size: i64,
    pub subtitles: i64,
    /// Trabajos de la cola pendientes o en curso: mientras haya, no se puede eliminar.
    pub active_jobs: i64,
    /// Lo que dura todo (en una serie, la suma de los episodios; con varias versiones, una).
    pub runtime: f64,
    /// Lo técnico de sus videos, sin repetir: si todos son 1080p, un solo "1080p".
    pub resolutions: Vec<String>,
    /// "HEVC 10 bits", "H.264 8 bits".
    pub video: Vec<String>,
    /// "SDR", "HDR10"…
    pub dynamic_range: Vec<String>,
    /// "AAC 2.0", "AC3 5.1".
    pub audio: Vec<String>,
    /// Idiomas del audio, códigos ISO 639 ("jpn", "spa").
    pub audio_languages: Vec<String>,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/files/{id}", delete(handlers::delete_file))
        .route("/libraries/{id}/contents", get(handlers::library_contents))
        .route("/contents/{id}", delete(handlers::delete_content))
}
