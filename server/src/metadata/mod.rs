//! Metadata de películas y series: títulos, años, sinopsis, carátulas y títulos de episodios.
//!
//! Sale de TMDB (con la clave que se carga en Configuración) o se corrige a mano. Las imágenes
//! se guardan como `poster.jpg` y `backdrop.jpg` en la carpeta de cada película o serie, al lado
//! de `kinora.json`: así viajan con la biblioteca.
//!
//! Se trae una vez, al elegir el resultado; el botón "Actualizar" la vuelve a pedir.

mod handlers;
mod repo;
pub mod tmdb;

use std::path::Path;

use axum::Router;
use axum::routing::{get, patch, post};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::warn;
use ts_rs::TS;

use crate::app::AppState;
use crate::contents::{self, Content, ContentKind};
use crate::db::now;
use crate::error::{AppError, AppResult};
use crate::manifest;
use crate::settings::{self, Settings};
use tmdb::Tmdb;

/// Dónde se guarda la clave de TMDB, en la tabla `settings`.
const TMDB_KEY: &str = "tmdb_key";

pub const POSTER_FILE: &str = "poster.jpg";
pub const BACKDROP_FILE: &str = "backdrop.jpg";

/// Si hay una clave de TMDB cargada. La clave nunca sale del servidor.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct TmdbSetting {
    pub configured: bool,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct TmdbKey {
    /// La "API Key" (v3) o el "API Read Access Token" (v4) de TMDB.
    pub key: String,
}

/// Un resultado de buscar en TMDB.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct MetadataCandidate {
    pub tmdb_id: i64,
    pub title: String,
    /// El título en su idioma original, si es distinto.
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    /// Carátula chica, servida por TMDB.
    pub thumbnail: Option<String>,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ApplyMetadata {
    pub tmdb_id: i64,
}

/// Corregir a mano el título, el año o la sinopsis de cualquier contenido (también episodios).
#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ContentChanges {
    pub title: String,
    pub year: Option<i64>,
    pub overview: Option<String>,
}

/// Rutas del Player (las imágenes). Se montan bajo /api.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/contents/{id}/poster", get(handlers::poster))
        .route("/contents/{id}/backdrop", get(handlers::backdrop))
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/settings/tmdb",
            get(handlers::tmdb_setting)
                .put(handlers::set_tmdb_key)
                .delete(handlers::remove_tmdb_key),
        )
        .route("/contents/{id}/metadata/search", get(handlers::search))
        .route("/contents/{id}/metadata", post(handlers::apply))
        .route("/contents/{id}", patch(handlers::edit))
}

/// El cliente de TMDB con la clave guardada, o un error que dice dónde cargarla.
async fn client(db: &SqlitePool) -> AppResult<Tmdb> {
    let key = settings::repo::get(db, TMDB_KEY).await?.ok_or_else(|| {
        AppError::BadRequest(
            "falta la clave de TMDB: cargala en Configuración para buscar metadata".to_string(),
        )
    })?;
    Ok(Tmdb::new(&key))
}

fn tmdb_kind(content: &Content) -> AppResult<tmdb::Kind> {
    match content.kind {
        ContentKind::Movie => Ok(tmdb::Kind::Movie),
        ContentKind::Show => Ok(tmdb::Kind::Show),
        _ => Err(AppError::BadRequest(
            "la metadata de TMDB se busca para películas y series enteras".to_string(),
        )),
    }
}

/// Trae de TMDB la película o serie `tmdb_id` y la guarda en `content`: título, año, sinopsis,
/// carátula, fondo y, en una serie, los nombres y sinopsis de sus temporadas y episodios.
/// Solo completa lo que ya existe: no agrega episodios que no se importaron.
async fn apply(
    db: &SqlitePool,
    settings: &Settings,
    content: &Content,
    tmdb_id: i64,
) -> AppResult<()> {
    let kind = tmdb_kind(content)?;
    let tmdb = client(db).await?;
    let details = tmdb.details(kind, tmdb_id).await?;

    let mut seasons = Vec::new();
    if content.kind == ContentKind::Show {
        for (season_id, number) in repo::seasons_of(db, content.id).await? {
            if let Some(season) = tmdb.season(tmdb_id, number).await? {
                seasons.push((season_id, season));
            }
        }
    }

    // Las imágenes, en la carpeta del contenido. Si una no baja, se sigue sin ella.
    let folder = content_folder(db, settings, content).await?;
    let mut poster_at = content.poster_at;
    let mut backdrop_at = content.backdrop_at;
    let images = [
        (&details.poster_path, "w780", POSTER_FILE, &mut poster_at),
        (
            &details.backdrop_path,
            "w1280",
            BACKDROP_FILE,
            &mut backdrop_at,
        ),
    ];
    for (image_path, size, file, saved_at) in images {
        let Some(image_path) = image_path else {
            continue;
        };
        match tmdb
            .download_image(image_path, size, &folder.join(file))
            .await
        {
            Ok(()) => *saved_at = Some(now()),
            Err(err) => warn!(
                "could not download {file} for \"{}\": {err:#}",
                content.title
            ),
        }
    }

    let mut tx = db.begin().await?;
    repo::save_top(
        &mut tx,
        content.id,
        &repo::TopMetadata {
            title: details.title.trim(),
            year: tmdb::year_of(&details.release_date),
            overview: non_blank(details.overview),
            tmdb_id,
            poster_at,
            backdrop_at,
        },
    )
    .await?;
    for (season_id, season) in seasons {
        // TMDB a veces nombra las temporadas "Temporada 1" y otras con su título ("Arco final").
        if let Some(name) = non_blank(season.name) {
            repo::save_text(
                &mut tx,
                season_id,
                &name,
                non_blank(season.overview).as_deref(),
            )
            .await?;
        }
        for episode in season.episodes {
            let Some(id) =
                repo::child_with_number(&mut tx, season_id, episode.episode_number).await?
            else {
                continue;
            };
            if let Some(name) = non_blank(episode.name) {
                repo::save_text(&mut tx, id, &name, non_blank(episode.overview).as_deref()).await?;
            }
        }
    }
    tx.commit().await?;

    if let Some(media_dir) = settings.media_dir() {
        manifest::refresh(db, &media_dir, content.id).await;
    }
    Ok(())
}

/// La carpeta de una película o serie en el disco.
async fn content_folder(
    db: &SqlitePool,
    settings: &Settings,
    content: &Content,
) -> AppResult<std::path::PathBuf> {
    let dir = content.dir.as_deref().ok_or_else(|| {
        AppError::Conflict(format!("\"{}\" todavía no tiene carpeta", content.title))
    })?;
    let library_dir = contents::repo::library_dir(db, content.library_id).await?;
    let folder = settings.require_media_dir()?.join(library_dir).join(dir);
    if !folder.is_dir() {
        return Err(AppError::Conflict(format!(
            "no está la carpeta de \"{}\" ({})",
            content.title,
            folder.display()
        )));
    }
    Ok(folder)
}

fn non_blank(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// Si la imagen existe en el disco (para no ofrecer una que se borró a mano).
fn exists(folder: &Path, file: &str) -> bool {
    folder.join(file).is_file()
}
