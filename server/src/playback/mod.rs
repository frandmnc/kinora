//! Reproducción: "seguir viendo" para el Player y minutos vistos para el Admin.

mod handlers;
pub mod repo;

use axum::Router;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::AppState;

/// Una fila de "seguir viendo": algo empezado, o el episodio que sigue a uno terminado.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ContinueItem {
    /// La película o el episodio a reproducir.
    pub content_id: i64,
    /// La película o la serie, para la carátula y el detalle.
    pub top_id: i64,
    pub top_title: String,
    /// Cuándo se guardó la carátula de la película o serie; `null` si no tiene.
    pub top_poster_at: Option<i64>,
    /// "Sousou no Frieren · T1 E2" o el título de la película.
    pub title: String,
    pub position: f64,
    pub duration: Option<f64>,
    /// `true` si es el episodio siguiente a uno terminado (todavía no empezado).
    pub next_up: bool,
}

/// Lo visto hoy y en la semana, para la página Estado.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct PlaybackSummary {
    pub today_seconds: f64,
    /// Los últimos 7 días, del más viejo al de hoy.
    pub days: Vec<DayTotal>,
}

/// Qué período mostrar en las métricas de visualización.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ViewingRange {
    Week,
    Month,
    Year,
    All,
}

/// Qué se miró y cuánto en un período.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ViewingStats {
    /// Primer día del período (AAAA-MM-DD).
    pub from: String,
    pub total_seconds: f64,
    /// Visualizaciones de al menos 30 segundos, y cuántas llegaron al final.
    pub sessions: i64,
    pub completed: i64,
    /// `true` si `buckets` va por mes ("AAAA-MM") en lugar de por día ("AAAA-MM-DD").
    pub monthly: bool,
    /// Tiempo visto por día o por mes, del más viejo al actual, con cero en los vacíos.
    pub buckets: Vec<DayTotal>,
    /// Lo más visto, agrupado por película o serie.
    pub top: Vec<TitleTotal>,
    pub libraries: Vec<TitleTotal>,
    pub profiles: Vec<TitleTotal>,
}

/// Una visualización del historial: quién, qué, cuándo y cuánto.
#[derive(Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct Session {
    pub id: i64,
    /// `null` si el contenido ya no existe (el título se conserva igual).
    pub content_id: Option<i64>,
    pub title: String,
    pub library_name: Option<String>,
    /// Quién miró (el nombre que tenía el perfil en ese momento).
    pub profile_name: Option<String>,
    pub client: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub seconds_watched: f64,
    pub start_position: f64,
    pub end_position: f64,
    pub duration: Option<f64>,
    pub completed: bool,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct DayTotal {
    /// AAAA-MM-DD.
    pub day: String,
    pub seconds: f64,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct TitleTotal {
    pub title: String,
    pub seconds: f64,
}

/// Rutas del Player. Se montan bajo /api.
pub fn routes() -> Router<AppState> {
    Router::new().route("/continue-watching", get(handlers::continue_watching))
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/metrics/playback", get(handlers::summary))
        .route("/metrics/viewing", get(handlers::viewing))
        .route("/metrics/sessions", get(handlers::sessions))
}

/// Fecha local de hoy (o de hace `days_ago` días) en formato AAAA-MM-DD.
pub fn local_day(days_ago: i64) -> String {
    (chrono::Local::now() - chrono::Duration::days(days_ago))
        .format("%Y-%m-%d")
        .to_string()
}
