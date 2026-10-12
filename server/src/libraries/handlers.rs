use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use ts_rs::TS;

use super::{Library, repo};
use crate::app::AppState;
use crate::config::expand_home;
use crate::error::{AppError, AppResult};

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct NewLibrary {
    name: String,
    root_dir: String,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct LibraryChanges {
    name: String,
}

/// GET /api/admin/libraries
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Library>>> {
    let libraries = repo::list_all(&state.db).await?;
    Ok(Json(libraries))
}

/// POST /api/admin/libraries
pub async fn create(
    State(state): State<AppState>,
    Json(new): Json<NewLibrary>,
) -> AppResult<(StatusCode, Json<Library>)> {
    let name = validate_name(&new.name)?;
    let root_dir = validate_root_dir(&new.root_dir)?;
    let library = repo::insert(&state.db, &name, &root_dir).await?;
    Ok((StatusCode::CREATED, Json(library)))
}

/// PATCH /api/admin/libraries/{id}
pub async fn rename(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(changes): Json<LibraryChanges>,
) -> AppResult<Json<Library>> {
    let name = validate_name(&changes.name)?;
    let library = repo::update_name(&state.db, id, &name).await?;
    Ok(Json(library))
}

/// DELETE /api/admin/libraries/{id}
pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<StatusCode> {
    repo::remove(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "el nombre no puede estar vacío".to_string(),
        ));
    }
    Ok(name.to_string())
}

/// La carpeta tiene que existir. Se guarda como ruta absoluta y sin enlaces simbólicos.
fn validate_root_dir(root_dir: &str) -> AppResult<String> {
    let path = expand_home(std::path::Path::new(root_dir.trim()))?;
    if !path.is_absolute() {
        return Err(AppError::BadRequest(format!(
            "{} no es una ruta absoluta",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(AppError::BadRequest(format!(
            "{} no existe o no es una carpeta",
            path.display()
        )));
    }
    let path = path.canonicalize()?;
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest("la ruta tiene caracteres no válidos".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_name_and_missing_folder() {
        assert!(matches!(validate_name("   "), Err(AppError::BadRequest(_))));
        assert!(matches!(
            validate_root_dir("/no/existe"),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            validate_root_dir("relativa"),
            Err(AppError::BadRequest(_))
        ));
    }
}
