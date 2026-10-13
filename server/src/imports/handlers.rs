use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use super::guess::guess;
use sqlx::SqliteConnection;
use tracing::info;

use super::place::{PlaceFile, free_version, place};
use super::{
    AnalyzeRequest, ConflictChoice, EpisodeFile, FileAnalysis, ImportAction, ImportRequest,
    ImportResult, ImportTarget, ImportedFile, paths,
};
use crate::app::AppState;
use crate::contents::ContentKind;
use crate::contents::repo::{self, NewContent, insert_content};
use crate::disk::{self, format_size};
use crate::error::{AppError, AppResult};
use crate::probe::{self, Analysis, Operation};
use crate::{jobs, libraries, manifest, subtitles, trash, uploads};

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
        subtitles: Vec::new(),
        operation: None,
        changes: Vec::new(),
        blockers: Vec::new(),
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
            analysis.subtitles = subtitles::describe(&probe);
            let plan = probe.plan(path);
            analysis.operation = Some(plan.operation);
            analysis.changes = plan.changes;
            analysis.blockers = plan.blockers;
        }
    }
    analysis
}

/// Un archivo validado, listo para mover.
struct PlannedFile {
    episode: Option<i64>,
    on_conflict: Option<ConflictChoice>,
    source: PathBuf,
    /// Copiar (en el momento) o remuxar (en la cola).
    operation: Operation,
    analysis: Analysis,
}

/// Un archivo pedido: número de episodio (nada si es película), nombre en la bandeja y qué
/// hacer si el destino ya tiene archivo.
type Requested<'a> = (Option<i64>, &'a str, Option<ConflictChoice>);

/// Qué se hace con cada archivo. Se decide dentro de la transacción, mirando lo que ya hay.
struct Placement {
    content_id: i64,
    action: ImportAction,
    /// Ruta final; vacía si se omite.
    rel_path: String,
    /// Archivos que reemplaza (van a la papelera).
    replaces: Vec<i64>,
}

/// POST /api/admin/imports
pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<ImportRequest>,
) -> AppResult<(StatusCode, Json<ImportResult>)> {
    let library = libraries::find(&state.db, request.library_id).await?;
    let media_dir = state.settings.require_media_dir()?;
    let root = media_dir.join(&library.dir);

    // 1. Validar todo antes de crear o mover nada.
    let new_content = match &request.target {
        ImportTarget::Movie { title, year, .. } | ImportTarget::Show { title, year, .. } => {
            let title = clean_title(title)?;
            let dir = paths::content_dir(&title, *year);
            if root.join(&dir).exists() {
                return Err(AppError::Conflict(format!(
                    "ya existe la carpeta \"{dir}\" en la biblioteca {}: elegí \"Agregar a uno existente\"",
                    library.name
                )));
            }
            Some((title, *year, dir))
        }
        _ => None,
    };

    let (season, files): (Option<i64>, Vec<Requested>) = match &request.target {
        ImportTarget::Movie { file, .. } => (None, vec![(None, file.as_str(), None)]),
        ImportTarget::ExistingMovie {
            file, on_conflict, ..
        } => (None, vec![(None, file.as_str(), *on_conflict)]),
        ImportTarget::Show {
            season, episodes, ..
        }
        | ImportTarget::ExistingShow {
            season, episodes, ..
        } => {
            validate_episodes(*season, episodes)?;
            let files = episodes
                .iter()
                .map(|e| (Some(e.number), e.file.as_str(), e.on_conflict))
                .collect();
            (Some(*season), files)
        }
    };

    let mut planned = Vec::new();
    for (episode, name, on_conflict) in files {
        let source = uploads::resolve(&media_dir, name)?;
        let analysis = probe::run(&source)
            .await
            .map_err(|err| AppError::BadRequest(format!("{name}: {err:#}")))?;
        let plan = analysis.probe.plan(&source);
        if !plan.blockers.is_empty() {
            return Err(AppError::BadRequest(format!(
                "{name} no se puede importar todavía: {}",
                plan.blockers.join("; ")
            )));
        }
        planned.push(PlannedFile {
            episode,
            on_conflict,
            source,
            operation: plan.operation,
            analysis,
        });
    }

    // El remux escribe un archivo nuevo antes de borrar el original: hace falta lugar para
    // los dos mientras dura.
    let needed: u64 = planned
        .iter()
        .filter(|f| f.operation != Operation::Copy)
        .filter_map(|f| std::fs::metadata(&f.source).ok())
        .map(|m| m.len())
        .sum();
    if needed > 0
        && let Some(space) = disk::space_of(&media_dir)
        && space.available < needed + 1024 * 1024 * 1024
    {
        return Err(AppError::Conflict(format!(
            "no hay lugar en el disco: el remux necesita {} libres (más 1 GB de margen) y hay {}",
            format_size(needed),
            format_size(space.available)
        )));
    }

    // 2. En una transacción: crear lo que falta (serie, temporada, episodios) y decidir qué
    //    hacer con cada archivo. Si hay un conflicto sin resolver, no queda nada creado.
    let mut tx = state.db.begin().await?;
    let content = |kind, parent_id, number, title: String, year, dir| NewContent {
        library_id: library.id,
        kind,
        parent_id,
        number,
        title,
        year,
        dir,
    };

    let (top_id, top_dir) = match (&request.target, new_content) {
        (ImportTarget::Movie { .. }, Some((title, year, dir))) => {
            let movie = content(
                ContentKind::Movie,
                None,
                None,
                title,
                year,
                Some(dir.clone()),
            );
            (insert_content(&mut tx, &movie).await?, dir)
        }
        (ImportTarget::Show { .. }, Some((title, year, dir))) => {
            let show = content(
                ContentKind::Show,
                None,
                None,
                title,
                year,
                Some(dir.clone()),
            );
            (insert_content(&mut tx, &show).await?, dir)
        }
        (ImportTarget::ExistingMovie { movie_id: id, .. }, _) => {
            existing(&mut tx, *id, ContentKind::Movie, library.id).await?
        }
        (ImportTarget::ExistingShow { show_id: id, .. }, _) => {
            existing(&mut tx, *id, ContentKind::Show, library.id).await?
        }
        _ => unreachable!("los contenidos nuevos siempre traen título"),
    };

    // A qué contenido va cada archivo: la película misma, o el episodio (creándolo si falta).
    let mut content_ids = Vec::new();
    match season {
        None => content_ids.push(top_id),
        Some(season) => {
            let season_id = match repo::child_with_number(&mut tx, top_id, season).await? {
                Some(id) => id,
                None => {
                    let new = content(
                        ContentKind::Season,
                        Some(top_id),
                        Some(season),
                        format!("Temporada {season}"),
                        None,
                        None,
                    );
                    insert_content(&mut tx, &new).await?
                }
            };
            for file in &planned {
                let number = file.episode.unwrap_or_default();
                let id = match repo::child_with_number(&mut tx, season_id, number).await? {
                    Some(id) => id,
                    None => {
                        let new = content(
                            ContentKind::Episode,
                            Some(season_id),
                            Some(number),
                            format!("Episodio {number}"),
                            None,
                            None,
                        );
                        insert_content(&mut tx, &new).await?
                    }
                };
                content_ids.push(id);
            }
        }
    }

    let mut placements = Vec::new();
    let mut unresolved = Vec::new();
    for (file, &content_id) in planned.iter().zip(&content_ids) {
        let path = match (season, file.episode) {
            (Some(season), Some(number)) => paths::episode_file(&top_dir, season, number),
            _ => paths::movie_file(&top_dir),
        };
        let current = repo::file_ids_of(&mut tx, content_id).await?;
        let placement = |action, rel_path, replaces| Placement {
            content_id,
            action,
            rel_path,
            replaces,
        };
        placements.push(match (current.is_empty(), file.on_conflict) {
            (true, _) => placement(ImportAction::Added, path, Vec::new()),
            (false, None) => {
                unresolved.push(match file.episode {
                    Some(number) => format!("el episodio {number}"),
                    None => "la película".to_string(),
                });
                continue;
            }
            (false, Some(ConflictChoice::Skip)) => {
                placement(ImportAction::Skipped, String::new(), Vec::new())
            }
            (false, Some(ConflictChoice::Replace)) => {
                placement(ImportAction::Replaced, path, current)
            }
            (false, Some(ConflictChoice::AddVersion)) => {
                let path = free_version(&mut tx, &root, &path).await?;
                placement(ImportAction::AddedVersion, path, Vec::new())
            }
        });
    }
    if !unresolved.is_empty() {
        return Err(AppError::Conflict(format!(
            "{} ya tiene archivo: elegí omitir, reemplazar o agregar como otra versión",
            unresolved.join(", ")
        )));
    }
    tx.commit().await?;

    // 3. Los que ya cumplen el formato se mueven ahora; los que necesitan remux van a la cola.
    //    Si uno falla, los demás siguen.
    let trash_batch = trash::batch_dir(&root);
    let mut results = Vec::new();
    let mut queued = false;
    for (file, placement) in planned.iter().zip(placements) {
        let mut imported = ImportedFile {
            file: file_name(&file.source),
            action: placement.action,
            destination: None,
            job_id: None,
            trashed: Vec::new(),
            subtitles: 0,
            error: None,
        };
        if placement.action == ImportAction::Skipped {
        } else if file.operation == Operation::Copy {
            let placed = place(
                &state,
                &root,
                &trash_batch,
                PlaceFile {
                    content_id: placement.content_id,
                    rel_path: &placement.rel_path,
                    file: &file.source,
                    analysis: &file.analysis,
                    source_name: &imported.file,
                    replaces: &placement.replaces,
                },
            )
            .await;
            match placed {
                Ok((media_file_id, trashed)) => {
                    imported.trashed = trashed;
                    let tracks = subtitles::find_tracks(&file.analysis.probe);
                    imported.subtitles = subtitles::attach_subtitles(
                        &state.db,
                        &root,
                        media_file_id,
                        &placement.rel_path,
                        &root.join(&placement.rel_path),
                        tracks,
                    )
                    .await as i64;
                    imported.destination = Some(placement.rel_path);
                }
                Err(err) => imported.error = Some(format!("{err:#}")),
            }
        } else {
            match enqueue(&state, &root, library.id, file, &placement).await {
                Ok(job_id) => {
                    imported.job_id = Some(job_id);
                    imported.destination = Some(placement.rel_path);
                    queued = true;
                }
                Err(err) => imported.error = Some(format!("{err:#}")),
            }
        }
        results.push(imported);
    }
    if queued {
        state.jobs.wake();
    }

    manifest::refresh(&state.db, &media_dir, top_id).await;

    Ok((
        StatusCode::CREATED,
        Json(ImportResult {
            content_id: top_id,
            files: results,
        }),
    ))
}

/// Un contenido existente del tipo esperado y de esta biblioteca, con su carpeta.
async fn existing(
    conn: &mut SqliteConnection,
    id: i64,
    kind: ContentKind,
    library_id: i64,
) -> AppResult<(i64, String)> {
    let content = repo::find(conn, id)
        .await?
        .filter(|c| c.kind == kind && c.library_id == library_id)
        .ok_or_else(|| {
            AppError::NotFound(format!("no existe el contenido {id} en esta biblioteca"))
        })?;
    let dir = content
        .dir
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("content {id} has no folder")))?;
    Ok((id, dir))
}

/// Crea el trabajo y saca el archivo de la bandeja: pasa a `.kinora-tmp/`, donde espera su turno
/// (así no se puede importar dos veces). Si no se puede mover, el trabajo no queda.
async fn enqueue(
    state: &AppState,
    root: &Path,
    library_id: i64,
    file: &PlannedFile,
    placement: &Placement,
) -> anyhow::Result<i64> {
    let source_name = file_name(&file.source);
    let source_size = tokio::fs::metadata(&file.source).await?.len() as i64;
    let id = jobs::repo::insert(
        &state.db,
        &jobs::repo::NewJob {
            library_id,
            content_id: placement.content_id,
            source_name: &source_name,
            operation: file.operation,
            action: placement.action,
            rel_path: &placement.rel_path,
            source_size,
        },
    )
    .await?;
    let waiting = root
        .join(".kinora-tmp")
        .join(jobs::source_file_name(id, &source_name));
    let moved = async {
        tokio::fs::create_dir_all(waiting.parent().expect("tiene carpeta")).await?;
        tokio::fs::rename(&file.source, &waiting).await
    }
    .await;
    if let Err(err) = moved {
        jobs::repo::delete(&state.db, id).await?;
        return Err(err).context("no se pudo pasar el archivo a la cola");
    }
    info!("queued job {id}: {:?} {source_name}", file.operation);
    Ok(id)
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
