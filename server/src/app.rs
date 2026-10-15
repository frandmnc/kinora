//! Estado compartido y armado de todas las rutas.

use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use sqlx::SqlitePool;

use crate::auth::{self, LoginLimiter};
use crate::error::AppError;
use crate::jobs::Jobs;
use crate::monitor::Monitor;
use crate::settings::Settings;
use crate::{
    contents, debug, health, imports, jobs, libraries, metadata, monitor, opensubtitles, playback,
    profiles, settings, subtitles, trash, uploads, web,
};

/// Lo que comparten todos los handlers. Axum le pasa una copia a cada pedido;
/// copiar un `SqlitePool` es barato porque todas las copias usan las mismas conexiones.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub monitor: Arc<Monitor>,
    pub settings: Arc<Settings>,
    pub jobs: Arc<Jobs>,
    /// Freno a los intentos de contraseña.
    pub login_limiter: Arc<LoginLimiter>,
}

pub fn router(state: AppState) -> Router {
    let admin = Router::new()
        .merge(monitor::admin_routes())
        .merge(playback::admin_routes())
        .merge(settings::admin_routes())
        .merge(uploads::admin_routes())
        .merge(libraries::admin_routes())
        .merge(contents::admin_routes())
        .merge(metadata::admin_routes())
        .merge(opensubtitles::admin_routes())
        .merge(imports::admin_routes())
        .merge(jobs::admin_routes())
        .merge(subtitles::admin_routes())
        .merge(trash::admin_routes())
        .merge(debug::admin_routes())
        .merge(auth::admin_routes())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            settings::block_while_moving,
        ));

    // Todo lo que está acá pide sesión. Una capa (`layer`) se aplica a las rutas agregadas
    // antes que ella: por eso lo público se suma después, afuera.
    let protected = Router::new()
        .merge(libraries::routes())
        .merge(contents::routes())
        .merge(metadata::routes())
        .merge(playback::routes())
        .merge(subtitles::routes())
        .merge(profiles::routes())
        .nest("/admin", admin)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ));

    let api = Router::new()
        .route("/health", get(health::health))
        .merge(auth::public_routes())
        .merge(protected)
        .fallback(api_not_found);

    Router::new()
        .nest("/api", api)
        .fallback_service(web::service())
        .with_state(state)
}

async fn api_not_found() -> AppError {
    AppError::NotFound("ruta de API inexistente".to_string())
}
