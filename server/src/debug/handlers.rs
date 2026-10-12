use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use super::{TablePage, TableSummary, repo};
use crate::app::AppState;
use crate::error::{AppError, AppResult};

#[derive(Deserialize)]
pub struct PageParams {
    limit: Option<i64>,
    offset: Option<i64>,
}

/// GET /api/admin/debug/tables
pub async fn list_tables(State(state): State<AppState>) -> AppResult<Json<Vec<TableSummary>>> {
    let mut tables = Vec::new();
    for name in repo::table_names(&state.db).await? {
        let rows = repo::count_rows(&state.db, &name).await?;
        tables.push(TableSummary { name, rows });
    }
    Ok(Json(tables))
}

/// GET /api/admin/debug/tables/{name}?limit=100&offset=0
pub async fn table_page(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(page): Query<PageParams>,
) -> AppResult<Json<TablePage>> {
    // El nombre llega desde la URL: solo se aceptan tablas que existen.
    if !repo::table_names(&state.db).await?.contains(&name) {
        return Err(AppError::NotFound(format!("no existe la tabla {name}")));
    }

    let limit = page.limit.unwrap_or(100).clamp(1, 500);
    let offset = page.offset.unwrap_or(0).max(0);

    Ok(Json(TablePage {
        columns: repo::columns(&state.db, &name).await?,
        rows: repo::rows(&state.db, &name, limit, offset).await?,
        total: repo::count_rows(&state.db, &name).await?,
        name,
        limit,
        offset,
    }))
}
