//! Bandeja de entrada: archivos subidos desde el Admin que esperan ser importados.
//!
//! Vive en `<media_dir>/.kinora-inbox`, en el mismo disco que las bibliotecas:
//! importar es mover el archivo, que es instantáneo y no ocupa el doble de espacio.

mod handlers;

use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get};
use serde::Serialize;
use ts_rs::TS;

use crate::app::AppState;
use crate::error::{AppError, AppResult};

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "mov", "avi", "webm", "ts", "m2ts"];

#[derive(Serialize, TS)]
#[ts(export)]
pub struct InboxFile {
    pub name: String,
    pub size: i64,
    /// Segundos desde 1970 (UTC).
    pub uploaded_at: i64,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/uploads",
            get(handlers::list)
                // Axum limita los cuerpos a 2 MB por defecto; un video pesa gigas.
                .post(handlers::upload)
                .layer(DefaultBodyLimit::disable()),
        )
        .route("/uploads/{name}", delete(handlers::discard))
}

pub fn inbox_dir(media_dir: &Path) -> PathBuf {
    media_dir.join(".kinora-inbox")
}

/// Ruta de un archivo de la bandeja a partir de su nombre. Rechaza nombres que
/// intenten salir de la carpeta ("../", "/").
pub fn resolve(media_dir: &Path, name: &str) -> AppResult<PathBuf> {
    validate_name(name)?;
    let path = inbox_dir(media_dir).join(name);
    if !path.is_file() {
        return Err(AppError::NotFound(format!(
            "{name} no está en la bandeja de entrada"
        )));
    }
    Ok(path)
}

fn validate_name(name: &str) -> AppResult<()> {
    let invalid = name.is_empty()
        || name.starts_with('.')
        || name.contains(['/', '\\'])
        || name.chars().any(char::is_control);
    if invalid {
        return Err(AppError::BadRequest(format!(
            "\"{name}\" no es un nombre de archivo válido"
        )));
    }
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if !VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        return Err(AppError::BadRequest(format!("{name} no es un video")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_names_that_escape_the_inbox() {
        assert!(validate_name("Frieren - 01.mkv").is_ok());
        assert!(validate_name("../config.mkv").is_err());
        assert!(validate_name("a/b.mkv").is_err());
        assert!(validate_name(".oculto.mkv").is_err());
        assert!(validate_name("notas.txt").is_err());
        assert!(validate_name("Frieren - 01.spa.srt").is_err());
    }
}
