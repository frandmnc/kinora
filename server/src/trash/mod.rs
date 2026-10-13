//! Papelera: lo que Kinora saca de una biblioteca no se borra. Va a
//! `<biblioteca>/.kinora-trash/<fecha y hora>/`, con la misma ruta que tenía adentro de la
//! biblioteca, así se puede recuperar a mano. Vaciarla (toda o por tanda) es una acción
//! explícita del Admin, en su propia pantalla.

mod handlers;

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::routing::delete;
use serde::Serialize;
use ts_rs::TS;

use crate::app::AppState;

/// La carpeta de la papelera dentro de cada biblioteca.
pub const DIR: &str = ".kinora-trash";

#[derive(Serialize, TS)]
#[ts(export)]
pub struct TrashView {
    /// Bytes en todas las papeleras.
    pub size: i64,
    pub libraries: Vec<TrashLibrary>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct TrashLibrary {
    pub library_id: i64,
    pub name: String,
    pub size: i64,
    /// De la más nueva a la más vieja.
    pub batches: Vec<TrashBatch>,
}

/// Una tanda: lo que salió de la biblioteca de una vez (un contenido eliminado, los archivos
/// reemplazados en una importación…).
#[derive(Serialize, TS)]
#[ts(export)]
pub struct TrashBatch {
    /// El nombre de la carpeta, "2026-10-13 10.49.29": identifica la tanda.
    pub name: String,
    /// Cuándo (segundos desde 1970), si el nombre se pudo leer como fecha.
    pub deleted_at: Option<i64>,
    pub size: i64,
    pub items: Vec<TrashItem>,
}

/// Lo que hay en una tanda, por carpeta de primer nivel (la película o serie).
#[derive(Serialize, TS)]
#[ts(export)]
pub struct TrashItem {
    pub name: String,
    pub files: i64,
    pub size: i64,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/trash",
            axum::routing::get(handlers::list).delete(handlers::empty),
        )
        .route(
            "/trash/{library_id}/{batch}",
            delete(handlers::delete_batch),
        )
}

/// Carpeta de la papelera para una tanda de archivos (todos los de una importación juntos).
pub fn batch_dir(root: &Path) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M.%S");
    root.join(DIR).join(stamp.to_string())
}

/// Mueve cada archivo (rutas relativas a `root`) a la papelera. Los que ya no están en el disco
/// se saltean. Devuelve los que se movieron, para poder devolverlos con `restore`.
pub async fn move_files(
    root: &Path,
    batch: &Path,
    rel_paths: &[String],
) -> std::io::Result<Vec<String>> {
    let mut moved = Vec::new();
    for rel in rel_paths {
        let target = batch.join(rel);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        match tokio::fs::rename(root.join(rel), &target).await {
            Ok(()) => moved.push(rel.clone()),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                // Lo que ya se movió vuelve a su lugar: o se mueve todo o nada.
                restore(root, batch, &moved).await;
                return Err(err);
            }
        }
    }
    Ok(moved)
}

/// Devuelve archivos de la papelera a su lugar (si algo falló después de moverlos).
pub async fn restore(root: &Path, batch: &Path, rel_paths: &[String]) {
    for rel in rel_paths {
        if let Err(err) = tokio::fs::rename(batch.join(rel), root.join(rel)).await {
            tracing::warn!("could not restore {rel} from the trash: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn moves_and_restores_keeping_the_path() {
        let root = std::env::temp_dir().join("kinora-test-trash");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Show/Season 01")).unwrap();
        std::fs::write(root.join("Show/Season 01/E01.mp4"), b"video").unwrap();

        let batch = batch_dir(&root);
        let files = vec![
            "Show/Season 01/E01.mp4".to_string(),
            "no-existe.vtt".to_string(),
        ];
        let moved = move_files(&root, &batch, &files).await.unwrap();
        assert_eq!(moved, ["Show/Season 01/E01.mp4"]);
        assert!(batch.join("Show/Season 01/E01.mp4").is_file());
        assert!(!root.join("Show/Season 01/E01.mp4").exists());

        restore(&root, &batch, &moved).await;
        assert!(root.join("Show/Season 01/E01.mp4").is_file());
    }
}
