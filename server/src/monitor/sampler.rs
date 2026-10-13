//! Tarea en segundo plano que mide CPU y memoria cada 2 segundos
//! y escribe un resumen en la terminal.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tracing::{info, warn};

use super::history::{self, MinuteTotals};
use super::{Monitor, Sample, clock};
use crate::db::now;

const SAMPLE_EVERY: Duration = Duration::from_secs(2);
/// Con actividad (streams o la cola) se informa seguido; sin actividad, cada tanto.
const LOG_EVERY_ACTIVE: Duration = Duration::from_secs(10);
const LOG_EVERY_IDLE: Duration = Duration::from_secs(60);

pub fn spawn_sampler(monitor: Arc<Monitor>, db: SqlitePool) {
    tokio::spawn(async move {
        let mut system = System::new();
        let pid = sysinfo::get_current_pid().ok();
        let mut interval = tokio::time::interval(SAMPLE_EVERY);
        let mut last_bytes = 0;
        let mut last_log = Instant::now();
        let mut minute = MinuteTotals::default();

        loop {
            interval.tick().await;

            // La CPU se calcula comparando con la medición anterior: por eso se mide en intervalos.
            // Primero se actualiza todo (préstamos mutables de `system`) y recién después se lee
            // (préstamos compartidos). Rust no permite mezclarlos al mismo tiempo.
            system.refresh_cpu_usage();
            system.refresh_memory();
            let worker = monitor.worker_pid().map(Pid::from_u32);
            let pids: Vec<Pid> = pid.into_iter().chain(worker).collect();
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&pids),
                true,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );
            let process = pid.and_then(|pid| system.process(pid));
            let ffmpeg = worker.and_then(|pid| system.process(pid));

            let bytes = monitor.bytes_sent.load(Ordering::Relaxed);
            let sample = Sample {
                cpu_kinora: process.map_or(0.0, |p| p.cpu_usage()),
                cpu_system: system.global_cpu_usage(),
                cpu_cores: system.cpus().len() as i64,
                memory_kinora: process.map_or(0, |p| p.memory() as i64),
                cpu_ffmpeg: ffmpeg.map(|p| p.cpu_usage()),
                memory_ffmpeg: ffmpeg.map(|p| p.memory() as i64),
                memory_used: system.used_memory() as i64,
                memory_total: system.total_memory() as i64,
                send_rate: ((bytes - last_bytes) as f64 / SAMPLE_EVERY.as_secs_f64()) as i64,
            };
            let sent = bytes - last_bytes;
            last_bytes = bytes;
            *monitor.latest.write().unwrap() = sample.clone();

            monitor.expire_viewers();
            let viewers = monitor.viewer_list();

            // Cada minuto se guarda el resumen en el historial.
            if let Some(row) = minute.add(now(), &sample, sent as i64, viewers.len())
                && let Err(err) = history::save(&db, &row).await
            {
                warn!("could not save the server history: {err}");
            }
            // Con alguien mirando o la cola trabajando se informa seguido.
            let log_every = if viewers.is_empty() && sample.cpu_ffmpeg.is_none() {
                LOG_EVERY_IDLE
            } else {
                LOG_EVERY_ACTIVE
            };
            if last_log.elapsed() >= log_every {
                last_log = Instant::now();
                let watching: Vec<String> = viewers
                    .iter()
                    .map(|v| {
                        let state = if v.playing { "▶" } else { "⏸" };
                        let total = v.duration.map(clock).unwrap_or_else(|| "?".to_string());
                        format!(
                            "{state} {} → {} {}/{total}",
                            v.client,
                            v.title,
                            clock(v.position)
                        )
                    })
                    .collect();
                let ffmpeg = sample
                    .cpu_ffmpeg
                    .map(|cpu| format!(" · CPU ffmpeg {cpu:.0}%"))
                    .unwrap_or_default();
                info!(
                    "CPU kinora {:.1}%{ffmpeg} · CPU total {:.0}% · memory kinora {} · sending {}/s · watching: {}",
                    sample.cpu_kinora,
                    sample.cpu_system,
                    megabytes(sample.memory_kinora),
                    megabytes(sample.send_rate),
                    if watching.is_empty() {
                        "nobody".to_string()
                    } else {
                        watching.join(", ")
                    }
                );
            }
        }
    });
}

fn megabytes(bytes: i64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}
