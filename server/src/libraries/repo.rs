//! Consultas a la tabla `libraries`.

use sqlx::SqlitePool;

use super::Library;
use crate::error::{AppError, AppResult};

pub async fn list_all(db: &SqlitePool) -> AppResult<Vec<Library>> {
    let libraries = sqlx::query_as("SELECT id, name, root_dir FROM libraries ORDER BY name")
        .fetch_all(db)
        .await?;
    Ok(libraries)
}

pub async fn insert(db: &SqlitePool, name: &str, root_dir: &str) -> AppResult<Library> {
    sqlx::query_as(
        "INSERT INTO libraries (name, root_dir) VALUES (?, ?) RETURNING id, name, root_dir",
    )
    .bind(name)
    .bind(root_dir)
    .fetch_one(db)
    .await
    .map_err(duplicate_as_conflict)
}

pub async fn update_name(db: &SqlitePool, id: i64, name: &str) -> AppResult<Library> {
    sqlx::query_as("UPDATE libraries SET name = ? WHERE id = ? RETURNING id, name, root_dir")
        .bind(name)
        .bind(id)
        .fetch_optional(db)
        .await
        .map_err(duplicate_as_conflict)?
        .ok_or_else(|| not_found(id))
}

pub async fn remove(db: &SqlitePool, id: i64) -> AppResult<()> {
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
}
