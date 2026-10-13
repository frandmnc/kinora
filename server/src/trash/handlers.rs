use std::fs;
use std::path::{Path, PathBuf};

use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use tracing::info;

use super::{DIR, TrashBatch, TrashItem, TrashLibrary, TrashView};
use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::libraries;

/// GET /api/admin/trash
/// Qué hay en la papelera de cada biblioteca, por tanda (una por eliminación o importación).
pub async fn list(State(state): State<AppState>) -> AppResult<Json<TrashView>> {
    let media_dir = state.settings.require_media_dir()?;
    let libraries = libraries::list_all(&state.db).await?;
    let view = tokio::task::spawn_blocking(move || {
        let libraries: Vec<TrashLibrary> = libraries
            .into_iter()
            .map(|library| {
                let batches = read_batches(&media_dir.join(&library.dir).join(DIR));
                TrashLibrary {
                    size: batches.iter().map(|b| b.size).sum(),
                    library_id: library.id,
                    name: library.name,
                    batches,
                }
            })
            .collect();
        TrashView {
            size: libraries.iter().map(|l| l.size).sum(),
            libraries,
        }
    })
    .await?;
    Ok(Json(view))
}

/// DELETE /api/admin/trash
/// Vacía la papelera de todas las bibliotecas. Lo que se borra no se puede recuperar.
pub async fn empty(State(state): State<AppState>) -> AppResult<StatusCode> {
    let media_dir = state.settings.require_media_dir()?;
    let folders: Vec<PathBuf> = libraries::list_all(&state.db)
        .await?
        .into_iter()
        .map(|library| media_dir.join(library.dir).join(DIR))
        .filter(|folder| folder.is_dir())
        .collect();
    let freed = tokio::task::spawn_blocking(move || -> std::io::Result<u64> {
        let mut freed = 0;
        for folder in folders {
            freed += tree_size(&folder);
            fs::remove_dir_all(&folder)?;
        }
        Ok(freed)
    })
    .await??;
    info!("trash emptied: {:.1} MB freed", freed as f64 / 1e6);
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/admin/trash/{library_id}/{batch}
/// Borra para siempre una tanda de la papelera de una biblioteca.
pub async fn delete_batch(
    State(state): State<AppState>,
    UrlPath((library_id, batch)): UrlPath<(i64, String)>,
) -> AppResult<StatusCode> {
    // El nombre de la tanda es una carpeta adentro de la papelera: nada de "../" ni "/".
    if batch.is_empty() || batch.starts_with('.') || batch.contains(['/', '\\']) {
        return Err(AppError::BadRequest(format!(
            "\"{batch}\" no es una tanda de la papelera"
        )));
    }
    let library = libraries::find(&state.db, library_id).await?;
    let folder = state
        .settings
        .require_media_dir()?
        .join(&library.dir)
        .join(DIR)
        .join(&batch);
    if !folder.is_dir() {
        return Err(AppError::NotFound(format!(
            "no hay una tanda \"{batch}\" en la papelera de {}",
            library.name
        )));
    }
    tokio::task::spawn_blocking(move || fs::remove_dir_all(folder)).await??;
    info!("deleted trash batch {batch} of {}", library.name);
    Ok(StatusCode::NO_CONTENT)
}

/// Las tandas de una papelera, de la más nueva a la más vieja.
fn read_batches(trash: &Path) -> Vec<TrashBatch> {
    let Ok(entries) = fs::read_dir(trash) else {
        return Vec::new();
    };
    let mut batches: Vec<TrashBatch> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let items = read_items(&entry.path());
            TrashBatch {
                deleted_at: parse_batch_name(&name),
                size: items.iter().map(|item| item.size).sum(),
                items,
                name,
            }
        })
        .collect();
    batches.sort_by(|a, b| b.name.cmp(&a.name));
    batches
}

/// Lo que hay en una tanda, agrupado por su primera carpeta (la película o serie a la que
/// pertenecía): "Sousou no Frieren (2023)" con 7 archivos, por ejemplo.
fn read_items(batch: &Path) -> Vec<TrashItem> {
    let Ok(entries) = fs::read_dir(batch) else {
        return Vec::new();
    };
    let mut items: Vec<TrashItem> = entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            let (size, files) = if path.is_dir() {
                (tree_size(&path), count_files(&path))
            } else {
                (entry.metadata().map(|m| m.len()).unwrap_or(0), 1)
            };
            TrashItem {
                name: entry.file_name().to_string_lossy().into_owned(),
                files,
                size: size as i64,
            }
        })
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items
}

/// "2026-10-13 10.49.29" (el nombre de la tanda) → segundos desde 1970.
fn parse_batch_name(name: &str) -> Option<i64> {
    let local = chrono::NaiveDateTime::parse_from_str(name, "%Y-%m-%d %H.%M.%S").ok()?;
    Some(
        local
            .and_local_timezone(chrono::Local)
            .single()?
            .timestamp(),
    )
}

fn tree_size(dir: &Path) -> u64 {
    crate::settings::transfer::tree_size(dir)
}

fn count_files(dir: &Path) -> i64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => count_files(&entry.path()),
            Ok(kind) if kind.is_file() => 1,
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_each_batch_by_its_first_folder() {
        let trash = std::env::temp_dir().join("kinora-test-trash-view");
        let _ = fs::remove_dir_all(&trash);
        let older = trash.join("2026-10-12 09.00.00/Prueba/Season 01");
        let newer = trash.join("2026-10-13 10.49.29/Sousou no Frieren (2023)/Season 01");
        fs::create_dir_all(&older).unwrap();
        fs::create_dir_all(&newer).unwrap();
        fs::write(older.join("E01.mp4"), vec![0u8; 100]).unwrap();
        fs::write(older.join("E01.spa.vtt"), b"vtt").unwrap();
        fs::write(newer.join("E01.mp4"), vec![0u8; 50]).unwrap();

        let batches = read_batches(&trash);
        assert_eq!(batches.len(), 2);
        assert_eq!(
            batches[0].name, "2026-10-13 10.49.29",
            "la más nueva primero"
        );
        assert_eq!(batches[1].size, 103);
        assert_eq!(batches[1].items[0].name, "Prueba");
        assert_eq!(batches[1].items[0].files, 2);
        assert!(batches[0].deleted_at.is_some());
        assert_eq!(parse_batch_name("cualquier cosa"), None);
    }
}
