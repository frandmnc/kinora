//! Consultas a `account` y `sessions`.

use sqlx::SqlitePool;

use super::SessionInfo;
use crate::db::now;

/// Una sesión deja de valer si no se usa en este tiempo.
pub const SESSION_IDLE_SECS: i64 = 180 * 24 * 3600;

pub async fn account_exists(db: &SqlitePool) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM account)")
        .fetch_one(db)
        .await
}

/// El usuario y el hash de la contraseña, si la cuenta existe.
pub async fn account(db: &SqlitePool) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as("SELECT username, password_hash FROM account WHERE id = 1")
        .fetch_optional(db)
        .await
}

/// Crea la cuenta. Falla si ya existe (la tabla admite una sola fila).
pub async fn create_account(
    db: &SqlitePool,
    username: &str,
    password_hash: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO account (id, username, password_hash, created_at) VALUES (1, ?, ?, ?)",
    )
    .bind(username)
    .bind(password_hash)
    .bind(now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_password(db: &SqlitePool, password_hash: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE account SET password_hash = ? WHERE id = 1")
        .bind(password_hash)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn create_session(
    db: &SqlitePool,
    token_hash: &str,
    device: Option<&str>,
    client: &str,
) -> sqlx::Result<i64> {
    let now = now();
    sqlx::query_scalar(
        "INSERT INTO sessions (token_hash, device, client, created_at, last_seen_at) \
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(token_hash)
    .bind(device)
    .bind(client)
    .bind(now)
    .bind(now)
    .fetch_one(db)
    .await
}

/// La sesión de un token, si existe y no venció. De paso anota que se usó (como mucho una vez
/// por hora, para no escribir en la base en cada pedido).
pub async fn touch_session(db: &SqlitePool, token_hash: &str) -> sqlx::Result<Option<i64>> {
    let now = now();
    let found: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, last_seen_at FROM sessions WHERE token_hash = ? AND last_seen_at > ?",
    )
    .bind(token_hash)
    .bind(now - SESSION_IDLE_SECS)
    .fetch_optional(db)
    .await?;
    let Some((id, last_seen)) = found else {
        return Ok(None);
    };
    if now - last_seen > 3600 {
        sqlx::query("UPDATE sessions SET last_seen_at = ? WHERE id = ?")
            .bind(now)
            .bind(id)
            .execute(db)
            .await?;
    }
    Ok(Some(id))
}

pub async fn delete_session(db: &SqlitePool, id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() == 1)
}

/// Cierra todas las sesiones menos una (al cambiar la contraseña).
pub async fn delete_other_sessions(db: &SqlitePool, keep: i64) -> sqlx::Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE id != ?")
        .bind(keep)
        .execute(db)
        .await?;
    Ok(result.rows_affected())
}

pub async fn sessions(db: &SqlitePool) -> sqlx::Result<Vec<SessionInfo>> {
    sqlx::query_as(
        "SELECT id, device, client, created_at, last_seen_at, 0 AS current FROM sessions \
         WHERE last_seen_at > ? ORDER BY last_seen_at DESC",
    )
    .bind(now() - SESSION_IDLE_SECS)
    .fetch_all(db)
    .await
}
