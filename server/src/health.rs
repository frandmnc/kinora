use axum::Json;
use axum::extract::State;
use serde::Serialize;
use ts_rs::TS;

use crate::app::AppState;

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Health {
    status: String,
    version: String,
}

/// GET /api/health
pub async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Datos del servidor para el Admin.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ServerInfo {
    version: String,
    media_dir: String,
    media_available: bool,
    import_sources: Vec<String>,
}

/// GET /api/admin/info
pub async fn admin_info(State(state): State<AppState>) -> Json<ServerInfo> {
    let config = &state.config;
    Json(ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        media_dir: config.media_dir.to_string_lossy().into_owned(),
        media_available: config.media_dir.is_dir(),
        import_sources: config
            .import_sources
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
    })
}
