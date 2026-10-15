use std::sync::Mutex;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use tracing::info;

use super::client::OpenSubtitles;
use super::{
    DownloadSubtitle, DownloadedSubtitle, KEY, OpenSubtitlesAccount, OpenSubtitlesSetting,
    PASSWORD, SubtitleCandidate, SubtitleLanguage, SubtitleSearch, USERNAME, client, full_label,
    language_and_label,
};
use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::settings;
use crate::subtitles::{NewSubtitle, attach_bytes};

/// La lista de idiomas de OpenSubtitles casi no cambia: se pide una vez.
static LANGUAGES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// GET /api/admin/settings/opensubtitles
pub async fn setting(State(state): State<AppState>) -> AppResult<Json<OpenSubtitlesSetting>> {
    let key = settings::repo::get(&state.db, KEY).await?;
    let username = settings::repo::get(&state.db, USERNAME).await?;
    Ok(Json(OpenSubtitlesSetting {
        configured: key.is_some() && username.is_some(),
        username,
    }))
}

/// PUT /api/admin/settings/opensubtitles
/// Guarda la clave y la cuenta después de comprobar que OpenSubtitles las acepta.
pub async fn save_account(
    State(state): State<AppState>,
    Json(account): Json<OpenSubtitlesAccount>,
) -> AppResult<Json<OpenSubtitlesSetting>> {
    let key = account.key.trim();
    let username = account.username.trim();
    if key.is_empty() || username.is_empty() || account.password.is_empty() {
        return Err(AppError::BadRequest(
            "faltan la clave, el usuario o la contraseña".to_string(),
        ));
    }
    OpenSubtitles::new(key, username, &account.password)
        .check()
        .await?;
    settings::repo::set(&state.db, KEY, key).await?;
    settings::repo::set(&state.db, USERNAME, username).await?;
    settings::repo::set(&state.db, PASSWORD, &account.password).await?;
    info!("OpenSubtitles account saved ({username})");
    Ok(Json(OpenSubtitlesSetting {
        configured: true,
        username: Some(username.to_string()),
    }))
}

/// DELETE /api/admin/settings/opensubtitles
pub async fn remove_account(State(state): State<AppState>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM settings WHERE key IN (?, ?, ?)")
        .bind(KEY)
        .bind(USERNAME)
        .bind(PASSWORD)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/admin/opensubtitles/languages
pub async fn languages(State(state): State<AppState>) -> AppResult<Json<Vec<SubtitleLanguage>>> {
    Ok(Json(
        language_list(&state)
            .await?
            .into_iter()
            .map(|(code, name)| SubtitleLanguage { code, name })
            .collect(),
    ))
}

async fn language_list(state: &AppState) -> AppResult<Vec<(String, String)>> {
    let cached = LANGUAGES.lock().unwrap().clone();
    if !cached.is_empty() {
        return Ok(cached);
    }
    let list: Vec<(String, String)> = client(&state.db)
        .await?
        .languages()
        .await?
        .into_iter()
        .map(|l| (l.language_code.to_lowercase(), l.language_name))
        .collect();
    *LANGUAGES.lock().unwrap() = list.clone();
    Ok(list)
}

#[derive(Deserialize)]
pub struct SearchParams {
    /// Códigos de OpenSubtitles separados por coma: "es,ea,en".
    languages: String,
}

/// Qué se reproduce con un archivo: una película o un episodio, con lo necesario para buscar.
#[derive(sqlx::FromRow)]
struct Target {
    is_episode: bool,
    season: Option<i64>,
    episode: Option<i64>,
    /// El de la película, o el de la serie si es un episodio.
    tmdb_id: Option<i64>,
    title: String,
    year: Option<i64>,
}

/// GET /api/admin/files/{id}/subtitles/search?languages=es,ea,en
/// Subtítulos para el video: los más bajados primero, y los de confianza antes que el resto.
pub async fn search(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    Query(params): Query<SearchParams>,
) -> AppResult<Json<SubtitleSearch>> {
    let languages: Vec<String> = params
        .languages
        .split(',')
        .map(|l| l.trim().to_lowercase())
        .filter(|l| {
            !l.is_empty() && l.len() <= 8 && l.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
        })
        .collect();
    if languages.is_empty() {
        return Err(AppError::BadRequest("elegí al menos un idioma".to_string()));
    }
    let target: Target = sqlx::query_as(
        "SELECT c.kind = 'episode' AS is_episode, season.number AS season, c.number AS episode, \
                COALESCE(show.tmdb_id, c.tmdb_id) AS tmdb_id, COALESCE(show.title, c.title) AS title, \
                COALESCE(show.year, c.year) AS year \
         FROM media_files f JOIN contents c ON c.id = f.content_id \
         LEFT JOIN contents season ON c.kind = 'episode' AND season.id = c.parent_id \
         LEFT JOIN contents show ON show.id = season.parent_id \
         WHERE f.id = ?",
    )
    .bind(file_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("no existe el archivo {file_id}")))?;

    let mut query = vec![
        ("languages", languages.join(",")),
        (
            "type",
            if target.is_episode {
                "episode"
            } else {
                "movie"
            }
            .to_string(),
        ),
    ];
    match (target.tmdb_id, target.is_episode) {
        (Some(id), true) => query.push(("parent_tmdb_id", id.to_string())),
        (Some(id), false) => query.push(("tmdb_id", id.to_string())),
        (None, _) => {
            query.push(("query", target.title.clone()));
            if let (Some(year), false) = (target.year, target.is_episode) {
                query.push(("year", year.to_string()));
            }
        }
    }
    if target.is_episode {
        query.extend(target.season.map(|s| ("season_number", s.to_string())));
        query.extend(target.episode.map(|e| ("episode_number", e.to_string())));
    }

    let os = client(&state.db).await?;
    let items = os.search(query).await?;
    let names = language_list(&state).await.unwrap_or_default();
    let mut results: Vec<SubtitleCandidate> = items
        .into_iter()
        .filter_map(|item| {
            let a = item.attributes;
            // Por título pueden aparecer otros episodios: solo el pedido.
            if target.is_episode
                && let Some(details) = &a.feature_details
                && (details.season_number.is_some() && details.season_number != target.season
                    || details.episode_number.is_some() && details.episode_number != target.episode)
            {
                return None;
            }
            let file = a.files.first()?;
            let language = a.language.clone()?.to_lowercase();
            let os_name = names
                .iter()
                .find(|(code, _)| *code == language)
                .map(|(_, n)| n.as_str());
            let (_, label) = language_and_label(&language, os_name);
            let release = a
                .release
                .clone()
                .filter(|r| !r.trim().is_empty())
                .or_else(|| file.file_name.clone())
                .unwrap_or_else(|| "Sin nombre".to_string());
            Some(SubtitleCandidate {
                file_id: file.file_id,
                label: full_label(&label, &release, a.hearing_impaired, a.foreign_parts_only),
                language,
                release,
                download_count: a.download_count,
                hearing_impaired: a.hearing_impaired,
                forced: a.foreign_parts_only,
                machine_translated: a.machine_translated || a.ai_translated,
                trusted: a.from_trusted,
                uploader: a.uploader.and_then(|u| u.name),
            })
        })
        .collect();
    // Primero lo hecho por personas, después lo de confianza, después lo más bajado.
    results.sort_by(|a, b| {
        (a.machine_translated, !a.trusted, -a.download_count).cmp(&(
            b.machine_translated,
            !b.trusted,
            -b.download_count,
        ))
    });

    Ok(Json(SubtitleSearch {
        results,
        by_tmdb: target.tmdb_id.is_some(),
        remaining: os.remaining().await.ok().flatten(),
    }))
}

/// POST /api/admin/files/{id}/subtitles/opensubtitles
/// Baja el subtítulo elegido y lo agrega al video, como uno subido a mano.
pub async fn download(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    Json(body): Json<DownloadSubtitle>,
) -> AppResult<(StatusCode, Json<DownloadedSubtitle>)> {
    let os = client(&state.db).await?;
    let downloaded = os.download(body.file_id).await?;
    let names = language_list(&state).await.unwrap_or_default();
    let os_name = names
        .iter()
        .find(|(code, _)| *code == body.language)
        .map(|(_, n)| n.as_str());
    let (language, label) = language_and_label(&body.language, os_name);
    let track = attach_bytes(
        &state,
        file_id,
        NewSubtitle {
            // Queda como su origen: "OpenSubtitles · Frieren.S01E01.1080p.srt".
            name: format!(
                "OpenSubtitles · {}.srt",
                body.release.trim_end_matches(".srt")
            ),
            language: Some(language),
            forced: body.forced,
            label: Some(full_label(
                &label,
                &body.release,
                body.hearing_impaired,
                body.forced,
            )),
        },
        &downloaded.bytes,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(DownloadedSubtitle {
            track,
            remaining: downloaded.remaining,
        }),
    ))
}
