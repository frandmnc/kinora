//! Importación: analizar archivos de las carpetas de origen y agregarlos al catálogo.
//!
//! Los archivos vienen de la bandeja de entrada (ver `uploads`) y se identifican por su nombre.
//! Etapa 1: solo se importan archivos que ya cumplen el formato de Kinora (se mueven tal cual),
//! y siempre como contenido nuevo.

mod guess;
mod handlers;
mod paths;

use axum::Router;
use axum::routing::post;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::AppState;

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct AnalyzeRequest {
    /// Nombres de archivos de la bandeja de entrada.
    pub files: Vec<String>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct FileAnalysis {
    pub name: String,
    pub size: i64,
    pub duration: Option<f64>,
    /// Ej. "hevc 1920×1080 10 bits".
    pub video: Option<String>,
    /// Ej. ["aac 2 canales jpn"].
    pub audio: Vec<String>,
    /// Ej. ["Español (ASS)", "Inglés (SRT, archivo)"].
    pub subtitles: Vec<String>,
    /// Diferencias con el formato de Kinora. Vacía = se puede importar copiando.
    pub problems: Vec<String>,
    /// Si ffprobe no pudo leer el archivo.
    pub error: Option<String>,
    pub guess: Guess,
}

/// Lo que se deduce del nombre del archivo, para completar el formulario.
#[derive(Debug, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Guess {
    pub title: Option<String>,
    pub year: Option<i64>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ImportRequest {
    pub library_id: i64,
    pub target: ImportTarget,
}

/// Qué crear. En JSON lleva un campo `kind`: `{"kind": "movie", ...}` o `{"kind": "show", ...}`.
#[derive(Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum ImportTarget {
    Movie {
        title: String,
        year: Option<i64>,
        file: String,
    },
    Show {
        title: String,
        year: Option<i64>,
        season: i64,
        episodes: Vec<EpisodeFile>,
    },
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct EpisodeFile {
    pub number: i64,
    pub file: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ImportResult {
    /// La película o la serie creada.
    pub content_id: i64,
    pub files: Vec<ImportedFile>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ImportedFile {
    pub file: String,
    /// Ruta final dentro de la biblioteca, si salió bien.
    pub destination: Option<String>,
    /// Cuántas pistas de subtítulos quedaron.
    pub subtitles: i64,
    pub error: Option<String>,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/imports/analyze", post(handlers::analyze))
        .route("/imports", post(handlers::create))
}
