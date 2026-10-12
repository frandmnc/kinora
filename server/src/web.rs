//! Entrega del Admin y del Player compilados por Vite (`web/dist`).
//!
//! Por ahora se leen del disco. En la Etapa 5 se embeberán en el binario.

use std::path::PathBuf;

use axum::http::{HeaderValue, header};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeader;

pub fn dist_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../web/dist"))
}

/// Sirve `/` (Player) y `/admin/` (Admin). Cada carpeta entrega su `index.html`.
///
/// `Cache-Control: no-cache` hace que el navegador pregunte siempre si cambió algo
/// (si no cambió, el servidor responde "304 Not Modified" sin reenviarlo). Así, después
/// de compilar la web, alcanza con recargar la página para ver la versión nueva.
pub fn service() -> SetResponseHeader<ServeDir, HeaderValue> {
    SetResponseHeader::overriding(
        ServeDir::new(dist_dir()),
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    )
}

pub fn is_built() -> bool {
    dist_dir().join("index.html").is_file()
}
