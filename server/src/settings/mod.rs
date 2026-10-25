//! Configuración editable desde el Admin. Por ahora, la carpeta media.
//!
//! Se guarda en la tabla `settings` y se mantiene una copia en memoria para no
//! consultar la base en cada pedido.

mod folders;
mod handlers;
mod relocate;
pub mod repo;
pub mod transfer;
mod usage;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use axum::Router;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

use crate::app::AppState;
use crate::error::{AppError, AppResult};

const MEDIA_DIR: &str = "media_dir";

pub struct Settings {
    media_dir: RwLock<Option<PathBuf>>,
    /// La copia de las bibliotecas a otro disco, si hay una en marcha.
    pub transfer: transfer::Transfer,
}

impl Settings {
    pub async fn load(db: &SqlitePool) -> Result<Arc<Settings>> {
        let media_dir = repo::get(db, MEDIA_DIR).await?.map(PathBuf::from);
        transfer::clean_up_interrupted(db).await;
        Ok(Arc::new(Settings {
            media_dir: RwLock::new(media_dir),
            transfer: transfer::Transfer::new(),
        }))
    }

    pub fn media_dir(&self) -> Option<PathBuf> {
        self.media_dir.read().unwrap().clone()
    }

    /// La carpeta media, o un error claro si todavía no se eligió o no está disponible
    /// (por ejemplo, un disco externo desconectado).
    pub fn require_media_dir(&self) -> AppResult<PathBuf> {
        let media_dir = self.media_dir().ok_or_else(|| {
            AppError::BadRequest(
                "todavía no elegiste la carpeta media (Admin → Configuración)".to_string(),
            )
        })?;
        if !media_dir.is_dir() {
            return Err(AppError::BadRequest(format!(
                "la carpeta media {} no está disponible",
                media_dir.display()
            )));
        }
        Ok(media_dir)
    }

    pub async fn set_media_dir(&self, db: &SqlitePool, path: PathBuf) -> Result<()> {
        repo::set(db, MEDIA_DIR, &path.to_string_lossy()).await?;
        *self.media_dir.write().unwrap() = Some(path);
        Ok(())
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct MediaDirSetting {
    /// `null` si todavía no se eligió.
    pub path: Option<String>,
    pub available: bool,
    /// Si se están copiando las bibliotecas a otro disco (la carpeta cambia al terminar).
    pub moving: bool,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SetMediaDir {
    pub path: String,
    /// Obligatorio si ya hay bibliotecas: qué hacer con sus carpetas.
    pub change: Option<MediaDirChange>,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MediaDirChange {
    /// Kinora mueve las carpetas de las bibliotecas (solo dentro del mismo disco).
    Move,
    /// Las carpetas ya están en la ubicación nueva: solo se verifica.
    AlreadyMoved,
}

/// Cuánto ocupa la carpeta media, y qué es cada subcarpeta.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct MediaUsage {
    pub path: String,
    pub size: i64,
    pub files: i64,
    pub disk_total: Option<i64>,
    pub disk_available: Option<i64>,
    pub folders: Vec<MediaFolderUsage>,
    /// Bibliotecas cuya carpeta no está en la carpeta media.
    pub missing: Vec<LibraryRef>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct MediaFolderUsage {
    pub name: String,
    pub size: i64,
    pub files: i64,
    pub role: FolderRole,
    pub library: Option<LibraryRef>,
}

/// Qué es una subcarpeta de la carpeta media. El orden de las variantes es el orden en pantalla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FolderRole {
    Library,
    Inbox,
    /// Una carpeta que no es de ninguna biblioteca (quizás de antes, o de Plex).
    Other,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct LibraryRef {
    pub id: i64,
    pub name: String,
}

/// Una carpeta del servidor, para el explorador del Admin.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct Folder {
    pub name: String,
    pub path: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct FolderListing {
    pub path: String,
    pub parent: Option<String>,
    pub folders: Vec<Folder>,
    /// Lugares para empezar: la carpeta del usuario y los discos.
    pub shortcuts: Vec<Folder>,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct NewFolder {
    /// Carpeta donde crearla.
    pub parent: String,
    pub name: String,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/settings/media-dir",
            get(handlers::get_media_dir).put(handlers::set_media_dir),
        )
        .route("/settings/media-dir/usage", get(handlers::media_usage))
        .route(
            "/settings/media-dir/reveal",
            post(handlers::reveal_media_dir),
        )
        .route("/settings/media-dir/move", get(handlers::move_status))
        .route(
            "/settings/media-dir/move/cancel",
            post(handlers::cancel_move),
        )
        .route(
            "/folders",
            get(handlers::browse_folders).post(handlers::create_folder),
        )
}

/// Mientras se copian las bibliotecas a otro disco, el Admin no deja hacer cambios: lo que se
/// escribiera en la carpeta vieja se perdería al terminar. Se puede mirar (GET) y cancelar la copia.
pub async fn block_while_moving(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let reading = request.method() == axum::http::Method::GET;
    let cancelling = request
        .uri()
        .path()
        .ends_with("/settings/media-dir/move/cancel");
    if state.settings.transfer.is_busy() && !reading && !cancelling {
        return AppError::Conflict(
            "se están copiando las bibliotecas a otro disco: esperá a que termine (o cancelalo en \
             Configuración)"
                .to_string(),
        )
        .into_response();
    }
    next.run(request).await
}
