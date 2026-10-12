mod app;
mod config;
mod contents;
mod db;
mod debug;
mod error;
mod health;
mod imports;
mod libraries;
mod naming;
mod probe;
mod sources;
mod web;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::app::AppState;
use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // La ruta de la configuración puede pasarse como primer argumento.
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.toml"));
    let config = Arc::new(Config::load(&config_path)?);

    let db = db::connect(&config.data_dir).await?;
    let app = app::router(AppState {
        db,
        config: config.clone(),
    });

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
    println!(
        "Admin:                http://localhost:{}/admin/",
        config.port
    );
    println!("Media en {}", config.media_dir.display());
    println!("Datos en {}", config.data_dir.display());
    if !web::is_built() {
        println!(
            "Aviso: no está compilada la web ({}). Ejecutá `npm run build` en web/ o usá `npm run dev`.",
            web::dist_dir().display()
        );
    }

    axum::serve(listener, app)
        .await
        .context("el servidor se detuvo con un error")?;
    Ok(())
}
