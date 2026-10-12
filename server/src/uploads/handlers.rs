use std::path::{Path, PathBuf};

use axum::Json;
use axum::body::Body;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use http_body_util::BodyExt;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tracing::info;

use super::{InboxFile, inbox_dir, resolve, validate_name};
use crate::app::AppState;
use crate::error::{AppError, AppResult};

/// GET /api/admin/uploads
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<InboxFile>>> {
    let inbox = inbox_dir(&state.settings.require_media_dir()?);
    let mut files = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(&inbox).await else {
        return Ok(Json(files)); // Todavía no se subió nada.
    };
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let metadata = entry.metadata().await?;
        // Los ".part" son subidas en curso o interrumpidas.
        if !metadata.is_file() || name.starts_with('.') || name.ends_with(".part") {
            continue;
        }
        files.push(InboxFile {
            name,
            size: metadata.len() as i64,
            uploaded_at: metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64),
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(files))
}

#[derive(Deserialize)]
pub struct UploadParams {
    name: String,
}

/// POST /api/admin/uploads?name=Frieren%20-%2001.mkv
/// El cuerpo es el archivo tal cual. Se escribe al disco a medida que llega, de a pedazos:
/// nunca se carga entero en memoria.
pub async fn upload(
    State(state): State<AppState>,
    Query(params): Query<UploadParams>,
    headers: HeaderMap,
    body: Body,
) -> AppResult<(StatusCode, Json<InboxFile>)> {
    validate_name(&params.name)?;
    let media_dir = state.settings.require_media_dir()?;
    let inbox = inbox_dir(&media_dir);
    tokio::fs::create_dir_all(&inbox).await?;

    // Si el navegador informa el tamaño, se comprueba que entre en el disco antes de empezar.
    let expected = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let free = crate::disk::space_of(&media_dir).map(|disk| disk.available);
    if let (Some(expected), Some(free)) = (expected, free)
        && expected > free
    {
        return Err(AppError::BadRequest(format!(
            "no hay espacio: el archivo pesa {} MB y quedan {} MB libres",
            expected / 1_000_000,
            free / 1_000_000
        )));
    }

    let part = inbox.join(format!(".{}.part", params.name));
    let size = match write_body(body, &part).await {
        Ok(size) => size,
        Err(err) => {
            // Subida cortada o error de disco: no se deja basura.
            let _ = tokio::fs::remove_file(&part).await;
            return Err(err);
        }
    };

    let destination = unique_path(&inbox, &params.name);
    tokio::fs::rename(&part, &destination).await?;
    let name = destination
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    info!("uploaded {name} to the inbox ({} MB)", size / 1_000_000);

    Ok((
        StatusCode::CREATED,
        Json(InboxFile {
            name,
            size: size as i64,
            uploaded_at: crate::db::now(),
        }),
    ))
}

/// DELETE /api/admin/uploads/{name}
pub async fn discard(
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
) -> AppResult<StatusCode> {
    let path = resolve(&state.settings.require_media_dir()?, &name)?;
    tokio::fs::remove_file(&path).await?;
    info!("discarded {name} from the inbox");
    Ok(StatusCode::NO_CONTENT)
}

/// Copia el cuerpo del pedido a un archivo, pedazo por pedazo. Devuelve los bytes escritos.
async fn write_body(mut body: Body, path: &Path) -> AppResult<u64> {
    let mut file = tokio::fs::File::create(path).await?;
    let mut size = 0u64;
    while let Some(frame) = body.frame().await {
        let frame = frame
            .map_err(|err| AppError::BadRequest(format!("la subida se interrumpió: {err}")))?;
        if let Ok(data) = frame.into_data() {
            file.write_all(&data).await?;
            size += data.len() as u64;
        }
    }
    file.flush().await?;
    Ok(size)
}

/// "a.mkv" si está libre; si no, "a (2).mkv", "a (3).mkv"...
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).{extension}")))
        .find(|path| !path.exists())
        .expect("siempre hay un número libre")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_path_adds_a_number() {
        let dir = std::env::temp_dir().join("kinora-test-unique");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_path(&dir, "a.mkv"), dir.join("a.mkv"));
        std::fs::write(dir.join("a.mkv"), b"").unwrap();
        std::fs::write(dir.join("a (2).mkv"), b"").unwrap();
        assert_eq!(unique_path(&dir, "a.mkv"), dir.join("a (3).mkv"));
    }
}
