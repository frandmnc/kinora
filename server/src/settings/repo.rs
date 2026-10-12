//! Consultas a la tabla `settings` (clave → valor).

use sqlx::SqlitePool;

pub async fn get(db: &SqlitePool, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await
}

pub async fn set(db: &SqlitePool, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?) \
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn set_overwrites_previous_value() {
        let db = connect_in_memory().await;
        assert_eq!(get(&db, "media_dir").await.unwrap(), None);
        set(&db, "media_dir", "/a").await.unwrap();
        set(&db, "media_dir", "/b").await.unwrap();
        assert_eq!(get(&db, "media_dir").await.unwrap().as_deref(), Some("/b"));
    }
}
