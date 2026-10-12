mod config;
mod db;
mod error;
mod libraries;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::config::Config;
use crate::error::AppError;

/// Lo que comparten todos los handlers. Axum le pasa una copia a cada pedido;
/// copiar un `SqlitePool` es barato porque todas las copias usan las mismas conexiones.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

// Lo que devuelve /api/health. `Serialize` permite convertirlo a JSON.
#[derive(Serialize)]
struct Health {
    status: String,
    version: String,
}

// Un "handler": la función que responde a una ruta.
async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn api_not_found() -> AppError {
    AppError::NotFound("ruta de API inexistente".to_string())
}

fn router(state: AppState) -> Router {
    let admin = Router::new()
        .route("/libraries", get(libraries::list).post(libraries::create))
        .route(
            "/libraries/{id}",
            patch(libraries::rename).delete(libraries::delete),
        );

    let api = Router::new()
        .route("/health", get(health))
        .nest("/admin", admin)
        .fallback(api_not_found);

    Router::new().nest("/api", api).with_state(state)
}

#[tokio::main]
async fn main() -> Result<()> {
    // La ruta de la configuración puede pasarse como primer argumento.
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.toml"));
    let config = Config::load(&config_path)?;

    let db = db::connect(&config.data_dir).await?;
    let app = router(AppState { db });

    let addr = SocketAddr::new(config.bind, config.port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("no se pudo abrir {addr}"))?;

    println!("Kinora escuchando en http://localhost:{}", config.port);
    if !config.bind.is_loopback()
        && let Ok(ip) = local_ip_address::local_ip()
    {
        println!("Desde la red:         http://{ip}:{}", config.port);
    }
    println!("Datos en {}", config.data_dir.display());

    axum::serve(listener, app)
        .await
        .context("el servidor se detuvo con un error")?;
    Ok(())
}
