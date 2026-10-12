use std::path::PathBuf;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::Deserialize;

use super::folders::{home_dir, is_writable, shortcuts, subfolders};
use super::relocate::{check_already_moved, move_libraries};
use super::{
    Folder, FolderListing, MediaDirChange, MediaDirSetting, MediaUsage, NewFolder, SetMediaDir,
    usage,
};
use crate::app::AppState;
use crate::config::expand_home;
use crate::error::{AppError, AppResult};
use crate::libraries;
use crate::naming::safe_name;

/// GET /api/admin/settings/media-dir
pub async fn get_media_dir(State(state): State<AppState>) -> Json<MediaDirSetting> {
    let media_dir = state.settings.media_dir();
    Json(MediaDirSetting {
        available: media_dir.as_ref().is_some_and(|dir| dir.is_dir()),
        path: media_dir.map(|dir| dir.to_string_lossy().into_owned()),
    })
}

/// GET /api/admin/settings/media-dir/usage
pub async fn media_usage(State(state): State<AppState>) -> AppResult<Json<MediaUsage>> {
    let media_dir = state.settings.require_media_dir()?;
    let libraries = libraries::list_all(&state.db).await?;
    let usage = tokio::task::spawn_blocking(move || usage::measure(&media_dir, &libraries)).await?;
    Ok(Json(usage))
}

/// PUT /api/admin/settings/media-dir
/// Si ya hay bibliotecas, `change` dice si Kinora mueve sus carpetas o si ya se movieron.
pub async fn set_media_dir(
    State(state): State<AppState>,
    Json(request): Json<SetMediaDir>,
) -> AppResult<Json<MediaDirSetting>> {
    let path = existing_folder(&request.path)?;
    if !is_writable(&path) {
        return Err(AppError::BadRequest(format!(
            "Kinora no puede escribir en {}",
            path.display()
        )));
    }

    let library_dirs: Vec<String> = libraries::list_all(&state.db)
        .await?
        .into_iter()
        .map(|library| library.dir)
        .collect();
    let current = state.settings.media_dir();

    match (current, request.change) {
        (Some(current), _) if current == path => {}
        (Some(_), _) | (None, _) if library_dirs.is_empty() => {}
        (Some(current), Some(MediaDirChange::Move)) => {
            let (from, to, dirs) = (current.clone(), path.clone(), library_dirs.clone());
            tokio::task::spawn_blocking(move || move_libraries(&from, &to, &dirs)).await??;
            tracing::info!(
                "moved {} libraries from {} to {}",
                library_dirs.len(),
                current.display(),
                path.display()
            );
        }
        (None, Some(MediaDirChange::Move)) => {
            return Err(AppError::BadRequest(
                "no hay una carpeta media actual de donde mover: elegí \"Ya lo moví yo\""
                    .to_string(),
            ));
        }
        (_, Some(MediaDirChange::AlreadyMoved)) => check_already_moved(&path, &library_dirs)?,
        (_, _) => {
            return Err(AppError::BadRequest(
                "ya hay bibliotecas: elegí si Kinora mueve sus carpetas o si ya las moviste vos"
                    .to_string(),
            ));
        }
    }

    state
        .settings
        .set_media_dir(&state.db, path.clone())
        .await?;
    tracing::info!("media folder set to {}", path.display());
    Ok(Json(MediaDirSetting {
        path: Some(path.to_string_lossy().into_owned()),
        available: true,
    }))
}

#[derive(Deserialize)]
pub struct BrowseParams {
    path: Option<String>,
}

/// GET /api/admin/folders?path=/Users/fran
/// Sin `path`, empieza en la carpeta del usuario.
pub async fn browse_folders(Query(params): Query<BrowseParams>) -> AppResult<Json<FolderListing>> {
    let path = match params.path.as_deref() {
        Some(path) if !path.trim().is_empty() => existing_folder(path)?,
        _ => home_dir(),
    };

    let listing = tokio::task::spawn_blocking(move || {
        let folders = subfolders(&path)
            .map_err(|_| AppError::BadRequest(format!("no se puede leer {}", path.display())))?;
        Ok::<_, AppError>(FolderListing {
            parent: path.parent().map(|p| p.to_string_lossy().into_owned()),
            path: path.to_string_lossy().into_owned(),
            folders,
            shortcuts: shortcuts(&home_dir()),
        })
    })
    .await??;
    Ok(Json(listing))
}

/// POST /api/admin/folders
pub async fn create_folder(
    Json(request): Json<NewFolder>,
) -> AppResult<(StatusCode, Json<Folder>)> {
    let parent = existing_folder(&request.parent)?;
    let name = safe_name(&request.name);
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "el nombre de la carpeta no es válido".to_string(),
        ));
    }
    let path = parent.join(&name);
    tokio::fs::create_dir(&path)
        .await
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::AlreadyExists => {
                AppError::Conflict(format!("ya existe la carpeta {name}"))
            }
            _ => AppError::BadRequest(format!("no se pudo crear {}: {err}", path.display())),
        })?;
    Ok((
        StatusCode::CREATED,
        Json(Folder {
            name,
            path: path.to_string_lossy().into_owned(),
        }),
    ))
}

/// Ruta absoluta de una carpeta que existe, sin `..` ni enlaces simbólicos.
fn existing_folder(path: &str) -> AppResult<PathBuf> {
    let path = expand_home(std::path::Path::new(path.trim()))?;
    if !path.is_absolute() {
        return Err(AppError::BadRequest(format!(
            "{} no es una ruta absoluta",
            path.display()
        )));
    }
    let path = path
        .canonicalize()
        .map_err(|_| AppError::BadRequest(format!("{} no existe", path.display())))?;
    if !path.is_dir() {
        return Err(AppError::BadRequest(format!(
            "{} no es una carpeta",
            path.display()
        )));
    }
    Ok(path)
}
