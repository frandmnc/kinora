use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use super::worker::give_back;
use super::{ConversionSummary, JobState, JobView, QueueView, repo};
use crate::app::AppState;
use crate::contents;
use crate::error::{AppError, AppResult};

/// GET /api/admin/jobs
/// Si la cola está en marcha, lo pendiente y lo que corre, y los últimos 20 terminados.
pub async fn list(State(state): State<AppState>) -> AppResult<Json<QueueView>> {
    let mut jobs = Vec::new();
    for job in repo::recent(&state.db, 20).await? {
        let title = contents::repo::display_title(&state.db, job.content_id)
            .await?
            .unwrap_or_else(|| job.source_name.clone());
        jobs.push(JobView { job, title });
    }
    Ok(Json(QueueView {
        started: state.jobs.is_started(),
        jobs,
    }))
}

/// POST /api/admin/jobs/start
/// Pone la cola en marcha. Se detiene sola cuando no queda nada pendiente.
pub async fn start(State(state): State<AppState>) -> AppResult<StatusCode> {
    state.jobs.set_started(&state.db, true).await?;
    tracing::info!("queue started");
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/admin/jobs/pause
/// Detiene la cola después del trabajo en curso (ese se termina).
pub async fn pause(State(state): State<AppState>) -> AppResult<StatusCode> {
    state.jobs.set_started(&state.db, false).await?;
    tracing::info!("queue paused");
    Ok(StatusCode::NO_CONTENT)
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

/// GET /api/admin/metrics/conversion
pub async fn summary(State(state): State<AppState>) -> AppResult<Json<ConversionSummary>> {
    let mut summary = repo::summary(&state.db).await?;
    summary.started = state.jobs.is_started();
    Ok(Json(summary))
}
