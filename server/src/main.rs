mod app;
mod auth;
mod config;
mod contents;
mod db;
mod debug;
mod disk;
mod error;
mod health;
mod imports;
mod jobs;
mod libraries;
mod manifest;
mod metadata;
mod monitor;
mod naming;
mod opensubtitles;
mod platform;
mod playback;
mod probe;
mod profiles;
mod settings;
mod subtitles;
mod trash;
mod uploads;
mod web;

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::ChronoLocal;

use crate::app::AppState;
use crate::config::Config;
use crate::jobs::Jobs;
use crate::monitor::Monitor;
use crate::settings::Settings;

#[tokio::main]
async fn main() -> Result<()> {
    // Registro en la terminal con la hora local. Se puede ajustar el detalle con la
    // variable RUST_LOG, por ejemplo `RUST_LOG=kinora=debug cargo run`.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("kinora=info")),
        )
        .with_timer(ChronoLocal::new("%H:%M:%S".to_string()))
        .with_target(false)
        // Sin colores cuando no es una terminal (Kinora.app lo escribe en un archivo).
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();

    // La ruta de la configuración puede pasarse como primer argumento; si no, se busca
    // (`config::find`), y si no hay ninguna, todo va por defecto.
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(config::find);
    let config = match &config_path {
        Some(path) => Config::load(path)?,
        None => Config::defaults()?,
    };

    let db = db::connect(&config.data_dir).await?;
    let settings = Settings::load(&db).await?;
    // Los kinora.json se ponen al día en segundo plano: el servidor no espera a que terminen.
    if let Some(media_dir) = settings.media_dir() {
        let db = db.clone();
        tokio::spawn(async move { manifest::refresh_all(&db, &media_dir).await });
    }
    let monitor = Monitor::new();
    monitor::spawn_sampler(monitor.clone(), db.clone());
    let jobs = Jobs::load(&db).await?;
    let state = AppState {
        db,
        monitor: monitor.clone(),
        settings: settings.clone(),
        jobs,
        login_limiter: Default::default(),
    };
    jobs::spawn_worker(state.clone());
    let app = app::router(state);

    let addr = SocketAddr::new(config.bind, config.port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("could not listen on {addr}"))?;

    println!("Kinora listening on http://localhost:{}", config.port);
    if !config.bind.is_loopback()
        && let Ok(ip) = local_ip_address::local_ip()
    {
        println!("From the network:     http://{ip}:{}", config.port);
    }
    println!(
        "Admin:                http://localhost:{}/admin/",
        config.port
    );
    match settings.media_dir() {
        Some(media_dir) => {
            println!("Media:                {}", media_dir.display());
            if !media_dir.is_dir() {
                tracing::warn!("media folder {} is not available", media_dir.display());
            }
        }
        None => {
            println!("Media:                (not set)");
            tracing::warn!("media folder not set: choose it in Admin → Configuración");
        }
    }
    println!("Data:                 {}", config.data_dir.display());
    match &config_path {
        Some(path) => println!("Config:               {}", path.display()),
        None => println!("Config:               (defaults)"),
    }
    if !web::is_built() {
        tracing::warn!(
            "web app is not built: run `npm run build` in web/ (and rebuild a release binary) or use `npm run dev`"
        );
    }

    // `with_connect_info` permite a los handlers saber la IP de quien pide (para el monitor).
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    );
    // Al apagar (Ctrl+C, o `kill` de un servicio del sistema) se corta el ffmpeg de la cola:
    // si no, sigue solo y al volver a arrancar habría dos escribiendo el mismo archivo.
    tokio::select! {
        result = server => result.context("the server stopped with an error")?,
        signal = platform::shutdown_signal() => {
            tracing::info!("{signal}: shutting down");
            jobs::stop_running(&monitor);
        }
    }
    Ok(())
}
