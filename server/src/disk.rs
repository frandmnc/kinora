//! Espacio en disco y tamaño de carpetas.

use std::fs;
use std::path::Path;

pub struct DiskSpace {
    pub total: u64,
    pub available: u64,
}

/// El disco que contiene `path`: el de punto de montaje más largo que sea prefijo de la ruta
/// (por ejemplo, para `/Volumes/Disco/Kinora`, gana `/Volumes/Disco` sobre `/`).
pub fn space_of(path: &Path) -> Option<DiskSpace> {
    let path = path.canonicalize().ok()?;
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| DiskSpace {
            total: disk.total_space(),
            available: disk.available_space(),
        })
}

/// "1.4 GB", "820 MB": para mensajes.
pub fn format_size(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else {
        format!("{:.0} MB", bytes / 1024.0 / 1024.0)
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct FolderSize {
    pub bytes: u64,
    pub files: u64,
}

/// Suma el tamaño de los archivos de una carpeta y sus subcarpetas.
///
/// - Ignora archivos ocultos: `.DS_Store` (el Finder crea uno en cada carpeta que se abre)
///   y los `._algo` que macOS deja en discos externos.
/// - Entra en las carpetas ocultas de Kinora (`.kinora-tmp`, `.kinora-inbox`…), que pueden
///   tener gigas, pero no en otras ocultas del sistema (`.Trashes`, `.Spotlight-V100`).
/// - No sigue enlaces simbólicos: podrían apuntar fuera de la carpeta o formar un ciclo.
pub fn folder_size(dir: &Path) -> FolderSize {
    let mut total = FolderSize::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return total;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(metadata) = entry.path().symlink_metadata() else {
            continue;
        };
        if metadata.is_dir() {
            if name.starts_with('.') && !name.starts_with(".kinora") {
                continue;
            }
            let inner = folder_size(&entry.path());
            total.bytes += inner.bytes;
            total.files += inner.files;
        } else if metadata.is_file() && !name.starts_with('.') {
            total.bytes += metadata.len();
            total.files += 1;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_files_in_subfolders() {
        let root = std::env::temp_dir().join("kinora-test-size");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("uno.mp4"), [0u8; 100]).unwrap();
        fs::write(root.join("a/b/dos.mp4"), [0u8; 50]).unwrap();
        // Lo que no cuenta: archivos ocultos y carpetas ocultas del sistema.
        fs::write(root.join(".DS_Store"), [0u8; 6000]).unwrap();
        fs::write(root.join("a/._dos.mp4"), [0u8; 4000]).unwrap();
        fs::create_dir_all(root.join(".Spotlight-V100")).unwrap();
        fs::write(root.join(".Spotlight-V100/indice"), [0u8; 9000]).unwrap();
        // Lo que sí: las carpetas ocultas de Kinora.
        fs::create_dir_all(root.join(".kinora-tmp")).unwrap();
        fs::write(root.join(".kinora-tmp/7.mp4.part"), [0u8; 25]).unwrap();
        assert_eq!(
            folder_size(&root),
            FolderSize {
                bytes: 175,
                files: 3
            }
        );
    }

    #[test]
    fn knows_the_disk_of_a_path() {
        let space = space_of(&std::env::temp_dir()).unwrap();
        assert!(space.total >= space.available);
    }
}
