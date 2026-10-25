use std::path::PathBuf;

use std::net::SocketAddr;

use axum::Json;
use axum::extract::{ConnectInfo, Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;

use super::folders::{home_dir, is_writable, shortcuts, subfolders};
use super::relocate::{check_already_moved, move_libraries, same_disk, validate_move};
use super::transfer::{self, TransferStatus};
use super::{
    Folder, FolderListing, MediaDirChange, MediaDirSetting, MediaUsage, NewFolder, SetMediaDir,
    usage,
};
use crate::app::AppState;
use crate::config::expand_home;
use crate::disk::{self, format_size};
use crate::error::{AppError, AppResult};
use crate::libraries;
use crate::naming::safe_name;

/// GET /api/admin/settings/media-dir
pub async fn get_media_dir(State(state): State<AppState>) -> Json<MediaDirSetting> {
    let media_dir = state.settings.media_dir();
    Json(MediaDirSetting {
        available: media_dir.as_ref().is_some_and(|dir| dir.is_dir()),
        path: media_dir.map(|dir| dir.to_string_lossy().into_owned()),
        moving: state.settings.transfer.is_busy(),
    })
}

/// GET /api/admin/settings/media-dir/usage
pub async fn media_usage(State(state): State<AppState>) -> AppResult<Json<MediaUsage>> {
    let media_dir = state.settings.require_media_dir()?;
    let libraries = libraries::list_all(&state.db).await?;
    let usage = tokio::task::spawn_blocking(move || usage::measure(&media_dir, &libraries)).await?;
    Ok(Json(usage))
}

/// POST /api/admin/settings/media-dir/reveal
/// Abre la carpeta media en el Finder o el Explorador. Se abre en la pantalla de la compu donde
/// corre Kinora: por eso solo desde esa misma compu.
pub async fn reveal_media_dir(
    State(state): State<AppState>,
    peer: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> AppResult<StatusCode> {
    if !crate::auth::same_machine(&headers, &peer) {
        return Err(AppError::Forbidden(
            "la carpeta se abre en la compu donde corre Kinora: usá el Admin desde ahí".to_string(),
        ));
    }
    let media_dir = state.settings.require_media_dir()?;
    crate::platform::reveal_folder(&media_dir).map_err(|error| {
        AppError::Internal(anyhow::anyhow!(
            "could not open {}: {error}",
            media_dir.display()
        ))
    })?;
    Ok(StatusCode::NO_CONTENT)
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
        (Some(current), Some(MediaDirChange::Move)) if !same_disk(&current, &path) => {
            // Otro disco: se copia en segundo plano y la carpeta media cambia recién al terminar.
            let moves = validate_move(&current, &path, &library_dirs)?;
            if state.settings.transfer.is_busy() {
                return Err(AppError::Conflict(
                    "ya se están moviendo las bibliotecas".to_string(),
                ));
            }
            if state.jobs.is_running() {
                return Err(AppError::Conflict(
                    "la cola está procesando un archivo: pausala y esperá a que termine"
                        .to_string(),
                ));
            }
            let needed = tokio::task::spawn_blocking(move || {
                moves
                    .iter()
                    .map(|(from, _)| transfer::tree_size(from))
                    .sum::<u64>()
            })
            .await?;
            if let Some(space) = disk::space_of(&path)
                && space.available < needed + 1024 * 1024 * 1024
            {
                return Err(AppError::Conflict(format!(
                    "no entra: hacen falta {} (más 1 GB de margen) y en el disco nuevo hay {}",
                    format_size(needed),
                    format_size(space.available)
                )));
            }
            tracing::info!(
                "copying {} libraries from {} to {} ({})",
                library_dirs.len(),
                current.display(),
                path.display(),
                format_size(needed)
            );
            transfer::start(state.clone(), current, path, library_dirs);
            return Ok(Json(MediaDirSetting {
                path: state
                    .settings
                    .media_dir()
                    .map(|p| p.to_string_lossy().into_owned()),
                available: true,
                moving: true,
            }));
        }
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
        moving: false,
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

/// GET /api/admin/settings/media-dir/move
/// Cómo va la copia a otro disco (o cómo terminó la última).
pub async fn move_status(State(state): State<AppState>) -> Json<TransferStatus> {
    Json(state.settings.transfer.status())
}

/// POST /api/admin/settings/media-dir/move/cancel
/// Corta la copia: se borra lo copiado y todo queda como estaba.
pub async fn cancel_move(State(state): State<AppState>) -> AppResult<StatusCode> {
    if !state.settings.transfer.request_cancel() {
        return Err(AppError::Conflict(
            "no hay una copia en marcha para cancelar".to_string(),
        ));
    }
    Ok(StatusCode::ACCEPTED)
}
