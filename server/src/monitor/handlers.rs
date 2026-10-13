use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;

use super::ServerStatus;
use super::history::{self, HistoryRange, ServerHistory};
use crate::app::AppState;
use crate::db::now;
use crate::error::AppResult;

/// GET /api/admin/status
pub async fn status(State(state): State<AppState>) -> Json<ServerStatus> {
    Json(state.monitor.status())
}

#[derive(Deserialize)]
pub struct HistoryParams {
    range: HistoryRange,
}

/// GET /api/admin/metrics/server?range=day|week|month
/// CPU, memoria y envío del último día, semana o mes, en unos 300 puntos.
pub async fn history(
    State(state): State<AppState>,
    Query(params): Query<HistoryParams>,
) -> AppResult<Json<ServerHistory>> {
    Ok(Json(history::load(&state.db, params.range, now()).await?))
}
