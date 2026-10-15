use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use serde::Deserialize;
use ts_rs::TS;

use super::{Library, LibraryIcon, repo};
use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::naming::safe_name;

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct NewLibrary {
    name: String,
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
/// Crea la biblioteca y su carpeta dentro de `media_dir`. Si la carpeta ya existe, la usa.
pub async fn create(
    State(state): State<AppState>,
    Json(new): Json<NewLibrary>,
) -> AppResult<(StatusCode, Json<Library>)> {
    let name = validate_name(&new.name)?;
    let dir = folder_for(&name)?;
    let media_dir = state.settings.require_media_dir()?;

    // Primero la base: detecta nombres o carpetas repetidos antes de tocar el disco.
    let library = repo::insert(&state.db, &name, &dir).await?;
    if let Err(err) = tokio::fs::create_dir_all(media_dir.join(&dir)).await {
        repo::remove(&state.db, library.id).await?;
        return Err(err.into());
    }
    Ok((StatusCode::CREATED, Json(library)))
}

/// PATCH /api/admin/libraries/{id}
/// Renombra la biblioteca y su carpeta. Los archivos guardan rutas relativas a la carpeta
/// de la biblioteca, así que siguen siendo válidos.
pub async fn rename(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<i64>,
    Json(changes): Json<LibraryChanges>,
) -> AppResult<Json<Library>> {
    let name = validate_name(&changes.name)?;
    let new_dir = folder_for(&name)?;
    let library = repo::find(&state.db, id).await?;

    if new_dir == library.dir {
        return Ok(Json(repo::update(&state.db, id, &name, &new_dir).await?));
    }

    let media_dir = state.settings.require_media_dir()?;
    let old_path = media_dir.join(&library.dir);
    let new_path = media_dir.join(&new_dir);
    // En macOS "anime" y "Anime" son la misma carpeta: cambiar solo mayúsculas no es un conflicto.
    let only_case_changes = new_dir.to_lowercase() == library.dir.to_lowercase();
    if new_path.exists() && !only_case_changes {
        return Err(AppError::Conflict(format!(
            "ya existe la carpeta {new_dir} en {}",
            media_dir.display()
        )));
    }

    let moved = old_path.exists();
    if moved {
        tokio::fs::rename(&old_path, &new_path).await?;
    }
    match repo::update(&state.db, id, &name, &new_dir).await {
        Ok(library) => Ok(Json(library)),
        Err(err) => {
            // La base rechazó el cambio: la carpeta vuelve a su nombre anterior.
            if moved {
                let _ = tokio::fs::rename(&new_path, &old_path).await;
            }
            Err(err)
        }
    }
}

/// PUT /api/admin/libraries/{id}/icon
/// El ícono del menú: un nombre de los que conoce el Player ("tv", "popcorn"), o `null` para que
/// se elija solo. Acá solo se controla la forma: si el Player no lo conoce, usa el automático.
pub async fn set_icon(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<i64>,
    Json(body): Json<LibraryIcon>,
) -> AppResult<Json<Library>> {
    let icon = body
        .icon
        .as_deref()
        .map(str::trim)
        .filter(|icon| !icon.is_empty());
    if let Some(icon) = icon
        && (icon.len() > 40
            || !icon
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
    {
        return Err(AppError::BadRequest(format!(
            "\"{icon}\" no es un ícono válido"
        )));
    }
    Ok(Json(repo::set_icon(&state.db, id, icon).await?))
}

/// DELETE /api/admin/libraries/{id}
/// Solo bibliotecas sin contenido. La carpeta queda en el disco.
pub async fn delete(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<i64>,
) -> AppResult<StatusCode> {
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

/// El nombre de la carpeta sale del nombre de la biblioteca: "Películas" → "Películas".
fn folder_for(name: &str) -> AppResult<String> {
    let dir = safe_name(name);
    if dir.is_empty() {
        return Err(AppError::BadRequest(format!(
            "\"{name}\" no sirve como nombre de carpeta"
        )));
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_names() {
        assert!(matches!(validate_name("   "), Err(AppError::BadRequest(_))));
        assert!(matches!(folder_for("///"), Err(AppError::BadRequest(_))));
        assert_eq!(folder_for("Series: TV").unwrap(), "Series TV");
    }
}
