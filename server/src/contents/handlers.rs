use std::collections::{BTreeSet, HashMap, HashSet};
use std::net::SocketAddr;

use axum::Json;
use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::services::ServeFile;
use tracing::info;

use super::{
    Content, ContentDetail, ContentKind, ContentSummary, MediaFile, PlaybackReport, Progress,
    RecentItem, repo,
};
use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::monitor::SessionStep;
use crate::probe::{Probe, TechInfo};
use crate::profiles::Profile;
use crate::subtitles::{self, SubtitleTrack};
use crate::{manifest, playback, trash};

#[derive(Deserialize)]
pub struct ListParams {
    library_id: Option<i64>,
    kind: Option<ContentKind>,
}

/// GET /api/contents?library_id=1&kind=show
/// Películas y series (los contenidos sin padre), por título.
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> AppResult<Json<Vec<Content>>> {
    let contents = repo::list_top_level(&state.db, params.library_id, params.kind).await?;
    Ok(Json(contents))
}

/// Cuántas películas o series muestra "Agregados recientemente".
const RECENT_ITEMS: usize = 20;
/// Lo importado dentro de este margen antes de lo último cuenta como la misma tanda.
const SAME_BATCH_SECS: i64 = 24 * 3600;

/// GET /api/recent
/// Las películas y series con algo agregado hace poco, de la más nueva a la más vieja.
pub async fn recent(State(state): State<AppState>) -> AppResult<Json<Vec<RecentItem>>> {
    // Agrupa los últimos archivos por película o serie, en el orden en que aparecen.
    let mut groups: Vec<(i64, Vec<repo::ImportedFile>)> = Vec::new();
    for file in repo::latest_files(&state.db, 500).await? {
        // Primero la posición (un número: no deja nada prestado) y después se modifica.
        match groups.iter().position(|(top, _)| *top == file.top_id) {
            Some(i) => groups[i].1.push(file),
            None if groups.len() < RECENT_ITEMS => groups.push((file.top_id, vec![file])),
            None => {}
        }
    }

    let mut conn = state.db.acquire().await?;
    let mut items = Vec::new();
    for (top_id, files) in groups {
        let Some(content) = repo::find(&mut conn, top_id).await? else {
            continue;
        };
        let added_at = files[0].imported_at;
        let batch: Vec<&repo::ImportedFile> = files
            .iter()
            .filter(|f| added_at - f.imported_at <= SAME_BATCH_SECS)
            .collect();
        items.push(RecentItem {
            note: recent_note(&batch),
            added_at,
            content,
        });
    }
    Ok(Json(items))
}

/// "T1 E5" si es un episodio; "T2 · 12 episodios" si son varios de una temporada;
/// "12 episodios" si son de varias. Vacío en una película.
fn recent_note(batch: &[&repo::ImportedFile]) -> String {
    let episodes: Vec<(i64, i64)> = batch
        .iter()
        .filter_map(|f| Some((f.season?, f.episode?)))
        .collect();
    match episodes.as_slice() {
        [] => String::new(),
        [(season, episode)] => format!("T{season} E{episode}"),
        [(season, _), rest @ ..] if rest.iter().all(|(s, _)| s == season) => {
            format!("T{season} · {} episodios", episodes.len())
        }
        _ => format!("{} episodios", episodes.len()),
    }
}

/// GET /api/contents/{id}
/// El contenido con todos sus descendientes (temporadas, episodios), sus archivos y su progreso.
pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    profile: Option<Profile>,
) -> AppResult<Json<ContentDetail>> {
    let contents = repo::with_descendants(&state.db, id).await?;
    let ids: Vec<i64> = contents.iter().map(|c| c.id).collect();

    // Agrupar hijos por padre, archivos y progreso por contenido, y armar el árbol desde la raíz.
    let mut parts = TreeParts::default();
    let files = repo::files_of(&state.db, &ids).await?;
    let file_ids: Vec<i64> = files.iter().map(|f| f.id).collect();
    let content_of_file: HashMap<i64, i64> = files.iter().map(|f| (f.id, f.content_id)).collect();
    for track in subtitles::repo::for_files(&state.db, &file_ids).await? {
        if let Some(content_id) = content_of_file.get(&track.media_file_id) {
            parts.subtitles.entry(*content_id).or_default().push(track);
        }
    }
    // Lo técnico de cada archivo sale de su análisis de ffprobe, guardado al importarlo.
    let tech: HashMap<i64, TechInfo> = repo::probes_of(&state.db, &file_ids)
        .await?
        .into_iter()
        .filter_map(|(id, json)| Some((id, Probe::parse(&json).ok()?.tech_info())))
        .collect();
    for mut file in files {
        file.tech = tech.get(&file.id).cloned();
        parts.files.entry(file.content_id).or_default().push(file);
    }
    // El progreso es de cada perfil; el Admin (sin perfil) no lo ve.
    if let Some(profile) = &profile {
        for progress in repo::progress_of(&state.db, profile.id, &ids).await? {
            parts.progress.insert(progress.content_id, progress);
        }
    }
    let mut root = None;
    for content in contents {
        match content.parent_id {
            Some(parent) if content.id != id => {
                parts.children.entry(parent).or_default().push(content)
            }
            _ => root = Some(content),
        }
    }

    let root = root.ok_or_else(|| AppError::NotFound(format!("no existe el contenido {id}")))?;
    Ok(Json(build_tree(root, &mut parts)))
}

/// Las piezas del árbol, agrupadas por id de contenido.
#[derive(Default)]
struct TreeParts {
    children: HashMap<i64, Vec<Content>>,
    files: HashMap<i64, Vec<MediaFile>>,
    progress: HashMap<i64, Progress>,
    subtitles: HashMap<i64, Vec<SubtitleTrack>>,
}

/// Arma el árbol de forma recursiva. Cada pieza se mueve (no se copia) a su lugar.
fn build_tree(content: Content, parts: &mut TreeParts) -> ContentDetail {
    let mut kids = parts.children.remove(&content.id).unwrap_or_default();
    kids.sort_by_key(|c| c.number);
    ContentDetail {
        files: parts.files.remove(&content.id).unwrap_or_default(),
        progress: parts.progress.remove(&content.id),
        subtitles: parts.subtitles.remove(&content.id).unwrap_or_default(),
        children: kids
            .into_iter()
            .map(|child| build_tree(child, parts))
            .collect(),
        content,
    }
}

/// PUT /api/contents/{id}/progress
/// El Player informa dónde está y si reproduce. Se guarda el progreso y se actualiza
/// la lista "viendo ahora" del monitor.
pub async fn report_progress(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    profile: Profile,
    Json(report): Json<PlaybackReport>,
) -> AppResult<StatusCode> {
    let title = repo::display_title(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el contenido {id}")))?;

    let position = report.position.max(0.0);
    let watched = report
        .duration
        .is_some_and(|duration| duration > 0.0 && position >= duration * WATCHED_AT);
    repo::save_progress(
        &state.db,
        profile.id,
        id,
        position,
        report.duration,
        watched,
    )
    .await?;

    let client_ip = client.ip().to_string();
    // Quién mira, para "Viendo ahora" y la terminal: el perfil y desde qué equipo.
    let viewer_name = format!("{} · {client_ip}", profile.name);
    let viewer = state.monitor.report_viewer(
        viewer_name.clone(),
        id,
        title.clone(),
        position,
        report.duration,
        report.state,
    );
    if viewer.watched > 0.0 {
        playback::repo::add_seconds(&state.db, &playback::local_day(0), id, viewer.watched).await?;
    }

    // Historial: una sesión por visualización, actualizada con cada aviso.
    match viewer.session {
        SessionStep::Start => {
            let session = playback::repo::start_session(
                &state.db,
                id,
                &title,
                &profile,
                &client_ip,
                position,
                report.duration,
            )
            .await?;
            state.monitor.set_session(&viewer_name, id, session);
        }
        SessionStep::Update(session) => {
            playback::repo::update_session(
                &state.db,
                session,
                viewer.watched,
                position,
                report.duration,
                watched,
            )
            .await?;
        }
        SessionStep::Pending => {}
    }
    Ok(StatusCode::NO_CONTENT)
}

/// A partir del 90 % de la duración, el contenido se considera visto.
const WATCHED_AT: f64 = 0.9;

/// GET /api/files/{id}/stream
/// Entrega el archivo de video. `ServeFile` responde los pedidos parciales (HTTP Range):
/// el navegador pide solo el tramo que necesita, y así puede adelantar y retroceder.
pub async fn stream(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    request: Request,
) -> AppResult<Response> {
    let (library_dir, rel_path) = repo::file_location(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el archivo {id}")))?;

    let path = state
        .settings
        .require_media_dir()?
        .join(library_dir)
        .join(rel_path);
    if !path.is_file() {
        return Err(AppError::NotFound(format!(
            "el archivo {id} no está en el disco ({})",
            path.display()
        )));
    }

    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("whole file")
        .to_string();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    let response = ServeFile::new(&path).oneshot(request).await?;
    info!(
        "▶ {} requested {file_name} ({range}) → {}",
        client.ip(),
        response.status()
    );

    // Mientras el navegador descarga, se cuentan los bytes enviados. Cuando la descarga
    // termina o se corta, el cuerpo se descarta, y con él el `guard`: el stream deja de figurar.
    let guard = state
        .monitor
        .start_transfer(client.ip().to_string(), id, file_name);
    let (parts, body) = response.into_parts();
    let body = body.map_frame(move |frame| {
        if let Some(data) = frame.data_ref() {
            guard.add_bytes(data.len() as u64);
        }
        frame
    });
    Ok(Response::from_parts(parts, Body::new(body)))
}

/// DELETE /api/admin/files/{id}
/// Saca un archivo (una versión) del catálogo. El video y sus subtítulos van a la papelera de
/// la biblioteca; el episodio o la película quedan, aunque sea sin archivo.
pub async fn delete_file(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    let (library_dir, rel_path) = repo::file_location(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el archivo {id}")))?;
    let top_id = repo::top_of_file(&state.db, id).await?;
    let media_dir = state.settings.require_media_dir()?;
    let root = media_dir.join(library_dir);

    let mut conn = state.db.acquire().await?;
    let paths = repo::paths_of_file(&mut conn, id).await?;
    let batch = trash::batch_dir(&root);
    let trashed = trash::move_files(&root, &batch, &paths).await?;
    if let Err(err) = repo::delete_media_file(&mut conn, id).await {
        // Si no se pudo borrar de la base, los archivos vuelven: o todo o nada.
        trash::restore(&root, &batch, &trashed).await;
        return Err(err.into());
    }
    drop(conn);
    info!("removed {rel_path}: moved to the trash");
    if let Some(top_id) = top_id {
        manifest::refresh(&state.db, &media_dir, top_id).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/admin/libraries/{id}/contents
/// Las películas y series de la biblioteca, cada una con sus números (temporadas, episodios,
/// archivos, tamaño, subtítulos).
pub async fn library_contents(
    State(state): State<AppState>,
    Path(library_id): Path<i64>,
) -> AppResult<Json<Vec<ContentSummary>>> {
    let contents = repo::all_of_library(&state.db, library_id).await?;
    let parents: HashMap<i64, Option<i64>> = contents.iter().map(|c| (c.id, c.parent_id)).collect();
    // La película o serie de cada contenido: se sube por los padres (episodio → temporada → serie).
    let top_of = |mut id: i64| {
        while let Some(Some(parent)) = parents.get(&id) {
            id = *parent;
        }
        id
    };

    let mut counts: HashMap<i64, Counts> = HashMap::new();
    let mut with_files: HashSet<i64> = HashSet::new();
    // Duración por episodio (o película): con varias versiones, cuenta la más larga una sola vez.
    let mut runtimes: HashMap<i64, f64> = HashMap::new();
    for file in repo::file_stats_of_library(&state.db, library_id).await? {
        with_files.insert(file.content_id);
        let runtime = runtimes.entry(file.content_id).or_default();
        *runtime = runtime.max(file.duration.unwrap_or(0.0));
        let entry = counts.entry(top_of(file.content_id)).or_default();
        entry.files += 1;
        entry.size += file.size;
        entry.subtitles += file.subtitles;
        if let Some(tech) = file
            .probe_json
            .as_deref()
            .and_then(|j| Probe::parse(j).ok())
            .map(|p| p.tech_info())
        {
            entry.add_tech(&tech);
        }
    }
    for (content_id, runtime) in runtimes {
        counts.entry(top_of(content_id)).or_default().runtime += runtime;
    }
    for (content_id, jobs) in repo::active_jobs_of_library(&state.db, library_id).await? {
        counts.entry(top_of(content_id)).or_default().active_jobs += jobs;
    }
    for content in &contents {
        let entry = counts.entry(top_of(content.id)).or_default();
        match content.kind {
            ContentKind::Season => entry.seasons += 1,
            ContentKind::Episode => {
                entry.episodes += 1;
                if !with_files.contains(&content.id) {
                    entry.missing += 1;
                }
            }
            _ => {}
        }
    }

    let mut result: Vec<ContentSummary> = contents
        .into_iter()
        .filter(|c| c.parent_id.is_none())
        .map(|content| {
            let c = counts.remove(&content.id).unwrap_or_default();
            ContentSummary {
                content,
                seasons: c.seasons,
                episodes: c.episodes,
                missing: c.missing,
                files: c.files,
                size: c.size,
                subtitles: c.subtitles,
                active_jobs: c.active_jobs,
                runtime: c.runtime,
                resolutions: c.resolutions.into_iter().collect(),
                video: c.video.into_iter().collect(),
                dynamic_range: c.dynamic_range.into_iter().collect(),
                audio: c.audio.into_iter().collect(),
                audio_languages: c.audio_languages.into_iter().collect(),
            }
        })
        .collect();
    result.sort_by_key(|s| s.content.title.to_lowercase());
    Ok(Json(result))
}

/// Los números de una película o serie, mientras se suman. `Default` los arranca en cero.
/// Lo técnico va en `BTreeSet`: sin repetidos y ordenado.
#[derive(Default)]
struct Counts {
    seasons: i64,
    episodes: i64,
    missing: i64,
    files: i64,
    size: i64,
    subtitles: i64,
    active_jobs: i64,
    runtime: f64,
    resolutions: BTreeSet<String>,
    video: BTreeSet<String>,
    dynamic_range: BTreeSet<String>,
    audio: BTreeSet<String>,
    audio_languages: BTreeSet<String>,
}

impl Counts {
    fn add_tech(&mut self, tech: &TechInfo) {
        self.resolutions.extend(tech.resolution.clone());
        if let Some(codec) = &tech.video_codec {
            self.video.insert(match tech.bit_depth {
                Some(bits) => format!("{codec} {bits} bits"),
                None => codec.clone(),
            });
        }
        self.dynamic_range.extend(tech.dynamic_range.clone());
        for audio in &tech.audio {
            self.audio
                .insert(format!("{} {}", audio.codec, audio.channels));
            self.audio_languages.extend(audio.language.clone());
        }
    }
}

/// DELETE /api/admin/contents/{id}
/// Elimina una película o serie entera: su carpeta (videos, subtítulos, kinora.json) va a la
/// papelera de la biblioteca y se borra del catálogo con temporadas, episodios y progreso.
/// El historial de reproducción queda.
pub async fn delete_content(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    let content = repo::find(&mut *state.db.acquire().await?, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no existe el contenido {id}")))?;
    if content.parent_id.is_some() {
        return Err(AppError::BadRequest(
            "solo se eliminan películas o series enteras".to_string(),
        ));
    }
    let ids: Vec<i64> = repo::with_descendants(&state.db, id)
        .await?
        .iter()
        .map(|c| c.id)
        .collect();
    let busy = repo::active_jobs_of_library(&state.db, content.library_id)
        .await?
        .into_iter()
        .any(|(content_id, _)| ids.contains(&content_id));
    if busy {
        return Err(AppError::Conflict(format!(
            "\"{}\" tiene archivos en la cola: cancelalos (o esperá a que terminen) antes de eliminarlo",
            content.title
        )));
    }

    let library_dir = repo::library_dir(&state.db, content.library_id).await?;
    let root = state.settings.require_media_dir()?.join(library_dir);
    // La carpeta entera a la papelera: así va todo junto, incluso lo que Kinora no registró.
    let batch = trash::batch_dir(&root);
    let folder: Vec<String> = content.dir.iter().cloned().collect();
    let trashed = trash::move_files(&root, &batch, &folder).await?;

    // Si la base falla, la carpeta vuelve de la papelera: o todo o nada.
    let deleted = async {
        let mut tx = state.db.begin().await?;
        repo::delete_tree(&mut tx, &ids).await?;
        tx.commit().await
    }
    .await;
    if let Err(err) = deleted {
        trash::restore(&root, &batch, &trashed).await;
        return Err(err.into());
    }
    info!(
        "deleted \"{}\" ({} items); its folder went to the trash",
        content.title,
        ids.len()
    );
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_what_was_added() {
        let file = |season, episode| repo::ImportedFile {
            top_id: 1,
            season,
            episode,
            imported_at: 0,
        };
        let one = [file(Some(1), Some(5))];
        assert_eq!(recent_note(&one.iter().collect::<Vec<_>>()), "T1 E5");
        let season = [file(Some(2), Some(1)), file(Some(2), Some(2))];
        assert_eq!(
            recent_note(&season.iter().collect::<Vec<_>>()),
            "T2 · 2 episodios"
        );
        let mixed = [file(Some(1), Some(9)), file(Some(2), Some(1))];
        assert_eq!(
            recent_note(&mixed.iter().collect::<Vec<_>>()),
            "2 episodios"
        );
        let movie = [file(None, None)];
        assert_eq!(recent_note(&movie.iter().collect::<Vec<_>>()), "");
    }
}
