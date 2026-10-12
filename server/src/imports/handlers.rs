use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use super::guess::guess;
use super::{
    AnalyzeRequest, EpisodeFile, FileAnalysis, ImportRequest, ImportResult, ImportTarget,
    ImportedFile, paths,
};
use crate::app::AppState;
use crate::contents::ContentKind;
use crate::contents::repo::{NewContent, NewMediaFile, insert_content, insert_media_file};
use crate::error::{AppError, AppResult};
use crate::probe::{self, Analysis};
use crate::{libraries, uploads};

/// POST /api/admin/imports/analyze
pub async fn analyze(
    State(state): State<AppState>,
    Json(request): Json<AnalyzeRequest>,
) -> AppResult<Json<Vec<FileAnalysis>>> {
    let media_dir = state.settings.require_media_dir()?;
    let mut results = Vec::with_capacity(request.files.len());
    for name in &request.files {
        let path = uploads::resolve(&media_dir, name)?;
        results.push(analyze_file(&path).await);
    }
    Ok(Json(results))
}

async fn analyze_file(path: &Path) -> FileAnalysis {
    let name = file_name(path);
    let size = tokio::fs::metadata(path)
        .await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let mut analysis = FileAnalysis {
        guess: guess(&name),
        name,
        size,
        duration: None,
        video: None,
        audio: Vec::new(),
        problems: Vec::new(),
        error: None,
    };

    match probe::run(path).await {
        Err(err) => analysis.error = Some(format!("{err:#}")),
        Ok(Analysis { probe, .. }) => {
            analysis.duration = probe.duration();
            analysis.video = probe.video().map(|v| {
                format!(
                    "{} {}×{} {} bits",
                    v.codec_name.as_deref().unwrap_or("?"),
                    v.width.unwrap_or(0),
                    v.height.unwrap_or(0),
                    v.bit_depth()
                )
            });
            analysis.audio = probe
                .audios()
                .map(|a| {
                    let codec = a.codec_name.as_deref().unwrap_or("?");
                    let channels = a.channels.unwrap_or(0);
                    match a.language() {
                        Some(lang) => format!("{codec} {channels} canales {lang}"),
                        None => format!("{codec} {channels} canales"),
                    }
                })
                .collect();
            analysis.problems = probe.problems(path);
        }
    }
    analysis
}

/// Un archivo validado, listo para copiar.
struct PlannedFile {
    episode: Option<i64>,
    source: PathBuf,
    rel_path: String,
    analysis: Analysis,
}

/// POST /api/admin/imports
pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<ImportRequest>,
) -> AppResult<(StatusCode, Json<ImportResult>)> {
    let library = libraries::find(&state.db, request.library_id).await?;
    let media_dir = state.settings.require_media_dir()?;
    let root = media_dir.join(&library.dir);

    // 1. Validar todo antes de crear o copiar nada.
    let (title, year) = match &request.target {
        ImportTarget::Movie { title, year, .. } | ImportTarget::Show { title, year, .. } => {
            (clean_title(title)?, *year)
        }
    };
    let dir = paths::content_dir(&title, year);
    if root.join(&dir).exists() {
        return Err(AppError::Conflict(format!(
            "ya existe la carpeta \"{dir}\" en la biblioteca {}. Agregar a un contenido existente llega en la Etapa 3",
            library.name
        )));
    }

    let files: Vec<(Option<i64>, &str)> = match &request.target {
        ImportTarget::Movie { file, .. } => vec![(None, file.as_str())],
        ImportTarget::Show {
            season, episodes, ..
        } => {
            validate_episodes(*season, episodes)?;
            episodes
                .iter()
                .map(|e| (Some(e.number), e.file.as_str()))
                .collect()
        }
    };

    let mut planned = Vec::new();
    for (episode, name) in files {
        let source = uploads::resolve(&media_dir, name)?;
        let analysis = probe::run(&source)
            .await
            .map_err(|err| AppError::BadRequest(format!("{name}: {err:#}")))?;

        let problems = analysis.probe.problems(&source);
        if !problems.is_empty() {
            return Err(AppError::BadRequest(format!(
                "{name} no está en el formato de Kinora: {}",
                problems.join("; ")
            )));
        }

        let rel_path = match (&request.target, episode) {
            (ImportTarget::Show { season, .. }, Some(number)) => {
                paths::episode_file(&dir, *season, number)
            }
            _ => paths::movie_file(&dir),
        };
        planned.push(PlannedFile {
            episode,
            source,
            rel_path,
            analysis,
        });
    }

    // 2. Crear el contenido en una transacción: se crea todo o nada.
    let new_content = |kind, parent_id, number, title: String, year, dir| NewContent {
        library_id: library.id,
        kind,
        parent_id,
        number,
        title,
        year,
        dir,
    };

    let mut tx = state.db.begin().await?;
    let (top_id, file_content_ids) = match &request.target {
        ImportTarget::Movie { .. } => {
            let movie = new_content(ContentKind::Movie, None, None, title, year, Some(dir));
            let id = insert_content(&mut tx, &movie).await?;
            (id, vec![id])
        }
        ImportTarget::Show { season, .. } => {
            let show = new_content(ContentKind::Show, None, None, title, year, Some(dir));
            let show_id = insert_content(&mut tx, &show).await?;

            let season_content = new_content(
                ContentKind::Season,
                Some(show_id),
                Some(*season),
                format!("Temporada {season}"),
                None,
                None,
            );
            let season_id = insert_content(&mut tx, &season_content).await?;

            let mut episode_ids = Vec::new();
            for file in &planned {
                let number = file.episode.unwrap_or_default();
                let episode = new_content(
                    ContentKind::Episode,
                    Some(season_id),
                    Some(number),
                    format!("Episodio {number}"),
                    None,
                    None,
                );
                episode_ids.push(insert_content(&mut tx, &episode).await?);
            }
            (show_id, episode_ids)
        }
    };
    tx.commit().await?;

    // 3. Mover y registrar cada archivo. Si uno falla, los demás siguen.
    let mut files = Vec::new();
    for (file, content_id) in planned.iter().zip(file_content_ids) {
        let result = move_and_register(&state, &root, content_id, file).await;
        files.push(ImportedFile {
            file: file_name(&file.source),
            destination: result.is_ok().then(|| file.rel_path.clone()),
            error: result.err().map(|err| format!("{err:#}")),
        });
    }

    Ok((
        StatusCode::CREATED,
        Json(ImportResult {
            content_id: top_id,
            files,
        }),
    ))
}

/// Mueve el archivo de la bandeja a su lugar en la biblioteca. Como están en el mismo disco,
/// es un renombrado: instantáneo y atómico. Si la biblioteca estuviera en otro disco, se copia
/// a `.kinora-tmp/` y recién al terminar se mueve, para que nunca quede un archivo a medias.
async fn move_and_register(
    state: &AppState,
    root: &Path,
    content_id: i64,
    file: &PlannedFile,
) -> anyhow::Result<()> {
    let destination = root.join(&file.rel_path);
    if destination.exists() {
        bail!("ya existe {}", destination.display());
    }
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    if tokio::fs::rename(&file.source, &destination).await.is_err() {
        let tmp_dir = root.join(".kinora-tmp");
        tokio::fs::create_dir_all(&tmp_dir).await?;
        let tmp = tmp_dir.join(format!("{content_id}.mp4.part"));
        let copied = async {
            tokio::fs::copy(&file.source, &tmp)
                .await
                .context("no se pudo copiar")?;
            tokio::fs::rename(&tmp, &destination)
                .await
                .context("no se pudo mover a la biblioteca")
        }
        .await;
        if copied.is_err() {
            let _ = tokio::fs::remove_file(&tmp).await;
        }
        copied?;
        tokio::fs::remove_file(&file.source).await?;
    }

    let size = tokio::fs::metadata(&destination).await?.len() as i64;
    let video = file.analysis.probe.video();
    let media_file = NewMediaFile {
        content_id,
        rel_path: file.rel_path.clone(),
        size,
        duration: file.analysis.probe.duration(),
        video_codec: video.and_then(|v| v.codec_string()),
        width: video.and_then(|v| v.width),
        height: video.and_then(|v| v.height),
        probe_json: file.analysis.json.clone(),
        source_path: file_name(&file.source),
    };
    insert_media_file(&mut *state.db.acquire().await?, &media_file).await?;
    Ok(())
}

fn clean_title(title: &str) -> AppResult<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest(
            "el título no puede estar vacío".to_string(),
        ));
    }
    Ok(title.to_string())
}

fn validate_episodes(season: i64, episodes: &[EpisodeFile]) -> AppResult<()> {
    if season < 0 {
        return Err(AppError::BadRequest(
            "la temporada no puede ser negativa".to_string(),
        ));
    }
    if episodes.is_empty() {
        return Err(AppError::BadRequest(
            "no hay episodios para importar".to_string(),
        ));
    }
    let mut numbers = HashSet::new();
    let mut paths = HashSet::new();
    for episode in episodes {
        if episode.number < 0 {
            return Err(AppError::BadRequest(
                "un número de episodio es negativo".to_string(),
            ));
        }
        if !numbers.insert(episode.number) {
            return Err(AppError::BadRequest(format!(
                "el episodio {} está repetido",
                episode.number
            )));
        }
        if !paths.insert(&episode.file) {
            return Err(AppError::BadRequest(format!(
                "{} está dos veces",
                episode.file
            )));
        }
    }
    Ok(())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
