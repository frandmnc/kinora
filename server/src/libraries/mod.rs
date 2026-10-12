//! Bibliotecas: categorías ("Películas", "Anime") con su carpeta destino.

mod handlers;
mod repo;

use axum::Router;
use axum::routing::{get, patch};
use serde::Serialize;
use ts_rs::TS;

use crate::app::AppState;

#[derive(Debug, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub root_dir: String,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/libraries", get(handlers::list).post(handlers::create))
        .route(
            "/libraries/{id}",
            patch(handlers::rename).delete(handlers::delete),
        )
}
