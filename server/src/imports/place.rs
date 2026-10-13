//! El último paso de toda importación: poner el archivo en su lugar de la biblioteca y
//! registrarlo. Lo usan la importación directa (archivos que ya cumplen el formato) y la cola
//! (archivos que primero hubo que remuxar).

use std::path::Path;

use anyhow::{Context, bail};
use sqlx::SqliteConnection;

use crate::app::AppState;
use crate::contents::repo::{self, NewMediaFile, insert_media_file};
use crate::imports::paths;
use crate::probe::Analysis;
use crate::trash;

pub struct PlaceFile<'a> {
    pub content_id: i64,
    /// Destino, relativo a la carpeta de la biblioteca.
    pub rel_path: &'a str,
    /// El archivo listo (en la bandeja o en `.kinora-tmp/`). Se mueve, no se copia.
    pub file: &'a Path,
    /// El análisis de ese archivo (el resultado, no el original si hubo remux).
    pub analysis: &'a Analysis,
    /// Nombre con el que se subió, para el registro.
    pub source_name: &'a str,
    /// Archivos que reemplaza: van a la papelera con sus subtítulos.
    pub replaces: &'a [i64],
}

/// Pone el archivo en su lugar y lo registra. Si reemplaza a otros, primero los manda a la
/// papelera (con sus subtítulos); si después algo falla, vuelven a su lugar.
/// Devuelve el id del archivo nuevo y lo que fue a la papelera.
pub async fn place(
    state: &AppState,
    root: &Path,
    trash_batch: &Path,
    request: PlaceFile<'_>,
) -> anyhow::Result<(i64, Vec<String>)> {
    let mut old_paths = Vec::new();
    for &id in request.replaces {
        old_paths.extend(repo::paths_of_file(&mut *state.db.acquire().await?, id).await?);
    }
    let trashed = trash::move_files(root, trash_batch, &old_paths)
        .await
        .context("no se pudo mover el archivo anterior a la papelera")?;

    if let Err(err) = move_into_library(root, request.file, request.rel_path).await {
        trash::restore(root, trash_batch, &trashed).await;
        return Err(err);
    }

    // En la base, el cambio es de una vez: se van los anteriores y entra el nuevo.
    let size = tokio::fs::metadata(root.join(request.rel_path))
        .await?
        .len() as i64;
    let probe = &request.analysis.probe;
    let video = probe.video();
    let media_file = NewMediaFile {
        content_id: request.content_id,
        rel_path: request.rel_path.to_string(),
        size,
        duration: probe.duration(),
        video_codec: video.and_then(|v| v.codec_string()),
        width: video.and_then(|v| v.width),
        height: video.and_then(|v| v.height),
        probe_json: request.analysis.json.clone(),
        source_path: request.source_name.to_string(),
    };
    let mut tx = state.db.begin().await?;
    for &id in request.replaces {
        repo::delete_media_file(&mut tx, id).await?;
    }
    let id = insert_media_file(&mut tx, &media_file).await?;
    tx.commit().await?;
    if !trashed.is_empty() {
        tracing::info!("moved to the trash: {}", trashed.join(", "));
    }
    Ok((id, trashed))
}

/// La primera versión libre: `… - v2.mp4`, `… - v3.mp4`…
pub async fn free_version(
    conn: &mut SqliteConnection,
    root: &Path,
    path: &str,
) -> sqlx::Result<String> {
    for version in 2.. {
        let candidate = paths::version_file(path, version);
        if !root.join(&candidate).exists() && !repo::rel_path_taken(conn, &candidate).await? {
            return Ok(candidate);
        }
    }
    unreachable!("siempre hay una versión libre")
}

/// Mueve un archivo a su lugar en la biblioteca. Como la bandeja y `.kinora-tmp/` están en el
/// mismo disco, es un renombrado: instantáneo y atómico. Si no se puede renombrar (otro disco),
/// se copia a `.kinora-tmp/` y recién al terminar se mueve, para que nunca quede a medias.
async fn move_into_library(root: &Path, source: &Path, rel_path: &str) -> anyhow::Result<()> {
    let destination = root.join(rel_path);
    if destination.exists() {
        bail!("ya existe {}", destination.display());
    }
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    if tokio::fs::rename(source, &destination).await.is_err() {
        let tmp_dir = root.join(".kinora-tmp");
        tokio::fs::create_dir_all(&tmp_dir).await?;
        let name = source.file_name().unwrap_or_default().to_string_lossy();
        let tmp = tmp_dir.join(format!("{name}.part"));
        let copied = async {
            tokio::fs::copy(source, &tmp)
                .await
                .context("no se pudo copiar")?;
            tokio::fs::rename(&tmp, &destination)
                .await
                .context("no se pudo mover a la biblioteca")
        }
        .await;
        if copied.is_err() {
            let _ = tokio::fs::remove_file(&tmp).await;
        }
        copied?;
        tokio::fs::remove_file(source).await?;
    }
    Ok(())
}
