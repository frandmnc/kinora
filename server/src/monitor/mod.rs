//! Monitor del servidor: quién está mirando, transferencias en curso, CPU y memoria.
//!
//! Los datos se ven en dos lugares: el registro de la terminal de Kinora
//! y la pantalla Estado del Admin (GET /api/admin/status).

mod handlers;
mod sampler;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::routing::get;
use serde::Serialize;
use tracing::info;
use ts_rs::TS;

use crate::app::AppState;
use crate::contents::PlaybackState;

pub use sampler::spawn_sampler;

/// Si un Player no informa nada en este tiempo, se considera que dejó de mirar
/// (por ejemplo, se apagó la TV sin cerrar el reproductor).
const VIEWER_TIMEOUT: Duration = Duration::from_secs(30);

/// Estado compartido del monitor. Vive en un `Arc` dentro de `AppState`.
pub struct Monitor {
    started: Instant,
    next_transfer_id: AtomicU64,
    bytes_sent: AtomicU64,
    transfers: Mutex<HashMap<u64, Transfer>>,
    viewers: Mutex<HashMap<(String, i64), Viewer>>,
    latest: RwLock<Sample>,
    /// El ffmpeg de la cola, mientras corre (0 = ninguno).
    worker_pid: AtomicU32,
}

/// Alguien mirando, según lo que informa su Player.
#[derive(Clone, Serialize, TS)]
#[ts(export)]
pub struct Viewer {
    pub client: String,
    pub content_id: i64,
    pub title: String,
    pub position: f64,
    pub duration: Option<f64>,
    pub playing: bool,
    /// Segundos desde 1970 (UTC).
    pub started_at: i64,
    #[serde(skip)]
    #[ts(skip)]
    last_seen: Instant,
    /// La fila de `playback_sessions` de esta visualización, cuando ya se creó.
    #[serde(skip)]
    #[ts(skip)]
    session_id: Option<i64>,
}

/// Qué hacer con el historial después de un aviso del reproductor.
#[derive(Debug, PartialEq)]
pub enum SessionStep {
    /// Empezó a mirar: hay que crear la sesión.
    Start,
    /// Sigue (o terminó) una sesión ya creada.
    Update(i64),
    /// La sesión se está creando todavía: este aviso se saltea.
    Pending,
}

#[derive(Debug)]
pub struct ViewerReport {
    /// Segundos de reproducción desde el aviso anterior.
    pub watched: f64,
    pub session: SessionStep,
}

impl Viewer {
    fn session_step(&self) -> SessionStep {
        self.session_id
            .map_or(SessionStep::Pending, SessionStep::Update)
    }

    fn watched_since_last_report(&self) -> f64 {
        if self.playing {
            self.last_seen.elapsed().min(VIEWER_TIMEOUT).as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Una descarga de video en curso (un pedido HTTP que todavía está enviando datos).
#[derive(Clone, Serialize, TS)]
#[ts(export)]
pub struct Transfer {
    pub client: String,
    pub file_id: i64,
    pub file_name: String,
    /// Segundos desde 1970 (UTC).
    pub started_at: i64,
    pub bytes_sent: i64,
}

/// La última medición de CPU y memoria.
#[derive(Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct Sample {
    /// Porcentaje de un núcleo: 100 = un núcleo completo.
    pub cpu_kinora: f32,
    /// Porcentaje de toda la CPU (0–100).
    pub cpu_system: f32,
    pub cpu_cores: i64,
    pub memory_kinora: i64,
    /// El ffmpeg de la cola (convirtiendo o remuxando); `null` si no hay nada en curso.
    pub cpu_ffmpeg: Option<f32>,
    pub memory_ffmpeg: Option<i64>,
    pub memory_used: i64,
    pub memory_total: i64,
    /// Bytes por segundo enviados en los últimos segundos.
    pub send_rate: i64,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ServerStatus {
    pub uptime_secs: i64,
    pub bytes_sent_total: i64,
    pub sample: Sample,
    pub viewers: Vec<Viewer>,
    pub transfers: Vec<Transfer>,
}

/// Mientras exista, la transferencia figura como en curso. Al soltarse (cuando el navegador
/// termina o corta la descarga) se quita sola: es el patrón RAII de Rust.
pub struct TransferGuard {
    monitor: Arc<Monitor>,
    id: u64,
}

impl Drop for TransferGuard {
    fn drop(&mut self) {
        self.monitor.transfers.lock().unwrap().remove(&self.id);
    }
}

impl TransferGuard {
    pub fn add_bytes(&self, bytes: u64) {
        self.monitor.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        if let Some(transfer) = self.monitor.transfers.lock().unwrap().get_mut(&self.id) {
            transfer.bytes_sent += bytes as i64;
        }
    }
}

impl Monitor {
    pub fn new() -> Arc<Monitor> {
        Arc::new(Monitor {
            started: Instant::now(),
            next_transfer_id: AtomicU64::new(1),
            bytes_sent: AtomicU64::new(0),
            transfers: Mutex::new(HashMap::new()),
            viewers: Mutex::new(HashMap::new()),
            latest: RwLock::new(Sample::default()),
            worker_pid: AtomicU32::new(0),
        })
    }

    /// Qué proceso de ffmpeg medir (o ninguno). Lo avisa la cola al arrancar y terminar.
    pub fn set_worker_pid(&self, pid: Option<u32>) {
        self.worker_pid.store(pid.unwrap_or(0), Ordering::Relaxed);
    }

    pub fn worker_pid(&self) -> Option<u32> {
        Some(self.worker_pid.load(Ordering::Relaxed)).filter(|&pid| pid != 0)
    }

    /// Registra una transferencia nueva. Se da de baja cuando se suelta el `TransferGuard`.
    pub fn start_transfer(
        self: &Arc<Self>,
        client: String,
        file_id: i64,
        file_name: String,
    ) -> TransferGuard {
        let id = self.next_transfer_id.fetch_add(1, Ordering::Relaxed);
        let transfer = Transfer {
            client,
            file_id,
            file_name,
            started_at: crate::db::now(),
            bytes_sent: 0,
        };
        self.transfers.lock().unwrap().insert(id, transfer);
        TransferGuard {
            monitor: Arc::clone(self),
            id,
        }
    }

    /// Actualiza "viendo ahora" con lo que informa un Player, y anota los cambios en la terminal.
    ///
    /// Devuelve los segundos de reproducción desde el aviso anterior: el tiempo real transcurrido
    /// si estaba reproduciendo, con un tope (si la TV se apagó sin avisar, no se cuentan horas).
    /// Así adelantar no suma tiempo y una pausa tampoco.
    pub fn report_viewer(
        &self,
        client: String,
        content_id: i64,
        title: String,
        position: f64,
        duration: Option<f64>,
        state: PlaybackState,
    ) -> ViewerReport {
        let mut viewers = self.viewers.lock().unwrap();
        let key = (client.clone(), content_id);
        let at = clock(position);

        if state == PlaybackState::Stopped {
            return match viewers.remove(&key) {
                Some(viewer) => {
                    info!("■ {client} stopped watching {title} at {at}");
                    ViewerReport {
                        watched: viewer.watched_since_last_report(),
                        session: viewer.session_step(),
                    }
                }
                None => ViewerReport {
                    watched: 0.0,
                    session: SessionStep::Pending,
                },
            };
        }

        let playing = state == PlaybackState::Playing;
        match viewers.get_mut(&key) {
            Some(viewer) => {
                let watched = viewer.watched_since_last_report();
                if viewer.playing != playing {
                    if playing {
                        info!("▶ {client} resumed {title} at {at}");
                    } else {
                        info!("⏸ {client} paused {title} at {at}");
                    }
                }
                viewer.position = position;
                viewer.duration = duration;
                viewer.playing = playing;
                viewer.last_seen = Instant::now();
                ViewerReport {
                    watched,
                    session: viewer.session_step(),
                }
            }
            None => {
                info!("▶ {client} started watching {title} from {at}");
                viewers.insert(
                    key,
                    Viewer {
                        client,
                        content_id,
                        title,
                        position,
                        duration,
                        playing,
                        started_at: crate::db::now(),
                        last_seen: Instant::now(),
                        session_id: None,
                    },
                );
                ViewerReport {
                    watched: 0.0,
                    session: SessionStep::Start,
                }
            }
        }
    }

    /// Anota la sesión de historial creada para una visualización.
    pub fn set_session(&self, client: &str, content_id: i64, session_id: i64) {
        if let Some(viewer) = self
            .viewers
            .lock()
            .unwrap()
            .get_mut(&(client.to_string(), content_id))
        {
            viewer.session_id = Some(session_id);
        }
    }

    /// Quita a quienes dejaron de informar. Lo llama el medidor periódicamente.
    fn expire_viewers(&self) {
        self.viewers.lock().unwrap().retain(|_, viewer| {
            let alive = viewer.last_seen.elapsed() < VIEWER_TIMEOUT;
            if !alive {
                info!(
                    "■ {} stopped watching {} at {} (no signal from the player)",
                    viewer.client,
                    viewer.title,
                    clock(viewer.position)
                );
            }
            alive
        });
    }

    pub fn status(&self) -> ServerStatus {
        ServerStatus {
            uptime_secs: self.started.elapsed().as_secs() as i64,
            bytes_sent_total: self.bytes_sent.load(Ordering::Relaxed) as i64,
            sample: self.latest.read().unwrap().clone(),
            viewers: self.viewer_list(),
            transfers: self.grouped_transfers(),
        }
    }

    fn viewer_list(&self) -> Vec<Viewer> {
        let mut list: Vec<Viewer> = self.viewers.lock().unwrap().values().cloned().collect();
        list.sort_by_key(|v| v.started_at);
        list
    }

    /// El navegador suele abrir varios pedidos para un mismo video (al empezar y al adelantar).
    /// Se agrupan por cliente y archivo.
    fn grouped_transfers(&self) -> Vec<Transfer> {
        let transfers = self.transfers.lock().unwrap();
        let mut grouped: HashMap<(String, i64), Transfer> = HashMap::new();
        for transfer in transfers.values() {
            grouped
                .entry((transfer.client.clone(), transfer.file_id))
                .and_modify(|g| {
                    g.bytes_sent += transfer.bytes_sent;
                    g.started_at = g.started_at.min(transfer.started_at);
                })
                .or_insert_with(|| transfer.clone());
        }
        let mut list: Vec<Transfer> = grouped.into_values().collect();
        list.sort_by_key(|t| t.started_at);
        list
    }
}

/// 754.2 → "12:34", 3723 → "1:02:03".
pub fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0) as i64;
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new().route("/status", get(handlers::status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_disappears_when_guard_is_dropped() {
        let monitor = Monitor::new();
        let first = monitor.start_transfer("192.168.0.20".into(), 1, "a.mp4".into());
        let second = monitor.start_transfer("192.168.0.20".into(), 1, "a.mp4".into());
        first.add_bytes(1000);
        second.add_bytes(500);

        let status = monitor.status();
        assert_eq!(
            status.transfers.len(),
            1,
            "mismo cliente y archivo: una fila"
        );
        assert_eq!(status.transfers[0].bytes_sent, 1500);

        drop(first);
        drop(second);
        assert!(monitor.status().transfers.is_empty());
        assert_eq!(monitor.status().bytes_sent_total, 1500);
    }

    #[test]
    fn viewer_follows_player_reports() {
        let monitor = Monitor::new();
        let report = |state| {
            monitor.report_viewer(
                "tv".into(),
                3,
                "Frieren · T1 E1".into(),
                60.0,
                Some(1560.0),
                state,
            )
        };

        assert_eq!(report(PlaybackState::Playing).session, SessionStep::Start);
        assert_eq!(report(PlaybackState::Paused).session, SessionStep::Pending);
        monitor.set_session("tv", 3, 42);
        assert_eq!(
            report(PlaybackState::Paused).session,
            SessionStep::Update(42)
        );
        let viewers = monitor.status().viewers;
        assert_eq!(viewers.len(), 1);
        assert!(!viewers[0].playing);

        report(PlaybackState::Stopped);
        assert!(monitor.status().viewers.is_empty());
    }

    #[test]
    fn formats_clock() {
        assert_eq!(clock(754.2), "12:34");
        assert_eq!(clock(3723.0), "1:02:03");
    }
}
