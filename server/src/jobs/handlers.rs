use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use super::worker::give_back;
use super::{JobState, JobView, repo};
use crate::app::AppState;
use crate::contents;
use crate::error::{AppError, AppResult};

/// GET /api/admin/jobs
/// Lo pendiente y lo que corre, y los últimos 20 terminados.
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<JobView>>> {
    let mut views = Vec::new();
    for job in repo::recent(&state.db, 20).await? {
        let title = contents::repo::display_title(&state.db, job.content_id)
            .await?
            .unwrap_or_else(|| job.source_name.clone());
        views.push(JobView { job, title });
    }
    Ok(Json(views))
}

/// POST /api/admin/jobs/{id}/cancel
/// Si todavía no empezó, se saca de la fila; si está corriendo, se corta ffmpeg.
/// En los dos casos el original vuelve a la bandeja de entrada.
pub async fn cancel(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<StatusCode> {
    let job = repo::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el trabajo {id}")))?;
    match job.state {
        JobState::Pending if repo::cancel_pending(&state.db, id).await? => {
            give_back(&state, &job).await;
            Ok(StatusCode::NO_CONTENT)
        }
        // El trabajador se encarga de devolver el archivo y anotar la cancelación.
        JobState::Pending | JobState::Running if state.jobs.cancel_running(id) => {
            Ok(StatusCode::ACCEPTED)
        }
        _ => Err(AppError::Conflict(format!("el trabajo {id} ya terminó"))),
    }
}
