//! Consultas a la tabla `profiles`.

use sqlx::SqlitePool;

use super::Profile;
use crate::db::now;

pub async fn list(db: &SqlitePool) -> sqlx::Result<Vec<Profile>> {
    sqlx::query_as("SELECT id, name FROM profiles ORDER BY id")
        .fetch_all(db)
        .await
}

pub async fn find(db: &SqlitePool, id: i64) -> sqlx::Result<Option<Profile>> {
    sqlx::query_as("SELECT id, name FROM profiles WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Si otro perfil ya usa ese nombre (sin distinguir mayúsculas).
pub async fn name_taken(db: &SqlitePool, name: &str, except: Option<i64>) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM profiles WHERE name = ? COLLATE NOCASE AND id != ?)",
    )
    .bind(name)
    .bind(except.unwrap_or(0))
    .fetch_one(db)
    .await
}

pub async fn create(db: &SqlitePool, name: &str) -> sqlx::Result<Profile> {
    sqlx::query_as("INSERT INTO profiles (name, created_at) VALUES (?, ?) RETURNING id, name")
        .bind(name)
        .bind(now())
        .fetch_one(db)
        .await
}

pub async fn rename(db: &SqlitePool, id: i64, name: &str) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE profiles SET name = ? WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() == 1)
}

/// Borra el perfil: su progreso se va con él; el historial queda (con el nombre guardado).
pub async fn delete(db: &SqlitePool, id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn count(db: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM profiles")
        .fetch_one(db)
        .await
}
