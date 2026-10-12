//! Depuración desde el Admin: ver las tablas de la base de datos y sus filas.
//! Es de solo lectura.

mod handlers;
mod repo;

use axum::Router;
use axum::routing::get;
use serde::Serialize;
use ts_rs::TS;

use crate::app::AppState;

#[derive(Serialize, TS)]
#[ts(export)]
pub struct TableSummary {
    pub name: String,
    pub rows: i64,
}

#[derive(Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct ColumnInfo {
    pub name: String,
    pub sql_type: String,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct TablePage {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    /// Cada fila es una lista de valores en el orden de `columns`.
    pub rows: Vec<Vec<serde_json::Value>>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/debug/tables", get(handlers::list_tables))
        .route("/debug/tables/{name}", get(handlers::table_page))
}
