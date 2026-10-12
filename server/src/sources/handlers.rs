use axum::Json;
use axum::extract::State;

use super::{Source, list_videos};
use crate::app::AppState;
use crate::error::AppResult;

/// GET /api/admin/sources
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Source>>> {
    let roots = state.config.import_sources.clone();

    // Recorrer carpetas es trabajo bloqueante del disco: se hace en un hilo aparte
    // para no frenar al resto del servidor.
    let sources = tokio::task::spawn_blocking(move || {
        roots
            .iter()
            .map(|root| {
                let available = root.is_dir();
                Source {
                    path: root.to_string_lossy().into_owned(),
                    available,
                    files: if available {
                        list_videos(root)
                    } else {
                        Vec::new()
                    },
                }
            })
            .collect()
    })
    .await?;

    Ok(Json(sources))
}
