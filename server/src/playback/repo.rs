//! Consultas de reproducción: tiempo visto por día y lo que quedó a medias.

use sqlx::SqlitePool;

use crate::profiles::Profile;

use super::{DayTotal, TitleTotal};

/// Suma segundos al día y contenido indicados.
pub async fn add_seconds(
    db: &SqlitePool,
    day: &str,
    content_id: i64,
    seconds: f64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO playback_daily (day, content_id, seconds) VALUES (?, ?, ?) \
         ON CONFLICT (day, content_id) DO UPDATE SET seconds = seconds + excluded.seconds",
    )
    .bind(day)
    .bind(content_id)
    .bind(seconds)
    .execute(db)
    .await?;
    Ok(())
}

/// Total por día desde `from_day` (inclusive).
pub async fn days_since(db: &SqlitePool, from_day: &str) -> sqlx::Result<Vec<DayTotal>> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT day, SUM(seconds) FROM playback_daily WHERE day >= ? GROUP BY day ORDER BY day",
    )
    .bind(from_day)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(day, seconds)| DayTotal { day, seconds })
        .collect())
}

/// Lo más visto desde `from_day`, agrupado por película o serie.
pub async fn top_since(
    db: &SqlitePool,
    from_day: &str,
    limit: i64,
) -> sqlx::Result<Vec<TitleTotal>> {
    // El título de la película, o el de la serie si es un episodio (episodio → temporada → serie).
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT COALESCE(show.title, c.title) AS title, SUM(p.seconds) AS total \
         FROM playback_daily p \
         JOIN contents c ON c.id = p.content_id \
         LEFT JOIN contents season ON season.id = c.parent_id \
         LEFT JOIN contents show ON show.id = season.parent_id \
         WHERE p.day >= ? \
         GROUP BY COALESCE(show.id, c.id) \
         ORDER BY total DESC LIMIT ?",
    )
    .bind(from_day)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(title, seconds)| TitleTotal { title, seconds })
        .collect())
}

/// Un contenido con progreso, con lo necesario para decidir qué mostrar en "seguir viendo".
#[derive(sqlx::FromRow)]
pub struct RecentProgress {
    pub content_id: i64,
    pub position: f64,
    pub duration: Option<f64>,
    pub watched: bool,
    pub episode_number: Option<i64>,
    pub season_number: Option<i64>,
    /// La película misma, o la serie del episodio.
    pub top_id: i64,
    pub top_title: String,
    pub is_episode: bool,
}

/// El progreso más reciente primero.
pub async fn recent_progress(
    db: &SqlitePool,
    profile_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<RecentProgress>> {
    sqlx::query_as(
        "SELECT p.content_id, p.position, p.duration, p.watched, \
                c.number AS episode_number, season.number AS season_number, \
                COALESCE(show.id, c.id) AS top_id, COALESCE(show.title, c.title) AS top_title, \
                c.kind = 'episode' AS is_episode \
         FROM progress p \
         JOIN contents c ON c.id = p.content_id \
         LEFT JOIN contents season ON season.id = c.parent_id AND c.kind = 'episode' \
         LEFT JOIN contents show ON show.id = season.parent_id \
         WHERE p.profile_id = ? \
         ORDER BY p.updated_at DESC LIMIT ?",
    )
    .bind(profile_id)
    .bind(limit)
    .fetch_all(db)
    .await
}

#[derive(sqlx::FromRow)]
pub struct NextEpisode {
    pub id: i64,
    pub episode_number: i64,
    pub season_number: i64,
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub watched: Option<bool>,
}

/// El episodio con archivo que sigue a (temporada, episodio) dentro de una serie.
pub async fn next_episode(
    db: &SqlitePool,
    profile_id: i64,
    show_id: i64,
    season: i64,
    episode: i64,
) -> sqlx::Result<Option<NextEpisode>> {
    sqlx::query_as(
        "SELECT e.id, e.number AS episode_number, s.number AS season_number, \
                p.position, p.duration, p.watched \
         FROM contents e \
         JOIN contents s ON s.id = e.parent_id \
         LEFT JOIN progress p ON p.content_id = e.id AND p.profile_id = ? \
         WHERE s.parent_id = ? AND e.kind = 'episode' \
           AND EXISTS (SELECT 1 FROM media_files f WHERE f.content_id = e.id) \
           AND (s.number > ? OR (s.number = ? AND e.number > ?)) \
         ORDER BY s.number, e.number LIMIT 1",
    )
    .bind(profile_id)
    .bind(show_id)
    .bind(season)
    .bind(season)
    .bind(episode)
    .fetch_optional(db)
    .await
}

// --- Historial de sesiones ---

/// Crea la sesión de una visualización que empieza. Guarda una copia del título y de la
/// biblioteca, para que el historial no dependa de que el contenido siga existiendo.
pub async fn start_session(
    db: &SqlitePool,
    content_id: i64,
    title: &str,
    profile: &Profile,
    client: &str,
    position: f64,
    duration: Option<f64>,
) -> sqlx::Result<i64> {
    let now = crate::db::now();
    sqlx::query_scalar(
        "INSERT INTO playback_sessions \
         (content_id, library_id, title, library_name, profile_id, profile_name, client, \
          started_at, ended_at, start_position, end_position, duration) \
         SELECT c.id, c.library_id, ?, l.name, ?, ?, ?, ?, ?, ?, ?, ? \
         FROM contents c JOIN libraries l ON l.id = c.library_id WHERE c.id = ? \
         RETURNING id",
    )
    .bind(title)
    .bind(profile.id)
    .bind(&profile.name)
    .bind(client)
    .bind(now)
    .bind(now)
    .bind(position)
    .bind(position)
    .bind(duration)
    .bind(content_id)
    .fetch_one(db)
    .await
}

/// Actualiza una sesión con un aviso del reproductor.
pub async fn update_session(
    db: &SqlitePool,
    id: i64,
    watched: f64,
    position: f64,
    duration: Option<f64>,
    completed: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE playback_sessions SET \
           ended_at = ?, seconds_watched = seconds_watched + ?, end_position = ?, \
           duration = COALESCE(?, duration), completed = completed OR ? \
         WHERE id = ?",
    )
    .bind(crate::db::now())
    .bind(watched)
    .bind(position)
    .bind(duration)
    .bind(completed)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

/// Las últimas sesiones, la más reciente primero. Las de menos de `min_seconds` se omiten
/// (abrir un episodio y cerrarlo enseguida no es "haberlo visto").
pub async fn recent_sessions(
    db: &SqlitePool,
    limit: i64,
    min_seconds: f64,
) -> sqlx::Result<Vec<super::Session>> {
    sqlx::query_as(
        "SELECT id, content_id, title, library_name, profile_name, client, started_at, ended_at, \
                seconds_watched, start_position, end_position, duration, completed \
         FROM playback_sessions WHERE seconds_watched >= ? \
         ORDER BY started_at DESC LIMIT ?",
    )
    .bind(min_seconds)
    .bind(limit)
    .fetch_all(db)
    .await
}

/// Total visto por biblioteca desde `since` (segundos desde 1970).
pub async fn libraries_since(db: &SqlitePool, since: i64) -> sqlx::Result<Vec<TitleTotal>> {
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT COALESCE(library_name, 'Sin biblioteca') AS name, SUM(seconds_watched) AS total \
         FROM playback_sessions WHERE started_at >= ? \
         GROUP BY COALESCE(library_id, library_name) ORDER BY total DESC",
    )
    .bind(since)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(title, seconds)| TitleTotal { title, seconds })
        .collect())
}
