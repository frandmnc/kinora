mod config;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use axum::{Json, Router, routing::get};
use serde::Serialize;

use crate::config::Config;

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

#[tokio::main]
async fn main() -> Result<()> {
    // La ruta de la configuración puede pasarse como primer argumento.
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.toml"));
    let config = Config::load(&config_path)?;

    let app = Router::new().route("/api/health", get(health));

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
