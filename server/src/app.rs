//! Estado compartido y armado de todas las rutas.

use axum::Router;
use axum::routing::get;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::{debug, health, libraries, web};

/// Lo que comparten todos los handlers. Axum le pasa una copia a cada pedido;
/// copiar un `SqlitePool` es barato porque todas las copias usan las mismas conexiones.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

pub fn router(state: AppState) -> Router {
    let admin = Router::new()
        .merge(libraries::admin_routes())
        .merge(debug::admin_routes());

    let api = Router::new()
        .route("/health", get(health::health))
        .nest("/admin", admin)
        .fallback(api_not_found);

    Router::new()
        .nest("/api", api)
        .fallback_service(web::service())
        .with_state(state)
}

async fn api_not_found() -> AppError {
    AppError::NotFound("ruta de API inexistente".to_string())
}
