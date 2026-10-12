use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::AppState;
use crate::config::expand_home;
use crate::error::{AppError, AppResult};

/// Una biblioteca: una categoría ("Películas", "Anime") con su carpeta destino.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub root_dir: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewLibrary {
    name: String,
    root_dir: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryChanges {
    name: String,
}

// --- Handlers HTTP ---

/// GET /api/admin/libraries
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Library>>> {
    let libraries = list_all(&state.db).await?;
    Ok(Json(libraries))
}

/// POST /api/admin/libraries
pub async fn create(
    State(state): State<AppState>,
    Json(new): Json<NewLibrary>,
) -> AppResult<(StatusCode, Json<Library>)> {
    let name = validate_name(&new.name)?;
    let root_dir = validate_root_dir(&new.root_dir)?;
    let library = insert(&state.db, &name, &root_dir).await?;
    Ok((StatusCode::CREATED, Json(library)))
}

/// PATCH /api/admin/libraries/{id}
pub async fn rename(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(changes): Json<LibraryChanges>,
) -> AppResult<Json<Library>> {
    let name = validate_name(&changes.name)?;
    let library = update_name(&state.db, id, &name).await?;
    Ok(Json(library))
}

/// DELETE /api/admin/libraries/{id}
pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<StatusCode> {
    remove(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Validaciones ---

fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "el nombre no puede estar vacío".to_string(),
        ));
    }
    Ok(name.to_string())
}

/// La carpeta tiene que existir. Se guarda como ruta absoluta y sin enlaces simbólicos.
fn validate_root_dir(root_dir: &str) -> AppResult<String> {
    let path = expand_home(std::path::Path::new(root_dir.trim()))?;
    if !path.is_absolute() {
        return Err(AppError::BadRequest(format!(
            "{} no es una ruta absoluta",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(AppError::BadRequest(format!(
            "{} no existe o no es una carpeta",
            path.display()
        )));
    }
    let path = path.canonicalize()?;
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest("la ruta tiene caracteres no válidos".to_string()))
}

// --- Base de datos ---

pub async fn list_all(db: &SqlitePool) -> AppResult<Vec<Library>> {
    let libraries = sqlx::query_as("SELECT id, name, root_dir FROM libraries ORDER BY name")
        .fetch_all(db)
        .await?;
    Ok(libraries)
}

async fn insert(db: &SqlitePool, name: &str, root_dir: &str) -> AppResult<Library> {
    sqlx::query_as(
        "INSERT INTO libraries (name, root_dir) VALUES (?, ?) RETURNING id, name, root_dir",
    )
    .bind(name)
    .bind(root_dir)
    .fetch_one(db)
    .await
    .map_err(duplicate_as_conflict)
}

async fn update_name(db: &SqlitePool, id: i64, name: &str) -> AppResult<Library> {
    sqlx::query_as("UPDATE libraries SET name = ? WHERE id = ? RETURNING id, name, root_dir")
        .bind(name)
        .bind(id)
        .fetch_optional(db)
        .await
        .map_err(duplicate_as_conflict)?
        .ok_or_else(|| not_found(id))
}

async fn remove(db: &SqlitePool, id: i64) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

fn not_found(id: i64) -> AppError {
    AppError::NotFound(format!("no existe la biblioteca {id}"))
}

/// Un nombre o una carpeta repetidos violan una restricción UNIQUE: es un conflicto, no un error interno.
fn duplicate_as_conflict(err: sqlx::Error) -> AppError {
    match &err {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            AppError::Conflict("ya existe una biblioteca con ese nombre o esa carpeta".to_string())
        }
        _ => AppError::Internal(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn insert_and_list_sorted_by_name() {
        let db = connect_in_memory().await;
        insert(&db, "Series", "/media/series").await.unwrap();
        insert(&db, "Anime", "/media/anime").await.unwrap();

        let names: Vec<String> = list_all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|l| l.name)
            .collect();
        assert_eq!(names, ["Anime", "Series"]);
    }

    #[tokio::test]
    async fn duplicate_name_is_conflict() {
        let db = connect_in_memory().await;
        insert(&db, "Anime", "/media/anime").await.unwrap();
        let result = insert(&db, "Anime", "/media/otra").await;
        assert!(matches!(result, Err(AppError::Conflict(_))));
    }

    #[tokio::test]
    async fn rename_and_delete_missing_are_not_found() {
        let db = connect_in_memory().await;
        assert!(matches!(
            update_name(&db, 99, "X").await,
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(remove(&db, 99).await, Err(AppError::NotFound(_))));
    }

    #[test]
    fn rejects_blank_name_and_missing_folder() {
        assert!(matches!(validate_name("   "), Err(AppError::BadRequest(_))));
        assert!(matches!(
            validate_root_dir("/no/existe"),
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            validate_root_dir("relativa"),
            Err(AppError::BadRequest(_))
        ));
    }
}
