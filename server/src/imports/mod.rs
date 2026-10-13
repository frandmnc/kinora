//! Importación: analizar archivos de las carpetas de origen y agregarlos al catálogo.
//!
//! Los archivos vienen de la bandeja de entrada (ver `uploads`) y se identifican por su nombre.
//! Los que ya cumplen el formato de Kinora se mueven tal cual, en el momento. Los que necesitan
//! remux (MKV con HEVC) van a la cola (`jobs`) y entran a la biblioteca al terminar.
//! El destino puede ser un contenido nuevo o uno existente (otra temporada, episodios que faltan,
//! una versión mejor). Si un episodio ya tiene archivo, el pedido dice qué hacer: omitir,
//! reemplazar (el anterior va a la papelera) o agregar como otra versión.

mod guess;
mod handlers;
mod paths;
mod place;

pub use place::{PlaceFile, free_version, place};

use axum::Router;
use axum::routing::post;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::AppState;
use crate::probe::Operation;

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
    /// Ej. ["Español (ASS)", "Inglés (SRT)"].
    pub subtitles: Vec<String>,
    /// Qué hay que hacerle para que cumpla el formato. `null` si no se pudo leer.
    pub operation: Option<Operation>,
    /// Lo que va a cambiar: "contenedor matroska → MP4", "audio opus → AAC".
    pub changes: Vec<String>,
    /// Lo que impide importarlo por ahora. Vacía = se puede importar.
    pub blockers: Vec<String>,
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

/// Dónde va lo importado. En JSON lleva un campo `kind`: `{"kind": "movie", ...}`.
#[derive(Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum ImportTarget {
    /// Una película nueva.
    Movie {
        title: String,
        year: Option<i64>,
        file: String,
    },
    /// Una serie nueva, con una temporada.
    Show {
        title: String,
        year: Option<i64>,
        season: i64,
        episodes: Vec<EpisodeFile>,
    },
    /// Otro archivo para una película que ya está en el catálogo.
    ExistingMovie {
        movie_id: i64,
        file: String,
        on_conflict: Option<ConflictChoice>,
    },
    /// Episodios para una serie que ya está en el catálogo, en una temporada existente o nueva.
    ExistingShow {
        show_id: i64,
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
    /// Qué hacer si el episodio ya tiene archivo. Sin elegir, el pedido se rechaza.
    #[serde(default)]
    #[ts(optional)]
    pub on_conflict: Option<ConflictChoice>,
}

/// Qué hacer cuando el destino ya tiene archivo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ConflictChoice {
    /// No importar este archivo: queda en la bandeja.
    Skip,
    /// El nuevo ocupa su lugar; el anterior y sus subtítulos van a `.kinora-trash/`.
    Replace,
    /// Se suma como otra versión (`… - S01E05 - v2.mp4`); el anterior sigue.
    AddVersion,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ImportResult {
    /// La película o la serie (creada o existente).
    pub content_id: i64,
    pub files: Vec<ImportedFile>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ImportedFile {
    pub file: String,
    /// Qué se hizo con el archivo.
    pub action: ImportAction,
    /// Qué necesitó: moverse tal cual, remux o conversión.
    pub operation: Operation,
    /// Ruta final dentro de la biblioteca, si se importó (o donde va a quedar, si está en cola).
    pub destination: Option<String>,
    /// Si necesitó remux: el trabajo en la cola. Entra a la biblioteca cuando termine.
    pub job_id: Option<i64>,
    /// Archivos anteriores que fueron a la papelera (al reemplazar).
    pub trashed: Vec<String>,
    /// Cuántas pistas de subtítulos quedaron.
    pub subtitles: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, sqlx::Type, TS)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
#[ts(export)]
pub enum ImportAction {
    /// El destino no tenía archivo.
    Added,
    Replaced,
    AddedVersion,
    /// Se eligió omitirlo: sigue en la bandeja.
    Skipped,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/imports/analyze", post(handlers::analyze))
        .route("/imports", post(handlers::create))
}
