//! Cuánto ocupa la carpeta media y qué es cada subcarpeta.

use std::fs;
use std::path::Path;

use super::{FolderRole, LibraryRef, MediaFolderUsage, MediaUsage};
use crate::disk::{folder_size, space_of};
use crate::libraries::Library;
use crate::uploads::inbox_dir;

/// Recorre la carpeta media. Es trabajo de disco que bloquea: llamarla con `spawn_blocking`.
pub fn measure(media_dir: &Path, libraries: &[Library]) -> MediaUsage {
    let inbox_name = inbox_dir(media_dir)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut folders: Vec<MediaFolderUsage> = fs::read_dir(media_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let library = libraries.iter().find(|l| l.dir == name);
            let role = if library.is_some() {
                FolderRole::Library
            } else if name == inbox_name {
                FolderRole::Inbox
            } else if name.starts_with('.') {
                return None; // Carpetas ocultas del sistema (.Trashes, .Spotlight…).
            } else {
                FolderRole::Other
            };
            let size = folder_size(&entry.path());
            Some(MediaFolderUsage {
                name,
                size: size.bytes as i64,
                files: size.files as i64,
                role,
                library: library.map(|l| LibraryRef {
                    id: l.id,
                    name: l.name.clone(),
                }),
            })
        })
        .collect();
    folders.sort_by_key(|f| (f.role, f.name.to_lowercase()));

    let missing = libraries
        .iter()
        .filter(|l| !media_dir.join(&l.dir).is_dir())
        .map(|l| LibraryRef {
            id: l.id,
            name: l.name.clone(),
        })
        .collect();

    let total = folder_size(media_dir);
    let disk = space_of(media_dir);
    MediaUsage {
        path: media_dir.to_string_lossy().into_owned(),
        size: total.bytes as i64,
        files: total.files as i64,
        disk_total: disk.as_ref().map(|d| d.total as i64),
        disk_available: disk.as_ref().map(|d| d.available as i64),
        folders,
        missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_folders_to_libraries() {
        let media = std::env::temp_dir().join("kinora-test-usage");
        let _ = fs::remove_dir_all(&media);
        fs::create_dir_all(media.join("Anime/Frieren (2023)")).unwrap();
        fs::create_dir_all(media.join("Viejo")).unwrap();
        fs::create_dir_all(inbox_dir(&media)).unwrap();
        fs::create_dir_all(media.join(".Trashes")).unwrap();
        fs::write(media.join("Anime/Frieren (2023)/e01.mp4"), [0u8; 300]).unwrap();
        fs::write(inbox_dir(&media).join("subido.mkv"), [0u8; 20]).unwrap();

        let libraries = vec![
            Library {
                id: 1,
                name: "Anime".into(),
                dir: "Anime".into(),
            },
            Library {
                id: 2,
                name: "Películas".into(),
                dir: "Películas".into(),
            },
        ];
        let usage = measure(&media, &libraries);

        let rows: Vec<(&str, FolderRole, i64)> = usage
            .folders
            .iter()
            .map(|f| (f.name.as_str(), f.role, f.size))
            .collect();
        assert_eq!(
            rows,
            [
                ("Anime", FolderRole::Library, 300),
                (".kinora-inbox", FolderRole::Inbox, 20),
                ("Viejo", FolderRole::Other, 0),
            ]
        );
        assert_eq!(usage.size, 320);
        assert_eq!(usage.missing.len(), 1);
        assert_eq!(usage.missing[0].name, "Películas");
    }
}
