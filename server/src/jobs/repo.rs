//! Consultas a la tabla `import_jobs`.

use sqlx::SqlitePool;

use super::Job;
use crate::db::now;
use crate::imports::ImportAction;
use crate::probe::Operation;

const COLUMNS: &str = "id, library_id, content_id, source_name, operation, action, rel_path, \
    state, progress, error, source_size, result_size, created_at, started_at, finished_at";

pub struct NewJob<'a> {
    pub library_id: i64,
    pub content_id: i64,
    pub source_name: &'a str,
    pub operation: Operation,
    pub action: ImportAction,
    pub rel_path: &'a str,
    pub source_size: i64,
}

pub async fn insert(db: &SqlitePool, new: &NewJob<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO import_jobs \
         (library_id, content_id, source_name, operation, action, rel_path, state, source_size, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?) RETURNING id",
    )
    .bind(new.library_id)
    .bind(new.content_id)
    .bind(new.source_name)
    .bind(new.operation)
    .bind(new.action)
    .bind(new.rel_path)
    .bind(new.source_size)
    .bind(now())
    .fetch_one(db)
    .await
}

pub async fn delete(db: &SqlitePool, id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM import_jobs WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn get(db: &SqlitePool, id: i64) -> sqlx::Result<Option<Job>> {
    let sql = format!("SELECT {COLUMNS} FROM import_jobs WHERE id = ?");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// El próximo trabajo a hacer: el pendiente más viejo.
pub async fn next_pending(db: &SqlitePool) -> sqlx::Result<Option<Job>> {
    let sql =
        format!("SELECT {COLUMNS} FROM import_jobs WHERE state = 'pending' ORDER BY id LIMIT 1");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .fetch_optional(db)
        .await
}

/// Los pendientes y el que corre, y los últimos terminados.
pub async fn recent(db: &SqlitePool, finished: i64) -> sqlx::Result<Vec<Job>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM import_jobs WHERE state IN ('pending', 'running') \
         UNION ALL \
         SELECT * FROM (SELECT {COLUMNS} FROM import_jobs \
           WHERE state NOT IN ('pending', 'running') ORDER BY id DESC LIMIT ?) \
         ORDER BY id DESC"
    );
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(finished)
        .fetch_all(db)
        .await
}

/// Los que quedaron "corriendo" porque el servidor se apagó en el medio.
pub async fn interrupted(db: &SqlitePool) -> sqlx::Result<Vec<Job>> {
    let sql = format!("SELECT {COLUMNS} FROM import_jobs WHERE state = 'running'");
    sqlx::query_as(sqlx::AssertSqlSafe(sql)).fetch_all(db).await
}

/// Empieza: pasa a "corriendo" y anota la hora. Devuelve `false` si ya no estaba pendiente
/// (por ejemplo, si lo cancelaron justo antes).
pub async fn start(db: &SqlitePool, id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE import_jobs SET state = 'running', started_at = ?, progress = 0, error = NULL \
         WHERE id = ? AND state = 'pending'",
    )
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Vuelve a la fila (después de un reinicio en el medio).
pub async fn requeue(db: &SqlitePool, id: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE import_jobs SET state = 'pending', started_at = NULL, progress = 0 WHERE id = ?",
    )
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn cancel(db: &SqlitePool, id: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE import_jobs SET state = 'cancelled', finished_at = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Cancela un trabajo que todavía no empezó. Devuelve `false` si ya había empezado.
pub async fn cancel_pending(db: &SqlitePool, id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE import_jobs SET state = 'cancelled', finished_at = ? \
         WHERE id = ? AND state = 'pending'",
    )
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn set_progress(db: &SqlitePool, id: i64, progress: f64) -> sqlx::Result<()> {
    sqlx::query("UPDATE import_jobs SET progress = ? WHERE id = ?")
        .bind(progress)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn fail(db: &SqlitePool, id: i64, error: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE import_jobs SET state = 'failed', error = ?, finished_at = ? WHERE id = ?")
        .bind(error)
        .bind(now())
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn finish(
    db: &SqlitePool,
    id: i64,
    rel_path: &str,
    result_size: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE import_jobs SET state = 'done', progress = 1, rel_path = ?, result_size = ?, \
         finished_at = ? WHERE id = ?",
    )
    .bind(rel_path)
    .bind(result_size)
    .bind(now())
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}
