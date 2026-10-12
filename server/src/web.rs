//! Entrega del Admin y del Player compilados por Vite (`web/dist`).
//!
//! Por ahora se leen del disco. En la Etapa 5 se embeberán en el binario.

use std::path::PathBuf;

use tower_http::services::ServeDir;

pub fn dist_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../web/dist"))
}

/// Sirve `/` (Player) y `/admin/` (Admin). Cada carpeta entrega su `index.html`.
pub fn service() -> ServeDir {
    ServeDir::new(dist_dir())
}

pub fn is_built() -> bool {
    dist_dir().join("index.html").is_file()
}
