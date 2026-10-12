use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use super::{Content, ContentDetail, ContentKind, MediaFile, repo};
use crate::app::AppState;
use crate::error::{AppError, AppResult};

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

/// GET /api/contents/{id}
/// El contenido con todos sus descendientes (temporadas, episodios) y sus archivos.
pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<ContentDetail>> {
    let contents = repo::with_descendants(&state.db, id).await?;
    let ids: Vec<i64> = contents.iter().map(|c| c.id).collect();
    let files = repo::files_of(&state.db, &ids).await?;

    // Agrupar hijos por padre y archivos por contenido, y armar el árbol desde la raíz.
    let mut children: HashMap<i64, Vec<Content>> = HashMap::new();
    let mut root = None;
    for content in contents {
        match content.parent_id {
            Some(parent) if content.id != id => children.entry(parent).or_default().push(content),
            _ => root = Some(content),
        }
    }
    let mut files_by_content: HashMap<i64, Vec<MediaFile>> = HashMap::new();
    for file in files {
        files_by_content
            .entry(file.content_id)
            .or_default()
            .push(file);
    }

    let root = root.ok_or_else(|| AppError::NotFound(format!("no existe el contenido {id}")))?;
    Ok(Json(build_tree(root, &mut children, &mut files_by_content)))
}

/// Arma el árbol de forma recursiva. Cada contenido y archivo se mueve (no se copia) a su lugar.
fn build_tree(
    content: Content,
    children: &mut HashMap<i64, Vec<Content>>,
    files: &mut HashMap<i64, Vec<MediaFile>>,
) -> ContentDetail {
    let mut kids = children.remove(&content.id).unwrap_or_default();
    kids.sort_by_key(|c| c.number);
    ContentDetail {
        files: files.remove(&content.id).unwrap_or_default(),
        children: kids
            .into_iter()
            .map(|child| build_tree(child, children, files))
            .collect(),
        content,
    }
}
