//! Cola de importación: los archivos que hay que procesar antes de entrar a la biblioteca.
//!
//! Importar agrega los trabajos pero no los empieza: la cola arranca cuando se toca "Iniciar" en
//! el Admin, y se detiene sola al vaciarse. "Pausar" deja terminar el trabajo en curso.
//! Si el servidor se reinicia, la cola sigue como estaba (en marcha o detenida).
//!
//! Es el único proceso en segundo plano de Kinora. Hace un trabajo por vez, en orden: remux
//! (cambiar la caja MKV → MP4 sin tocar la imagen, y si hace falta convertir el audio a AAC) o
//! conversión del video a HEVC con x265.
//!
//! Mientras espera, el archivo vive en `<biblioteca>/.kinora-tmp/job-<id>-source.<ext>` (fuera de
//! la bandeja, para que no se importe dos veces). El resultado se escribe en
//! `.kinora-tmp/job-<id>.mp4`, se verifica con ffprobe y recién ahí se mueve a su lugar. Si falla
//! o se cancela, el original vuelve a la bandeja.

mod handlers;
pub mod repo;
mod worker;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{get, post};
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::{Notify, oneshot};
use ts_rs::TS;

use crate::app::AppState;
use crate::imports::ImportAction;
use crate::probe::Operation;
use crate::settings;

pub use worker::{source_file_name, spawn_worker, stop_running};

/// El estado compartido de la cola: cómo despertar al trabajador y cómo cancelar lo que corre.
pub struct Jobs {
    wake: Notify,
    running: Mutex<Option<Running>>,
    /// Si la cola está en marcha. Se guarda en `settings` para sobrevivir a un reinicio.
    started: AtomicBool,
}

/// Clave en la tabla `settings`.
const STARTED_KEY: &str = "queue_started";

struct Running {
    id: i64,
    cancel: oneshot::Sender<()>,
}

impl Jobs {
    /// Arranca como quedó la última vez (detenida, si nunca se inició).
    pub async fn load(db: &SqlitePool) -> sqlx::Result<Arc<Jobs>> {
        let started = settings::repo::get(db, STARTED_KEY).await?.as_deref() == Some("1");
        Ok(Arc::new(Jobs {
            wake: Notify::new(),
            running: Mutex::new(None),
            started: AtomicBool::new(started),
        }))
    }

    /// Si hay un trabajo corriendo ahora.
    pub fn is_running(&self) -> bool {
        self.running.lock().unwrap().is_some()
    }

    pub fn is_started(&self) -> bool {
        self.started.load(Ordering::Relaxed)
    }

    /// Pone la cola en marcha o la detiene. Detenerla no corta el trabajo en curso: el
    /// trabajador lo termina y después no toma el siguiente.
    pub async fn set_started(&self, db: &SqlitePool, started: bool) -> sqlx::Result<()> {
        settings::repo::set(db, STARTED_KEY, if started { "1" } else { "0" }).await?;
        self.started.store(started, Ordering::Relaxed);
        if started {
            self.wake();
        }
        Ok(())
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
    /// `null` si el contenido se eliminó después (el trabajo queda en el historial).
    pub content_id: Option<i64>,
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
    /// Lo que dura el video original, en segundos (se conoce al empezar).
    pub duration: Option<f64>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

/// La cola para el Admin: si está en marcha y sus trabajos.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct QueueView {
    pub started: bool,
    /// Lo pendiente y lo que corre, y los últimos 20 terminados (del más nuevo al más viejo).
    pub jobs: Vec<JobView>,
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

/// Para el panel de Estado: cómo viene la cola y qué rindió.
#[derive(Debug, Default, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct ConversionSummary {
    /// Si la cola está en marcha (si no, lo pendiente espera a que se la inicie).
    #[sqlx(skip)]
    pub started: bool,
    pub pending: i64,
    pub running: i64,
    /// Terminados y fallidos en los últimos 30 días.
    pub done: i64,
    pub failed: i64,
    /// Lo que se ahorró convirtiendo (original − resultado), desde siempre. Solo conversiones:
    /// el remux deja el tamaño igual.
    pub saved_bytes: i64,
    /// Velocidad media de las conversiones de los últimos 30 días, en veces el tiempo real
    /// (2 = un episodio de 24 minutos tarda 12). `null` si todavía no hubo ninguna.
    pub speed: Option<f64>,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/jobs", get(handlers::list))
        .route("/jobs/start", post(handlers::start))
        .route("/jobs/pause", post(handlers::pause))
        .route("/jobs/{id}/cancel", post(handlers::cancel))
        .route("/metrics/conversion", get(handlers::summary))
}
