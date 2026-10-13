//! El trabajador de la cola: toma el pendiente más viejo, lo procesa y sigue con el próximo.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::oneshot;
use tracing::{info, warn};

use super::{Job, Running, repo};
use crate::app::AppState;
use crate::contents;
use crate::imports::paths::without_version;
use crate::imports::{ImportAction, PlaceFile, free_version, place};
use crate::monitor::Monitor;
use crate::probe::{self, Operation, Probe};
use crate::{manifest, subtitles, trash, uploads};

/// Arranca el trabajador. Antes, los trabajos que quedaron a medias por un reinicio vuelven a
/// la fila (su resultado parcial se borra).
pub fn spawn_worker(state: AppState) {
    tokio::spawn(async move {
        recover(&state).await;
        loop {
            // Detenida: espera a que la inicien (o cada tanto mira igual, por las dudas).
            // Mientras se copian las bibliotecas a otro disco tampoco: lo que se escribiera se perdería.
            if !state.jobs.is_started() || state.settings.transfer.is_busy() {
                wait(&state).await;
                continue;
            }
            match repo::next_pending(&state.db).await {
                Ok(Some(job)) => run(&state, job).await,
                Ok(None) => {
                    // Se vació: se detiene sola. Lo próximo que se importe espera a que la inicien.
                    match state.jobs.set_started(&state.db, false).await {
                        Ok(()) => info!("queue finished: nothing left to do"),
                        Err(err) => warn!("could not stop the queue: {err}"),
                    }
                }
                Err(err) => {
                    warn!("could not read the import queue: {err}");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    });
}

async fn wait(state: &AppState) {
    tokio::select! {
        _ = state.jobs.wake.notified() => {}
        _ = tokio::time::sleep(Duration::from_secs(60)) => {}
    }
}

async fn recover(state: &AppState) {
    let jobs = match repo::interrupted(&state.db).await {
        Ok(jobs) => jobs,
        Err(err) => return warn!("could not read the import queue: {err}"),
    };
    for job in jobs {
        if let Ok(paths) = JobPaths::of(state, &job).await {
            kill_strays(&paths.output);
            let _ = tokio::fs::remove_file(&paths.output).await;
        }
        match repo::requeue(&state.db, job.id).await {
            Ok(()) => info!("job {} was interrupted by a restart: queued again", job.id),
            Err(err) => warn!("could not requeue job {}: {err}", job.id),
        }
    }
}

/// Si Kinora murió sin poder avisar (`kill -9`, un corte de luz no: ahí muere todo), su ffmpeg
/// puede seguir vivo escribiendo el resultado. Se lo busca por la ruta de salida y se lo corta.
fn kill_strays(output: &Path) {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    for process in system.processes().values() {
        if process.cmd().iter().any(|arg| Path::new(arg) == output) {
            warn!(
                "killing a leftover ffmpeg (pid {}) from a previous run",
                process.pid()
            );
            process.kill();
        }
    }
}

/// Al apagar el servidor: corta el ffmpeg en curso, si hay. El trabajo queda "corriendo" en la
/// base y al volver a arrancar se reintenta desde cero.
pub fn stop_running(monitor: &Monitor) {
    let Some(pid) = monitor.worker_pid() else {
        return;
    };
    let mut system = System::new();
    let pid = Pid::from_u32(pid);
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    if let Some(process) = system.process(pid) {
        process.kill();
        info!("stopped the running ffmpeg (pid {pid})");
    }
}

/// Dónde están los archivos de un trabajo.
pub(super) struct JobPaths {
    media_dir: PathBuf,
    /// La carpeta de la biblioteca.
    root: PathBuf,
    /// El original, esperando en `.kinora-tmp/`.
    pub source: PathBuf,
    /// El resultado mientras se escribe.
    output: PathBuf,
}

impl JobPaths {
    pub async fn of(state: &AppState, job: &Job) -> Result<JobPaths> {
        let media_dir = state
            .settings
            .media_dir()
            .context("no hay carpeta media configurada")?;
        let library_dir = contents::repo::library_dir(&state.db, job.library_id).await?;
        let root = media_dir.join(library_dir);
        let tmp = root.join(".kinora-tmp");
        Ok(JobPaths {
            source: tmp.join(source_file_name(job.id, &job.source_name)),
            output: tmp.join(format!("job-{}.mp4", job.id)),
            media_dir,
            root,
        })
    }
}

/// `job-12-source.mkv`: conserva la extensión, que ffmpeg usa para reconocer el formato.
pub fn source_file_name(id: i64, source_name: &str) -> String {
    match Path::new(source_name).extension() {
        Some(ext) => format!("job-{id}-source.{}", ext.to_string_lossy()),
        None => format!("job-{id}-source"),
    }
}

async fn run(state: &AppState, job: Job) {
    match repo::start(&state.db, job.id).await {
        Ok(true) => {}
        Ok(false) => return,
        Err(err) => return warn!("could not start job {}: {err}", job.id),
    }
    info!(
        "job {} started: {:?} {}",
        job.id, job.operation, job.source_name
    );
    let (cancel, cancelled) = oneshot::channel();
    *state.jobs.running.lock().unwrap() = Some(Running { id: job.id, cancel });
    let started = Instant::now();
    let outcome = process(state, &job, cancelled).await;
    state.jobs.running.lock().unwrap().take();

    let result = match outcome {
        Ok(Some((rel_path, size))) => {
            info!(
                "job {} done in {:.0}s: {rel_path}",
                job.id,
                started.elapsed().as_secs_f64()
            );
            repo::finish(&state.db, job.id, &rel_path, size).await
        }
        Ok(None) => {
            info!("job {} cancelled", job.id);
            give_back(state, &job).await;
            repo::cancel(&state.db, job.id).await
        }
        Err(err) => {
            warn!("job {} failed: {err:#}", job.id);
            give_back(state, &job).await;
            repo::fail(&state.db, job.id, &format!("{err:#}")).await
        }
    };
    if let Err(err) = result {
        warn!("could not save the result of job {}: {err}", job.id);
    }
}

/// Procesa el trabajo. Devuelve la ruta final y el tamaño, o `None` si se canceló.
async fn process(
    state: &AppState,
    job: &Job,
    cancelled: oneshot::Receiver<()>,
) -> Result<Option<(String, i64)>> {
    let paths = JobPaths::of(state, job).await?;
    // Eliminar un contenido con trabajos pendientes no se permite, así que esto no debería pasar.
    let content_id = job
        .content_id
        .context("el contenido de este trabajo ya no existe")?;
    let source = probe::run(&paths.source)
        .await
        .context("no se pudo leer el original")?;
    let duration = source.probe.media_duration();

    if let Some(duration) = duration {
        repo::set_duration(&state.db, job.id, duration).await?;
    }

    // 1. Remux o conversión a `.kinora-tmp/job-<id>.mp4`.
    let args = ffmpeg_args(job.operation, &paths.source, &paths.output, &source.probe);
    if !ffmpeg(state, job.id, &args, duration, cancelled).await? {
        let _ = tokio::fs::remove_file(&paths.output).await;
        return Ok(None);
    }

    // 2. Verificar que el resultado cumple el formato y dura lo mismo que el original.
    let result = async {
        let result = probe::run(&paths.output)
            .await
            .context("no se pudo leer el resultado")?;
        let plan = result.probe.plan(&paths.output);
        if plan.operation != Operation::Copy || !plan.blockers.is_empty() {
            let reasons: Vec<String> = plan.changes.into_iter().chain(plan.blockers).collect();
            bail!("el resultado no cumple el formato: {}", reasons.join("; "));
        }
        // Busca archivos cortados (ffmpeg que muere a mitad de camino, disco lleno): ahí la
        // diferencia es de minutos. Los MKV a veces empiezan unos instantes después de 0 y lo
        // cuentan en su duración, por eso el margen no es más fino.
        if let (Some(before), Some(after)) = (duration, result.probe.media_duration())
            && (before - after).abs() > (before * 0.01).max(2.0)
        {
            bail!("el resultado dura {after:.1} s y el original {before:.1} s");
        }
        Ok(result)
    }
    .await;
    let result = match result {
        Ok(result) => result,
        Err(err) => {
            let _ = tokio::fs::remove_file(&paths.output).await;
            return Err(err);
        }
    };

    // 3. Ponerlo en su lugar. Qué reemplaza se decide ahora: es lo que tiene el episodio hoy.
    let mut conn = state.db.acquire().await?;
    let current = contents::repo::file_ids_of(&mut conn, content_id).await?;
    let (rel_path, replaces) = match job.action {
        ImportAction::Replaced => (job.rel_path.clone(), current),
        // La ruta `… - v2.mp4` se eligió al confirmar. Si mientras tanto se ocupó (otra importación
        // en el medio), se busca la próxima libre a partir de la ruta sin versión.
        ImportAction::AddedVersion => {
            let taken = paths.root.join(&job.rel_path).exists()
                || contents::repo::rel_path_taken(&mut conn, &job.rel_path).await?;
            let rel_path = if taken {
                free_version(&mut conn, &paths.root, &without_version(&job.rel_path)).await?
            } else {
                job.rel_path.clone()
            };
            (rel_path, Vec::new())
        }
        _ => (job.rel_path.clone(), Vec::new()),
    };
    drop(conn);
    let placed = place(
        state,
        &paths.root,
        &trash::batch_dir(&paths.root),
        PlaceFile {
            content_id,
            rel_path: &rel_path,
            file: &paths.output,
            analysis: &result,
            source_name: &job.source_name,
            replaces: &replaces,
        },
    )
    .await;
    let file_id = match placed {
        Ok((id, _)) => id,
        Err(err) => {
            let _ = tokio::fs::remove_file(&paths.output).await;
            return Err(err);
        }
    };

    // 4. Los subtítulos salen del original: el MP4 no lleva los del MKV.
    let tracks = subtitles::find_tracks(&source.probe);
    subtitles::attach_subtitles(
        &state.db,
        &paths.root,
        file_id,
        &rel_path,
        &paths.source,
        tracks,
    )
    .await;
    if let Err(err) = tokio::fs::remove_file(&paths.source).await {
        warn!("could not delete {}: {err}", paths.source.display());
    }
    manifest::refresh_for_file(&state.db, &paths.media_dir, file_id).await;

    let size = tokio::fs::metadata(paths.root.join(&rel_path)).await?.len() as i64;
    Ok(Some((rel_path, size)))
}

/// Si falla o se cancela, el original vuelve a la bandeja de entrada.
pub(super) async fn give_back(state: &AppState, job: &Job) {
    let result = async {
        let paths = JobPaths::of(state, job).await?;
        let inbox = uploads::inbox_dir(&paths.media_dir);
        tokio::fs::create_dir_all(&inbox).await?;
        let destination = uploads::unique_path(&inbox, &job.source_name);
        tokio::fs::rename(&paths.source, &destination).await?;
        anyhow::Ok(())
    }
    .await;
    if let Err(err) = result {
        warn!("could not return {} to the inbox: {err:#}", job.source_name);
    }
}

/// Calidad de la conversión (ver "Formato de Kinora" en el plan). CRF: menos es mejor calidad
/// y archivos más grandes; 22 se ve igual que el original en una TV de 43". El preset es el
/// equilibrio entre velocidad y tamaño: "medium" es el de referencia de x265.
const VIDEO_CRF: &str = "22";
const VIDEO_PRESET: &str = "medium";

/// Los argumentos de ffmpeg para llevar el archivo a MP4 + HEVC + AAC.
/// - Remux y audio: el video se copia tal cual.
/// - Video: se convierte con x265 a 10 bits (Etapa 0), conservando el espacio de color SDR.
///
/// En todos los casos el audio AAC se copia y el resto se convierte a AAC con los mismos
/// canales (hasta 5.1).
fn ffmpeg_args(operation: Operation, source: &Path, output: &Path, probe: &Probe) -> Vec<String> {
    let mut args: Vec<String> = ["-v", "error", "-nostdin", "-y", "-i"]
        .map(String::from)
        .into();
    args.push(source.to_string_lossy().into_owned());
    // `V` mayúscula: el primer video real, sin carátulas incrustadas. Todo el audio.
    args.extend(["-map", "0:V:0", "-map", "0:a"].map(String::from));

    if operation == Operation::Video {
        args.extend(
            [
                "-c:v",
                "libx265",
                "-preset",
                VIDEO_PRESET,
                "-crf",
                VIDEO_CRF,
                "-pix_fmt",
                "yuv420p10le",
                "-profile:v",
                "main10",
                "-x265-params",
                "log-level=error",
            ]
            .map(String::from),
        );
        // SDR: se conserva cómo estaba etiquetado. Si no decía nada, se asume lo habitual:
        // BT.709 en HD, BT.601 en definición estándar (DVD).
        let video = probe.video();
        let hd = video.and_then(|v| v.height).unwrap_or(1080) >= 720;
        let tag = |value: Option<&String>, hd_default: &str, sd_default: &str| {
            value
                .filter(|v| !v.is_empty() && *v != "unknown" && *v != "reserved")
                .cloned()
                .unwrap_or_else(|| if hd { hd_default } else { sd_default }.to_string())
        };
        args.extend([
            "-color_primaries".into(),
            tag(
                video.and_then(|v| v.color_primaries.as_ref()),
                "bt709",
                "smpte170m",
            ),
            "-color_trc".into(),
            tag(
                video.and_then(|v| v.color_transfer.as_ref()),
                "bt709",
                "smpte170m",
            ),
            "-colorspace".into(),
            tag(
                video.and_then(|v| v.color_space.as_ref()),
                "bt709",
                "smpte170m",
            ),
        ]);
    } else {
        args.extend(["-c:v", "copy"].map(String::from));
    }
    args.extend(["-tag:v", "hvc1"].map(String::from));

    for (i, audio) in probe.audios().enumerate() {
        if audio.codec_name.as_deref() == Some("aac") {
            args.extend([format!("-c:a:{i}"), "copy".into()]);
        } else {
            let channels = audio.channels.unwrap_or(2).min(6);
            args.extend([format!("-c:a:{i}"), "aac".into()]);
            args.extend([
                format!("-b:a:{i}"),
                format!("{}k", (channels * 64).max(128)),
            ]);
            args.extend([format!("-ac:a:{i}"), channels.to_string()]);
        }
    }
    // faststart: el índice al principio, para que el video arranque sin bajar todo el archivo.
    args.extend(["-movflags", "+faststart", "-progress", "pipe:1", "-nostats"].map(String::from));
    args.push(output.to_string_lossy().into_owned());
    args
}

/// Ejecuta ffmpeg y va guardando el avance (lo que ya escribió / la duración). Devuelve
/// `false` si se canceló: en ese caso corta ffmpeg en el momento.
async fn ffmpeg(
    state: &AppState,
    id: i64,
    args: &[String],
    duration: Option<f64>,
    mut cancelled: oneshot::Receiver<()>,
) -> Result<bool> {
    // Con prioridad baja: la conversión usa todos los núcleos, y ver algo tiene que ir primero.
    let mut child = tokio::process::Command::new("nice")
        .args(["-n", "10", "ffmpeg"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("no se pudo ejecutar ffmpeg (¿está instalado?)")?;
    // `nice` se reemplaza por ffmpeg (no queda como proceso padre): el pid es el de ffmpeg.
    let _pid = WorkerPid::set(&state.monitor, child.id());
    let mut progress = BufReader::new(child.stdout.take().expect("stdout")).lines();
    let mut stderr = child.stderr.take().expect("stderr");
    let errors = tokio::spawn(async move {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text).await;
        text
    });

    let mut saved = Instant::now();
    loop {
        tokio::select! {
            line = progress.next_line() => match line? {
                // `-progress` escribe líneas "clave=valor"; out_time_us es lo ya escrito.
                Some(line) => {
                    let written = line.strip_prefix("out_time_us=").and_then(|v| v.parse::<f64>().ok());
                    if let (Some(us), Some(total)) = (written, duration)
                        && total > 0.0
                        && saved.elapsed() >= Duration::from_secs(1)
                    {
                        let share = (us / 1_000_000.0 / total).clamp(0.0, 0.99);
                        let _ = repo::set_progress(&state.db, id, share).await;
                        saved = Instant::now();
                    }
                }
                None => break,
            },
            _ = &mut cancelled => {
                let _ = child.kill().await;
                return Ok(false);
            }
        }
    }
    let status = child.wait().await?;
    if !status.success() {
        bail!("ffmpeg falló: {}", errors.await.unwrap_or_default().trim());
    }
    Ok(true)
}

/// Mientras exista, el monitor mide CPU y memoria de ffmpeg; al soltarse, deja de medirlo.
struct WorkerPid<'a>(&'a Monitor);

impl<'a> WorkerPid<'a> {
    fn set(monitor: &'a Monitor, pid: Option<u32>) -> WorkerPid<'a> {
        monitor.set_worker_pid(pid);
        WorkerPid(monitor)
    }
}

impl Drop for WorkerPid<'_> {
    fn drop(&mut self) {
        self.0.set_worker_pid(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_aac_and_converts_other_audio() {
        let probe = Probe::parse(
            r#"{
                "format": { "format_name": "matroska,webm" },
                "streams": [
                    { "codec_type": "video", "codec_name": "hevc" },
                    { "codec_type": "audio", "codec_name": "aac", "channels": 2 },
                    { "codec_type": "audio", "codec_name": "dts", "channels": 8 }
                ]
            }"#,
        )
        .unwrap();
        let args = ffmpeg_args(
            Operation::Audio,
            Path::new("in.mkv"),
            Path::new("out.mp4"),
            &probe,
        )
        .join(" ");
        assert!(args.contains("-map 0:V:0 -map 0:a -c:v copy -tag:v hvc1"));
        assert!(args.contains("-c:a:0 copy"));
        assert!(args.contains("-c:a:1 aac -b:a:1 384k -ac:a:1 6"));
        assert!(args.ends_with("-movflags +faststart -progress pipe:1 -nostats out.mp4"));
        assert_eq!(source_file_name(7, "Frieren - 01.mkv"), "job-7-source.mkv");
    }

    #[test]
    fn converts_video_to_10_bit_hevc_keeping_sdr_tags() {
        let probe = Probe::parse(
            r#"{
                "format": { "format_name": "matroska,webm" },
                "streams": [
                    { "codec_type": "video", "codec_name": "h264", "height": 1080,
                      "color_primaries": "bt709" },
                    { "codec_type": "audio", "codec_name": "aac", "channels": 2 }
                ]
            }"#,
        )
        .unwrap();
        let args = ffmpeg_args(
            Operation::Video,
            Path::new("in.mkv"),
            Path::new("out.mp4"),
            &probe,
        )
        .join(" ");
        assert!(args.contains("-c:v libx265 -preset medium -crf 22 -pix_fmt yuv420p10le"));
        assert!(args.contains("-color_primaries bt709 -color_trc bt709 -colorspace bt709"));
        assert!(args.contains("-tag:v hvc1 -c:a:0 copy"));
    }
}
