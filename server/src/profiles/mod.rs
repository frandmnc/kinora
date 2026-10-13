//! Perfiles: cada persona de la casa dentro de la cuenta (como en Netflix). Cada uno tiene su
//! progreso, su "Seguir viendo" y su historial.
//!
//! El Player elige el perfil al abrirse ("¿Quién está mirando?") y lo manda en cada pedido en el
//! encabezado `Kinora-Profile`. Los handlers que dependen del perfil lo reciben con el
//! extractor `Profile`: si falta o no existe, el pedido se rechaza antes de llegar al handler.

pub mod handlers;
pub mod repo;

use axum::Router;
use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::request::Parts;
use axum::routing::{get, patch};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::AppState;
use crate::error::AppError;

/// El encabezado con el perfil elegido.
pub const HEADER: &str = "kinora-profile";

/// El perfil de un pedido.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct Profile {
    pub id: i64,
    pub name: String,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ProfileChanges {
    pub name: String,
}

/// Rutas (con sesión). Se montan bajo /api: los perfiles se manejan desde el Player.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/profiles", get(handlers::list).post(handlers::create))
        .route(
            "/profiles/{id}",
            patch(handlers::rename).delete(handlers::remove),
        )
}

/// Saca el perfil del encabezado y comprueba que exista. Los handlers que lo piden como
/// argumento (`profile: Profile`) no se ejecutan sin uno válido.
impl FromRequestParts<AppState> for Profile {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        <Profile as OptionalFromRequestParts<AppState>>::from_request_parts(parts, state)
            .await?
            .ok_or_else(|| AppError::BadRequest("elegí un perfil".to_string()))
    }
}

/// Lo mismo, pero opcional (`Option<Profile>`): el Admin pide cosas sin perfil.
impl OptionalFromRequestParts<AppState> for Profile {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Option<Self>, AppError> {
        let Some(value) = parts.headers.get(HEADER) else {
            return Ok(None);
        };
        let id: i64 = value
            .to_str()
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .ok_or_else(|| AppError::BadRequest("el perfil no es válido".to_string()))?;
        let profile = repo::find(&state.db, id).await?.ok_or_else(|| {
            AppError::BadRequest("ese perfil ya no existe: elegí otro".to_string())
        })?;
        Ok(Some(profile))
    }
}
