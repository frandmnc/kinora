use axum::Json;
use axum::extract::State;

use super::ServerStatus;
use crate::app::AppState;

/// GET /api/admin/status
pub async fn status(State(state): State<AppState>) -> Json<ServerStatus> {
    Json(state.monitor.status())
}
