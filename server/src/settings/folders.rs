//! Explorador de carpetas del servidor. Solo lista carpetas (nunca archivos) y oculta
//! las que empiezan con ".".

use std::fs;
use std::path::{Path, PathBuf};

use super::Folder;

/// Las subcarpetas visibles de `dir`, ordenadas por nombre.
pub fn subfolders(dir: &Path) -> std::io::Result<Vec<Folder>> {
    let mut folders: Vec<Folder> = fs::read_dir(dir)?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (!name.starts_with('.')).then(|| Folder {
                path: entry.path().to_string_lossy().into_owned(),
                name,
            })
        })
        .collect();
    folders.sort_by_key(|f| f.name.to_lowercase());
    Ok(folders)
}

/// Puntos de partida: la carpeta del usuario y los discos montados.
pub fn shortcuts(home: &Path) -> Vec<Folder> {
    let mut shortcuts = vec![Folder {
        name: "Inicio".to_string(),
        path: home.to_string_lossy().into_owned(),
    }];
    // Los discos: en macOS, los de /Volumes; en Linux, /media o /mnt; en Windows, las unidades.
    shortcuts.extend(
        crate::platform::drive_roots()
            .into_iter()
            .map(|(name, path)| Folder {
                name,
                path: path.to_string_lossy().into_owned(),
            }),
    );
    shortcuts
}

pub fn home_dir() -> PathBuf {
    crate::platform::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Comprueba que Kinora pueda escribir en la carpeta creando y borrando un archivo de prueba.
pub fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".kinora-write-test");
    let ok = fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_visible_folders() {
        let root = std::env::temp_dir().join("kinora-test-folders");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Series")).unwrap();
        fs::create_dir_all(root.join("anime")).unwrap();
        fs::create_dir_all(root.join(".oculta")).unwrap();
        fs::write(root.join("archivo.txt"), b"x").unwrap();

        let names: Vec<String> = subfolders(&root)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, ["anime", "Series"]);
        assert!(is_writable(&root));
    }
}
