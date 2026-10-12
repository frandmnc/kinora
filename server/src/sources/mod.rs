//! Carpetas de origen: desde dónde se puede importar (`import_sources` en config.toml).

mod handlers;

use std::fs;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::routing::get;
use serde::Serialize;
use ts_rs::TS;

use crate::app::AppState;
use crate::error::{AppError, AppResult};

#[derive(Serialize, TS)]
#[ts(export)]
pub struct Source {
    pub path: String,
    pub available: bool,
    pub files: Vec<SourceFile>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct SourceFile {
    /// Ruta absoluta: es lo que se manda al analizar o importar.
    pub path: String,
    /// Ruta relativa a la carpeta de origen, para mostrar.
    pub name: String,
    pub size: i64,
}

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "mov", "avi", "webm", "ts", "m2ts"];
const MAX_DEPTH: usize = 3;

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new().route("/sources", get(handlers::list))
}

/// Lista los videos de una carpeta de origen y de sus subcarpetas (hasta 3 niveles).
/// Ignora archivos y carpetas ocultos.
pub fn list_videos(root: &Path) -> Vec<SourceFile> {
    let mut files = Vec::new();
    walk(root, root, 0, &mut files);
    files.sort_by(|a, b| a.name.cmp(&b.name));
    files
}

fn walk(dir: &Path, root: &Path, depth: usize, files: &mut Vec<SourceFile>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            if depth < MAX_DEPTH {
                walk(&path, root, depth + 1, files);
            }
        } else if is_video(&path) {
            files.push(SourceFile {
                name: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
                path: path.to_string_lossy().into_owned(),
                size: metadata.len() as i64,
            });
        }
    }
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

/// Comprueba que `path` sea un archivo dentro de alguna carpeta de origen.
/// Devuelve la ruta canónica (absoluta y sin `..` ni enlaces simbólicos).
pub fn resolve_inside(sources: &[PathBuf], path: &str) -> AppResult<PathBuf> {
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|_| AppError::BadRequest(format!("{path} no existe")))?;

    let inside = sources
        .iter()
        .filter_map(|source| source.canonicalize().ok())
        .any(|source| canonical.starts_with(source));

    if !inside || !canonical.is_file() {
        return Err(AppError::BadRequest(format!(
            "{path} no es un archivo de una carpeta de origen"
        )));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_accepts_files_inside_sources() {
        let source = std::env::temp_dir().join("kinora-test-source");
        fs::create_dir_all(source.join("sub")).unwrap();
        fs::write(source.join("sub/a.mkv"), b"x").unwrap();
        let sources = vec![source.clone()];

        let inside = source.join("sub/a.mkv");
        assert!(resolve_inside(&sources, inside.to_str().unwrap()).is_ok());

        let escape = source.join("sub/../../");
        assert!(resolve_inside(&sources, escape.to_str().unwrap()).is_err());
        assert!(resolve_inside(&sources, "/etc/hosts").is_err());

        let names: Vec<String> = list_videos(&source).into_iter().map(|f| f.name).collect();
        assert_eq!(names, ["sub/a.mkv"]);
    }
}
