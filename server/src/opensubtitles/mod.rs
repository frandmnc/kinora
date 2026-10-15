//! Buscar y bajar subtítulos de OpenSubtitles desde el Admin.
//!
//! La búsqueda usa el `tmdb_id` de la película o serie (de la metadata de TMDB): para un
//! episodio, el de la serie con su temporada y número. Sin `tmdb_id`, busca por el título. El
//! subtítulo elegido se baja en SRT y entra como uno subido a mano: se convierte a WebVTT y se
//! guarda al lado del video.

mod client;
mod handlers;

use axum::Router;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;

use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::settings;
use crate::subtitles::SubtitleTrack;
use client::OpenSubtitles;

/// Dónde se guarda la cuenta, en la tabla `settings`. Nunca sale del servidor.
const KEY: &str = "opensubtitles_key";
const USERNAME: &str = "opensubtitles_username";
const PASSWORD: &str = "opensubtitles_password";

#[derive(Serialize, TS)]
#[ts(export)]
pub struct OpenSubtitlesSetting {
    pub configured: bool,
    /// El usuario de la cuenta (la clave y la contraseña no vuelven del servidor).
    pub username: Option<String>,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct OpenSubtitlesAccount {
    /// La clave de la API (opensubtitles.com → API consumers).
    pub key: String,
    pub username: String,
    pub password: String,
}

/// Un idioma de OpenSubtitles: "es", "ea" (español latino), "pt-br"…
#[derive(Serialize, TS)]
#[ts(export)]
pub struct SubtitleLanguage {
    pub code: String,
    pub name: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct SubtitleSearch {
    pub results: Vec<SubtitleCandidate>,
    /// Cómo se buscó: con el id de TMDB o por el título.
    pub by_tmdb: bool,
    /// Descargas que le quedan hoy a la cuenta.
    pub remaining: Option<i64>,
}

/// Un subtítulo encontrado.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct SubtitleCandidate {
    /// Para pedir la descarga.
    pub file_id: i64,
    /// Código de OpenSubtitles ("es", "ea", "en", "pt-br").
    pub language: String,
    /// Cómo va a quedar en el menú del Player: "Español (Latinoamérica)".
    pub label: String,
    /// El nombre del release: "Frieren.S01E01.1080p.WEB".
    pub release: String,
    pub download_count: i64,
    /// Con descripciones de sonidos, para personas sordas.
    pub hearing_impaired: bool,
    /// Solo las partes en otro idioma.
    pub forced: bool,
    /// Traducido por una máquina o una IA (suele ser peor).
    pub machine_translated: bool,
    /// Subido por alguien de confianza de OpenSubtitles.
    pub trusted: bool,
    pub uploader: Option<String>,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct DownloadSubtitle {
    pub file_id: i64,
    pub language: String,
    pub hearing_impaired: bool,
    pub forced: bool,
    pub release: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct DownloadedSubtitle {
    pub track: SubtitleTrack,
    pub remaining: Option<i64>,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/settings/opensubtitles",
            get(handlers::setting)
                .put(handlers::save_account)
                .delete(handlers::remove_account),
        )
        .route("/opensubtitles/languages", get(handlers::languages))
        .route("/files/{id}/subtitles/search", get(handlers::search))
        .route(
            "/files/{id}/subtitles/opensubtitles",
            post(handlers::download),
        )
}

/// El cliente con la cuenta guardada, o un error que dice dónde cargarla.
async fn client(db: &SqlitePool) -> AppResult<OpenSubtitles> {
    let get = |key| settings::repo::get(db, key);
    let (Some(key), Some(username), Some(password)) =
        (get(KEY).await?, get(USERNAME).await?, get(PASSWORD).await?)
    else {
        return Err(AppError::BadRequest(
            "falta la cuenta de OpenSubtitles: cargala en Configuración para buscar subtítulos"
                .to_string(),
        ));
    };
    Ok(OpenSubtitles::new(&key, &username, &password))
}

/// El idioma (ISO 639-2, como los demás subtítulos) y el nombre para el menú del Player, a
/// partir del código de OpenSubtitles. Un idioma que no está acá usa su código y el nombre que
/// da OpenSubtitles.
fn language_and_label(code: &str, os_name: Option<&str>) -> (String, String) {
    let known = match code {
        "es" => Some(("spa", "Español")),
        "ea" => Some(("spa", "Español (Latinoamérica)")),
        "en" => Some(("eng", "Inglés")),
        "pt-br" => Some(("por", "Portugués (Brasil)")),
        "pt-pt" | "pt" => Some(("por", "Portugués")),
        "fr" => Some(("fra", "Francés")),
        "de" => Some(("deu", "Alemán")),
        "it" => Some(("ita", "Italiano")),
        "ja" => Some(("jpn", "Japonés")),
        "ko" => Some(("kor", "Coreano")),
        "zh-cn" | "zh-tw" => Some(("chi", "Chino")),
        "ru" => Some(("rus", "Ruso")),
        "ar" => Some(("ara", "Árabe")),
        _ => None,
    };
    match known {
        Some((language, label)) => (language.to_string(), label.to_string()),
        None => {
            let base: String = code
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            (base, os_name.unwrap_or(code).to_string())
        }
    }
}

/// "Español (Latinoamérica)", más "(SDH)" o "(forzados)" si corresponde.
fn full_label(label: &str, hearing_impaired: bool, forced: bool) -> String {
    let mut label = label.to_string();
    if forced {
        label.push_str(" (forzados)");
    } else if hearing_impaired {
        label.push_str(" (SDH)");
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_map_to_our_codes_and_names() {
        assert_eq!(
            language_and_label("ea", None),
            ("spa".into(), "Español (Latinoamérica)".into())
        );
        assert_eq!(language_and_label("pt-br", None).0, "por");
        assert_eq!(
            language_and_label("sv", Some("Swedish")),
            ("sv".into(), "Swedish".into())
        );
        assert_eq!(full_label("Inglés", true, false), "Inglés (SDH)");
        assert_eq!(full_label("Inglés", true, true), "Inglés (forzados)");
    }
}
