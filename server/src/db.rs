use std::path::Path;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

/// Abre la base de datos (la crea si no existe) y aplica las migraciones pendientes.
pub async fn connect(data_dir: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(data_dir.join("kinora.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let db = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .context("no se pudo abrir la base de datos")?;

    sqlx::migrate!()
        .run(&db)
        .await
        .context("no se pudieron aplicar las migraciones")?;

    Ok(db)
}

/// Base de datos en memoria para los tests, con las migraciones aplicadas.
#[cfg(test)]
pub async fn connect_in_memory() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true);
    // Una sola conexión: cada conexión en memoria sería una base distinta.
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!().run(&db).await.unwrap();
    db
}
