//! Cambiar la carpeta media cuando ya hay bibliotecas.
//!
//! - "Mover": Kinora renombra cada carpeta de biblioteca (y la bandeja de entrada) a la carpeta
//!   nueva. Solo dentro del mismo disco, donde renombrar es instantáneo. Mover entre discos
//!   implica copiar, y llega con la cola de trabajos (Etapa 4).
//! - "Ya lo moví yo": no toca nada; solo verifica que las carpetas estén en la ubicación nueva.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::uploads::inbox_dir;

/// Comprueba que todas las carpetas de biblioteca estén en `new_media`.
pub fn check_already_moved(new_media: &Path, library_dirs: &[String]) -> AppResult<()> {
    let missing: Vec<&str> = library_dirs
        .iter()
        .filter(|dir| !new_media.join(dir).is_dir())
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(AppError::Conflict(format!(
            "en {} faltan las carpetas de: {}",
            new_media.display(),
            missing.join(", ")
        )));
    }
    Ok(())
}

/// Mueve las carpetas de biblioteca y la bandeja de entrada de `old_media` a `new_media`.
/// Si algo falla a mitad de camino, deshace lo que ya movió.
pub fn move_libraries(
    old_media: &Path,
    new_media: &Path,
    library_dirs: &[String],
) -> AppResult<()> {
    if !old_media.is_dir() {
        return Err(AppError::BadRequest(format!(
            "la carpeta media actual {} no está disponible: no hay nada para mover",
            old_media.display()
        )));
    }
    if !same_disk(old_media, new_media) {
        return Err(AppError::BadRequest(
            "la carpeta nueva está en otro disco. Mover entre discos llega con la cola de trabajos \
             (Etapa 4). Si ya copiaste las carpetas vos, elegí \"Ya lo moví yo\""
                .to_string(),
        ));
    }

    // Qué se mueve: cada biblioteca, y la bandeja si existe.
    let mut moves: Vec<(PathBuf, PathBuf)> = library_dirs
        .iter()
        .map(|dir| (old_media.join(dir), new_media.join(dir)))
        .collect();
    let old_inbox = inbox_dir(old_media);
    if old_inbox.is_dir() {
        moves.push((old_inbox, inbox_dir(new_media)));
    }

    for (from, to) in &moves {
        if new_media.starts_with(from) {
            return Err(AppError::BadRequest(format!(
                "la carpeta nueva está dentro de {}: elegí otra",
                from.display()
            )));
        }
        if to.exists() {
            return Err(AppError::Conflict(format!(
                "ya existe {}: no se puede mover ahí",
                to.display()
            )));
        }
    }

    let mut done: Vec<&(PathBuf, PathBuf)> = Vec::new();
    for pair in &moves {
        let (from, to) = pair;
        // Una biblioteca sin carpeta (nunca se creó o se borró a mano) no tiene nada que mover.
        if !from.exists() {
            continue;
        }
        if let Err(err) = std::fs::rename(from, to) {
            for (from, to) in done.iter().rev() {
                let _ = std::fs::rename(to, from);
            }
            return Err(AppError::BadRequest(format!(
                "no se pudo mover {}: {err}",
                from.display()
            )));
        }
        done.push(pair);
    }
    Ok(())
}

/// Dos rutas están en el mismo disco si su sistema de archivos tiene el mismo identificador.
#[cfg(unix)]
fn same_disk(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (a.metadata(), b.metadata()) {
        (Ok(a), Ok(b)) => a.dev() == b.dev(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_disk(_a: &Path, _b: &Path) -> bool {
    // Fuera de Unix no hay forma simple de saberlo: se intenta, y si renombrar falla, se avisa.
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&root);
        let old = root.join("viejo");
        let new = root.join("nuevo");
        fs::create_dir_all(old.join("Anime/Frieren (2023)")).unwrap();
        fs::create_dir_all(old.join("Series")).unwrap();
        fs::create_dir_all(inbox_dir(&old)).unwrap();
        fs::create_dir_all(&new).unwrap();
        (old, new)
    }

    #[test]
    fn moves_libraries_and_inbox() {
        let (old, new) = setup("kinora-test-relocate");
        let dirs = vec!["Anime".to_string(), "Series".to_string()];
        move_libraries(&old, &new, &dirs).unwrap();
        assert!(new.join("Anime/Frieren (2023)").is_dir());
        assert!(new.join("Series").is_dir());
        assert!(inbox_dir(&new).is_dir());
        assert!(!old.join("Anime").exists());
        check_already_moved(&new, &dirs).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_and_reports_missing_folders() {
        let (old, new) = setup("kinora-test-relocate-conflict");
        fs::create_dir_all(new.join("Series")).unwrap();
        let dirs = vec!["Anime".to_string(), "Series".to_string()];
        assert!(matches!(
            move_libraries(&old, &new, &dirs),
            Err(AppError::Conflict(_))
        ));
        // No se movió nada.
        assert!(old.join("Anime").is_dir());

        let error = check_already_moved(&new, &dirs).unwrap_err();
        assert!(matches!(error, AppError::Conflict(message) if message.contains("Anime")));
    }

    #[test]
    fn refuses_a_destination_inside_a_library() {
        let (old, _) = setup("kinora-test-relocate-inside");
        let inside = old.join("Anime/Frieren (2023)");
        let dirs = vec!["Anime".to_string()];
        assert!(move_libraries(&old, &inside, &dirs).is_err());
    }
}
