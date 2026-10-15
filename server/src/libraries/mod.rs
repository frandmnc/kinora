//! Bibliotecas: categorías ("Películas", "Anime") con su carpeta destino.

mod handlers;
mod repo;

use axum::Router;
use axum::routing::{get, patch, put};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::AppState;

pub use repo::{find, list_all};

#[derive(Debug, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct Library {
    pub id: i64,
    pub name: String,
    /// Nombre de la carpeta dentro de `media_dir`.
    pub dir: String,
    /// El ícono elegido para el menú del Player ("clapperboard", "tv"…). `null`: se elige solo,
    /// según si tiene más películas o series.
    pub icon: Option<String>,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct LibraryIcon {
    pub icon: Option<String>,
}

/// Rutas del Player. Se montan bajo /api.
pub fn routes() -> Router<AppState> {
    Router::new().route("/libraries", get(handlers::list))
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/libraries", get(handlers::list).post(handlers::create))
        .route(
            "/libraries/{id}",
            patch(handlers::rename).delete(handlers::delete),
        )
        .route("/libraries/{id}/icon", put(handlers::set_icon))
}
