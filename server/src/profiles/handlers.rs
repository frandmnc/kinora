use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use tracing::info;

use super::{Profile, ProfileChanges, repo};
use crate::app::AppState;
use crate::error::{AppError, AppResult};

/// Lo más largo que puede ser un nombre de perfil (se ve en una tarjeta en la TV).
const MAX_NAME: usize = 20;

/// GET /api/profiles
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Profile>>> {
    Ok(Json(repo::list(&state.db).await?))
}

/// POST /api/profiles
pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<ProfileChanges>,
) -> AppResult<(StatusCode, Json<Profile>)> {
    let name = clean_name(&request.name)?;
    if repo::name_taken(&state.db, &name, None).await? {
        return Err(AppError::Conflict(format!("ya hay un perfil \"{name}\"")));
    }
    let profile = repo::create(&state.db, &name).await?;
    info!("profile created: {name}");
    Ok((StatusCode::CREATED, Json(profile)))
}

/// PATCH /api/profiles/{id}
pub async fn rename(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ProfileChanges>,
) -> AppResult<Json<Profile>> {
    let name = clean_name(&request.name)?;
    if repo::name_taken(&state.db, &name, Some(id)).await? {
        return Err(AppError::Conflict(format!("ya hay un perfil \"{name}\"")));
    }
    if !repo::rename(&state.db, id, &name).await? {
        return Err(AppError::NotFound(format!("no existe el perfil {id}")));
    }
    Ok(Json(Profile { id, name }))
}

/// DELETE /api/profiles/{id}
/// Se borra con su progreso. Siempre tiene que quedar al menos uno.
pub async fn remove(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<StatusCode> {
    if repo::count(&state.db).await? <= 1 {
        return Err(AppError::Conflict(
            "es el único perfil: tiene que quedar al menos uno".to_string(),
        ));
    }
    if !repo::delete(&state.db, id).await? {
        return Err(AppError::NotFound(format!("no existe el perfil {id}")));
    }
    info!("profile {id} deleted");
    Ok(StatusCode::NO_CONTENT)
}

pub fn clean_name(name: &str) -> AppResult<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "el perfil necesita un nombre".to_string(),
        ));
    }
    if name.chars().count() > MAX_NAME {
        return Err(AppError::BadRequest(format!(
            "el nombre puede tener hasta {MAX_NAME} letras"
        )));
    }
    Ok(name)
}
