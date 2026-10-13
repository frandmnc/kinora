//! Cola de importación: los archivos que hay que procesar antes de entrar a la biblioteca.
//!
//! Es el único proceso en segundo plano de Kinora. Hace un trabajo por vez, en orden. Por ahora
//! solo remux (cambiar la caja MKV → MP4 sin tocar la imagen, y si hace falta convertir el audio
//! a AAC); en la Etapa 4 suma la conversión del video con x265.
//!
//! Mientras espera, el archivo vive en `<biblioteca>/.kinora-tmp/job-<id>-source.<ext>` (fuera de
//! la bandeja, para que no se importe dos veces). El resultado se escribe en
//! `.kinora-tmp/job-<id>.mp4`, se verifica con ffprobe y recién ahí se mueve a su lugar. Si falla
//! o se cancela, el original vuelve a la bandeja.

mod handlers;
pub mod repo;
mod worker;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{get, post};
use serde::Serialize;
use tokio::sync::{Notify, oneshot};
use ts_rs::TS;

use crate::app::AppState;
use crate::imports::ImportAction;
use crate::probe::Operation;

pub use worker::{source_file_name, spawn_worker};

/// El estado compartido de la cola: cómo despertar al trabajador y cómo cancelar lo que corre.
pub struct Jobs {
    wake: Notify,
    running: Mutex<Option<Running>>,
}

struct Running {
    id: i64,
    cancel: oneshot::Sender<()>,
}

impl Jobs {
    pub fn new() -> Arc<Jobs> {
        Arc::new(Jobs {
            wake: Notify::new(),
            running: Mutex::new(None),
        })
    }

    /// Avisa que hay trabajo nuevo. Si el trabajador está ocupado, lo ve al terminar.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Pide cancelar el trabajo `id` si es el que está corriendo. Devuelve si lo era.
    fn cancel_running(&self, id: i64) -> bool {
        let mut running = self.running.lock().unwrap();
        if running.as_ref().is_some_and(|r| r.id == id) {
            let _ = running.take().unwrap().cancel.send(());
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, sqlx::Type, TS)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
#[ts(export)]
pub enum JobState {
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct Job {
    pub id: i64,
    pub library_id: i64,
    pub content_id: i64,
    pub source_name: String,
    pub operation: Operation,
    pub action: ImportAction,
    pub rel_path: String,
    pub state: JobState,
    /// 0..1.
    pub progress: f64,
    pub error: Option<String>,
    pub source_size: i64,
    pub result_size: Option<i64>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

/// Un trabajo con el nombre de lo que se importa, para el Admin.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct JobView {
    #[serde(flatten)]
    #[ts(flatten)]
    pub job: Job,
    /// "Sousou no Frieren · T1 E5".
    pub title: String,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/jobs", get(handlers::list))
        .route("/jobs/{id}/cancel", post(handlers::cancel))
}
