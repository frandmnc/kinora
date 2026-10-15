use std::collections::HashSet;

use axum::Json;
use axum::extract::{Query, State};
use chrono::{Datelike, Days, Local, Months, NaiveDate, NaiveTime};
use serde::Deserialize;

use super::{
    ContinueItem, DayTotal, MIN_SESSION, PlaybackSummary, Session, ViewingRange, ViewingStats,
    local_day, repo,
};
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
                    top_poster_at: recent.top_poster_at,
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
                top_poster_at: recent.top_poster_at,
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
    }))
}

#[derive(Deserialize)]
pub struct ViewingParams {
    range: ViewingRange,
}

/// GET /api/admin/metrics/viewing?range=week|month|year|all
/// Qué se miró y cuánto en los últimos 7 días, 30 días, 12 meses o desde siempre.
pub async fn viewing(
    State(state): State<AppState>,
    Query(params): Query<ViewingParams>,
) -> AppResult<Json<ViewingStats>> {
    let today = Local::now().date_naive();
    let first = repo::first_day(&state.db)
        .await?
        .and_then(|day| NaiveDate::parse_from_str(&day, DAY).ok());
    let (from, monthly) = period(params.range, today, first);
    let from_day = from.format(DAY).to_string();
    // Las sesiones guardan segundos desde 1970: el período empieza a la medianoche local.
    let since = from
        .and_time(NaiveTime::MIN)
        .and_local_timezone(Local)
        .earliest()
        .map_or(0, |t| t.timestamp());

    let stored = if monthly {
        repo::months_since(&state.db, &from_day).await?
    } else {
        repo::days_since(&state.db, &from_day).await?
    };
    let buckets: Vec<DayTotal> = bucket_keys(from, today, monthly)
        .into_iter()
        .map(|day| {
            let seconds = stored
                .iter()
                .find(|d| d.day == day)
                .map_or(0.0, |d| d.seconds);
            DayTotal { day, seconds }
        })
        .collect();
    let (sessions, completed) = repo::session_counts(&state.db, since, MIN_SESSION).await?;

    Ok(Json(ViewingStats {
        from: from_day.clone(),
        total_seconds: buckets.iter().map(|b| b.seconds).sum(),
        sessions,
        completed,
        monthly,
        buckets,
        top: repo::top_since(&state.db, &from_day, 10).await?,
        libraries: repo::libraries_since(&state.db, since).await?,
        profiles: repo::profiles_since(&state.db, since).await?,
    }))
}

const DAY: &str = "%Y-%m-%d";

/// Desde qué día va el período y si conviene contarlo por mes (más de un mes) o por día.
fn period(range: ViewingRange, today: NaiveDate, first: Option<NaiveDate>) -> (NaiveDate, bool) {
    let month_start = |day: NaiveDate| day.with_day(1).unwrap_or(day);
    match range {
        ViewingRange::Week => (today - Days::new(6), false),
        ViewingRange::Month => (today - Days::new(29), false),
        ViewingRange::Year => (month_start(today) - Months::new(11), true),
        ViewingRange::All => {
            let from = first.unwrap_or(today).min(today);
            (from, (today - from).num_days() >= 31)
        }
    }
}

/// Todos los días ("AAAA-MM-DD") o meses ("AAAA-MM") de `from` a `to`, en orden.
fn bucket_keys(from: NaiveDate, to: NaiveDate, monthly: bool) -> Vec<String> {
    let mut keys = Vec::new();
    let mut day = from;
    while day <= to {
        if monthly {
            keys.push(day.format("%Y-%m").to_string());
            day = day.with_day(1).unwrap_or(day) + Months::new(1);
        } else {
            keys.push(day.format(DAY).to_string());
            day = day + Days::new(1);
        }
    }
    keys
}

/// GET /api/admin/metrics/sessions
/// El historial más reciente (las visualizaciones de menos de 30 segundos no se muestran).
pub async fn sessions(State(state): State<AppState>) -> AppResult<Json<Vec<Session>>> {
    Ok(Json(
        repo::recent_sessions(&state.db, 50, MIN_SESSION).await?,
    ))
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

    #[tokio::test]
    async fn adds_up_each_profiles_viewings_of_a_content() {
        let db = connect_in_memory().await;
        sqlx::query(
            "INSERT INTO libraries (id, name, dir) VALUES (1, 'Películas', 'Películas'); \
             INSERT INTO contents (id, library_id, kind, title, created_at) VALUES \
               (1, 1, 'movie', 'Interstellar', 0); \
             INSERT INTO profiles (id, name, created_at) VALUES (2, 'Otra', 0); \
             INSERT INTO playback_sessions (content_id, title, profile_id, client, started_at, \
               ended_at, seconds_watched, start_position, end_position, duration) VALUES \
               (1, 'Interstellar', 1, 'tv', 100, 1100, 1000, 0, 1000, 1000), \
               (1, 'Interstellar', 1, 'tv', 2000, 2500, 500, 0, 500, 1000), \
               (1, 'Interstellar', 1, 'tv', 3000, 3010, 10, 500, 510, NULL), \
               (1, 'Interstellar', 2, 'tv', 4000, 5000, 1000, 0, 1000, 1000);",
        )
        .execute(&db)
        .await
        .unwrap();

        let stats = repo::watch_stats(&db, 1, &[1, 2]).await.unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].seconds, 1510.0);
        // Una vez y media; la de 10 segundos no tiene duración y no suma veces.
        assert_eq!(stats[0].times, Some(1.5));
        // La última "de verdad": la de 10 segundos no cuenta.
        assert_eq!(stats[0].last_watched_at, Some(2500));
    }

    #[test]
    fn periods_count_by_day_up_to_a_month_and_by_month_beyond() {
        let date = |s: &str| NaiveDate::parse_from_str(s, DAY).unwrap();
        let today = date("2026-10-14");

        let (from, monthly) = period(ViewingRange::Month, today, None);
        assert_eq!((from, monthly), (date("2026-09-15"), false));
        assert_eq!(bucket_keys(from, today, monthly).len(), 30);

        let (from, monthly) = period(ViewingRange::Year, today, None);
        assert_eq!((from, monthly), (date("2025-11-01"), true));
        let months = bucket_keys(from, today, monthly);
        assert_eq!(months.len(), 12);
        assert_eq!(months.first().unwrap(), "2025-11");
        assert_eq!(months.last().unwrap(), "2026-10");

        // "Todo" empieza el primer día con algo visto; si es reciente, va por día.
        assert_eq!(
            period(ViewingRange::All, today, Some(date("2026-10-01"))),
            (date("2026-10-01"), false)
        );
        assert_eq!(
            period(ViewingRange::All, today, Some(date("2026-03-20"))),
            (date("2026-03-20"), true)
        );
        assert_eq!(period(ViewingRange::All, today, None), (today, false));
        let months = bucket_keys(date("2026-03-20"), today, true);
        assert_eq!(months.len(), 8);
        assert_eq!(months.first().unwrap(), "2026-03");
    }
}
