//! Mover las bibliotecas a otro disco: copiar, verificar y recién ahí cambiar.
//!
//! Entre discos no se puede renombrar: hay que copiar gigas. Para que nada se pierda:
//! 1. Se copia todo a carpetas temporales en el disco nuevo (`Anime.kinora-moving`), archivo por
//!    archivo, comprobando que cada copia tenga el tamaño del original.
//! 2. Solo cuando terminó todo, las temporales toman su nombre (renombrar en el mismo disco es
//!    instantáneo), se cambia la carpeta media y se borran las originales.
//! 3. Si algo falla o se cancela, se borran las copias a medias y todo queda como estaba.
//!
//! Mientras dura se puede seguir mirando (se lee del disco viejo), pero el Admin no deja hacer
//! cambios que escriban en las bibliotecas, y la cola no toma trabajos.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tracing::{info, warn};
use ts_rs::TS;

use super::repo;
use crate::app::AppState;
use crate::db::now;
use crate::uploads::inbox_dir;

/// Sufijo de las carpetas que se están copiando.
const MOVING: &str = ".kinora-moving";
/// Clave en `settings`: a dónde se estaba copiando, para limpiar si el servidor se cortó.
const TARGET_KEY: &str = "relocation_target";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TransferState {
    Idle,
    Copying,
    /// Copiado: cambiando nombres y borrando lo viejo.
    Finishing,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct TransferStatus {
    pub state: TransferState,
    pub from: Option<String>,
    pub to: Option<String>,
    pub total_bytes: i64,
    pub copied_bytes: i64,
    /// El archivo que se está copiando, relativo a la carpeta media.
    pub current: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

pub struct Transfer {
    status: Mutex<TransferStatus>,
    cancel: AtomicBool,
}

impl Transfer {
    pub fn new() -> Transfer {
        Transfer {
            status: Mutex::new(TransferStatus {
                state: TransferState::Idle,
                from: None,
                to: None,
                total_bytes: 0,
                copied_bytes: 0,
                current: None,
                error: None,
                started_at: None,
                finished_at: None,
            }),
            cancel: AtomicBool::new(false),
        }
    }

    pub fn status(&self) -> TransferStatus {
        self.status.lock().unwrap().clone()
    }

    /// Si hay una copia en marcha (y por lo tanto no se debe escribir en las bibliotecas).
    pub fn is_busy(&self) -> bool {
        matches!(
            self.status.lock().unwrap().state,
            TransferState::Copying | TransferState::Finishing
        )
    }

    pub fn request_cancel(&self) -> bool {
        let copying = self.status.lock().unwrap().state == TransferState::Copying;
        if copying {
            self.cancel.store(true, Ordering::Relaxed);
        }
        copying
    }

    fn update(&self, change: impl FnOnce(&mut TransferStatus)) {
        change(&mut self.status.lock().unwrap());
    }
}

/// Lo que se copia: carpeta original → carpeta temporal en el disco nuevo → nombre final.
struct Move {
    from: PathBuf,
    temp: PathBuf,
    to: PathBuf,
}

/// Empieza a copiar en segundo plano. Las comprobaciones (destino, lugar libre) ya están hechas.
pub fn start(state: AppState, old_media: PathBuf, new_media: PathBuf, library_dirs: Vec<String>) {
    let transfer = &state.settings.transfer;
    transfer.cancel.store(false, Ordering::Relaxed);
    transfer.update(|s| {
        *s = TransferStatus {
            state: TransferState::Copying,
            from: Some(old_media.to_string_lossy().into_owned()),
            to: Some(new_media.to_string_lossy().into_owned()),
            total_bytes: 0,
            copied_bytes: 0,
            current: None,
            error: None,
            started_at: Some(now()),
            finished_at: None,
        }
    });
    tokio::spawn(async move {
        let result = run(&state, &old_media, &new_media, &library_dirs).await;
        let transfer = &state.settings.transfer;
        let cancelled = transfer.cancel.load(Ordering::Relaxed);
        if let Err(err) = repo::set(&state.db, TARGET_KEY, "").await {
            warn!("could not clear {TARGET_KEY}: {err}");
        }
        transfer.update(|s| {
            s.finished_at = Some(now());
            s.current = None;
            match &result {
                Ok(()) => s.state = TransferState::Done,
                Err(_) if cancelled => s.state = TransferState::Cancelled,
                Err(err) => {
                    s.state = TransferState::Failed;
                    s.error = Some(err.to_string());
                }
            }
        });
        match result {
            Ok(()) => info!("libraries moved to {}", new_media.display()),
            Err(_) if cancelled => info!("moving the libraries was cancelled"),
            Err(err) => warn!("could not move the libraries: {err}"),
        }
    });
}

async fn run(
    state: &AppState,
    old_media: &Path,
    new_media: &Path,
    library_dirs: &[String],
) -> io::Result<()> {
    repo::set(&state.db, TARGET_KEY, &new_media.to_string_lossy())
        .await
        .map_err(io::Error::other)?;

    let mut moves: Vec<Move> = library_dirs
        .iter()
        .map(|dir| Move {
            from: old_media.join(dir),
            temp: new_media.join(format!("{dir}{MOVING}")),
            to: new_media.join(dir),
        })
        .collect();
    let inbox = inbox_dir(old_media);
    moves.push(Move {
        temp: new_media.join(format!(".kinora-inbox{MOVING}")),
        to: inbox_dir(new_media),
        from: inbox,
    });
    moves.retain(|m| m.from.is_dir());

    // 1. Copiar (en un hilo aparte: son operaciones de disco que bloquean).
    let copy = {
        let state = state.clone();
        let old_media = old_media.to_path_buf();
        let pairs: Vec<(PathBuf, PathBuf)> = moves
            .iter()
            .map(|m| (m.from.clone(), m.temp.clone()))
            .collect();
        tokio::task::spawn_blocking(move || {
            let transfer = &state.settings.transfer;
            let total: u64 = pairs.iter().map(|(from, _)| tree_size(from)).sum();
            transfer.update(|s| s.total_bytes = total as i64);
            for (from, temp) in &pairs {
                copy_tree(from, temp, &old_media, transfer)?;
            }
            Ok::<(), io::Error>(())
        })
    };
    if let Err(err) = copy.await.map_err(io::Error::other).and_then(|r| r) {
        remove_temps(&moves);
        return Err(err);
    }

    // 2. Las copias toman su nombre y se cambia la carpeta media.
    state
        .settings
        .transfer
        .update(|s| s.state = TransferState::Finishing);
    let mut renamed: Vec<&Move> = Vec::new();
    for m in &moves {
        if let Err(err) = fs::rename(&m.temp, &m.to) {
            for done in renamed.iter().rev() {
                let _ = fs::rename(&done.to, &done.temp);
            }
            remove_temps(&moves);
            return Err(err);
        }
        renamed.push(m);
    }
    if let Err(err) = state
        .settings
        .set_media_dir(&state.db, new_media.to_path_buf())
        .await
    {
        // No se pudo cambiar: la carpeta media sigue siendo la vieja, intacta. Se borran las copias.
        for m in &moves {
            let _ = fs::remove_dir_all(&m.to);
        }
        return Err(io::Error::other(err));
    }

    // 3. Lo viejo ya no se usa: se borra.
    for m in &moves {
        if let Err(err) = fs::remove_dir_all(&m.from) {
            warn!("could not delete the old copy {}: {err}", m.from.display());
        }
    }
    Ok(())
}

fn remove_temps(moves: &[Move]) {
    for m in moves {
        if m.temp.exists()
            && let Err(err) = fs::remove_dir_all(&m.temp)
        {
            warn!(
                "could not delete the partial copy {}: {err}",
                m.temp.display()
            );
        }
    }
}

/// Si el servidor se cortó en el medio de una copia, al arrancar se borran las copias a medias.
pub async fn clean_up_interrupted(db: &sqlx::SqlitePool) {
    let target = match repo::get(db, TARGET_KEY).await {
        Ok(Some(target)) if !target.is_empty() => PathBuf::from(target),
        _ => return,
    };
    if let Ok(entries) = fs::read_dir(&target) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(MOVING) {
                warn!(
                    "deleting {}: a copy to another disk was interrupted",
                    entry.path().display()
                );
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    let _ = repo::set(db, TARGET_KEY, "").await;
}

/// Bytes de todos los archivos de una carpeta (incluidos los ocultos: se copia todo).
pub fn tree_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => tree_size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

fn copy_tree(from: &Path, to: &Path, media: &Path, transfer: &Transfer) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target, media, transfer)?;
        } else if kind.is_file() {
            let path = entry.path();
            let shown = path
                .strip_prefix(media)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            transfer.update(|s| s.current = Some(shown));
            copy_file(&path, &target, transfer)?;
        }
        // Los enlaces simbólicos no se copian: Kinora no los crea.
    }
    Ok(())
}

/// Copia de a 1 MB, sumando el avance y mirando si se pidió cancelar.
/// Al final compara tamaños: una copia incompleta es un error, no un archivo más chico.
fn copy_file(from: &Path, to: &Path, transfer: &Transfer) -> io::Result<()> {
    let mut source = fs::File::open(from)?;
    let mut target = fs::File::create(to)?;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        if transfer.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado"));
        }
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        target.write_all(&buffer[..read])?;
        transfer.update(|s| s.copied_bytes += read as i64);
    }
    target.sync_all()?;
    let (expected, copied) = (fs::metadata(from)?.len(), fs::metadata(to)?.len());
    if expected != copied {
        return Err(io::Error::other(format!(
            "la copia de {} quedó incompleta ({copied} de {expected} bytes)",
            from.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_everything_and_counts_progress() {
        let root = std::env::temp_dir().join("kinora-test-transfer");
        let _ = fs::remove_dir_all(&root);
        let from = root.join("viejo/Anime");
        fs::create_dir_all(from.join("Frieren/Season 01")).unwrap();
        fs::create_dir_all(from.join(".kinora-trash/x")).unwrap();
        fs::write(from.join("Frieren/Season 01/E01.mp4"), vec![7u8; 3_000_000]).unwrap();
        fs::write(from.join("Frieren/kinora.json"), b"{}").unwrap();
        fs::write(from.join(".kinora-trash/x/old.mp4"), b"old").unwrap();

        let transfer = Transfer::new();
        let to = root.join("nuevo/Anime.kinora-moving");
        assert_eq!(tree_size(&from), 3_000_005);
        copy_tree(&from, &to, &root.join("viejo"), &transfer).unwrap();
        assert_eq!(
            fs::read(to.join("Frieren/Season 01/E01.mp4"))
                .unwrap()
                .len(),
            3_000_000
        );
        assert!(to.join(".kinora-trash/x/old.mp4").is_file());
        assert_eq!(transfer.status().copied_bytes, 3_000_005);

        // Cancelar corta en el próximo bloque.
        transfer.cancel.store(true, Ordering::Relaxed);
        let error = copy_tree(&from, &root.join("otra"), &root, &transfer).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    }
}
