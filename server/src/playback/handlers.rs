use std::collections::HashSet;

use axum::Json;
use axum::extract::State;

use super::{ContinueItem, DayTotal, PlaybackSummary, Session, local_day, repo};
use crate::app::AppState;
use crate::contents::repo::display_title;
use crate::error::AppResult;
use crate::profiles::Profile;

/// Menos de esto, no vale la pena ofrecer "seguir viendo": es como no haber empezado.
const MIN_POSITION: f64 = 10.0;
const MAX_ITEMS: usize = 12;

/// GET /api/continue-watching
/// Por cada película o serie, solo lo más reciente: lo que quedó a medias o, si se terminó
/// un episodio, el siguiente.
pub async fn continue_watching(
    State(state): State<AppState>,
    profile: Profile,
) -> AppResult<Json<Vec<ContinueItem>>> {
    let mut items = Vec::new();
    let mut seen_tops = HashSet::new();

    for recent in repo::recent_progress(&state.db, profile.id, 100).await? {
        // `insert` devuelve false si la película o serie ya tuvo su fila.
        if items.len() >= MAX_ITEMS || !seen_tops.insert(recent.top_id) {
            continue;
        }

        if !recent.watched {
            if recent.position >= MIN_POSITION {
                items.push(ContinueItem {
                    title: display_title(&state.db, recent.content_id)
                        .await?
                        .unwrap_or_else(|| recent.top_title.clone()),
                    content_id: recent.content_id,
                    top_id: recent.top_id,
                    top_title: recent.top_title,
                    position: recent.position,
                    duration: recent.duration,
                    next_up: false,
                });
            }
            continue;
        }

        // Terminado: si es un episodio, se ofrece el siguiente (si no está visto también).
        let (Some(season), Some(episode)) = (recent.season_number, recent.episode_number) else {
            continue;
        };
        if !recent.is_episode {
            continue;
        }
        if let Some(next) =
            repo::next_episode(&state.db, profile.id, recent.top_id, season, episode).await?
            && next.watched != Some(true)
        {
            items.push(ContinueItem {
                title: format!(
                    "{} · T{} E{}",
                    recent.top_title, next.season_number, next.episode_number
                ),
                content_id: next.id,
                top_id: recent.top_id,
                top_title: recent.top_title,
                position: next.position.unwrap_or(0.0),
                duration: next.duration,
                next_up: true,
            });
        }
    }

    Ok(Json(items))
}

/// GET /api/admin/metrics/playback
pub async fn summary(State(state): State<AppState>) -> AppResult<Json<PlaybackSummary>> {
    let today = local_day(0);
    let stored = repo::days_since(&state.db, &local_day(6)).await?;

    // Los 7 días completos, con cero en los que no se vio nada.
    let days: Vec<DayTotal> = (0..7)
        .rev()
        .map(|days_ago| {
            let day = local_day(days_ago);
            let seconds = stored
                .iter()
                .find(|d| d.day == day)
                .map_or(0.0, |d| d.seconds);
            DayTotal { day, seconds }
        })
        .collect();

    Ok(Json(PlaybackSummary {
        today_seconds: days
            .iter()
            .find(|d| d.day == today)
            .map_or(0.0, |d| d.seconds),
        days,
        top: repo::top_since(&state.db, &local_day(29), 5).await?,
        libraries: repo::libraries_since(&state.db, crate::db::now() - 30 * 86_400).await?,
    }))
}

/// GET /api/admin/metrics/sessions
/// El historial más reciente (las visualizaciones de menos de 30 segundos no se muestran).
pub async fn sessions(State(state): State<AppState>) -> AppResult<Json<Vec<Session>>> {
    Ok(Json(repo::recent_sessions(&state.db, 50, 30.0).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn sums_seconds_per_day_and_groups_episodes_by_show() {
        let db = connect_in_memory().await;
        sqlx::query(
            "INSERT INTO libraries (id, name, dir) VALUES (1, 'Anime', 'Anime'); \
             INSERT INTO contents (id, library_id, kind, parent_id, number, title, created_at) VALUES \
               (1, 1, 'show', NULL, NULL, 'Frieren', 0), \
               (2, 1, 'season', 1, 1, 'Temporada 1', 0), \
               (3, 1, 'episode', 2, 1, 'Episodio 1', 0), \
               (4, 1, 'episode', 2, 2, 'Episodio 2', 0), \
               (5, 1, 'movie', NULL, NULL, 'Interstellar', 0);",
        )
        .execute(&db)
        .await
        .unwrap();

        let today = local_day(0);
        repo::add_seconds(&db, &today, 3, 600.0).await.unwrap();
        repo::add_seconds(&db, &today, 3, 30.0).await.unwrap();
        repo::add_seconds(&db, &today, 4, 300.0).await.unwrap();
        repo::add_seconds(&db, &today, 5, 100.0).await.unwrap();

        let days = repo::days_since(&db, &today).await.unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].seconds, 1030.0);

        let top = repo::top_since(&db, &today, 5).await.unwrap();
        assert_eq!(top[0].title, "Frieren");
        assert_eq!(top[0].seconds, 930.0);
        assert_eq!(top[1].title, "Interstellar");
    }
}
