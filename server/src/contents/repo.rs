//! Consultas a `contents` y `media_files`.
//!
//! Las funciones reciben una conexión (`&mut SqliteConnection`) en lugar del pool,
//! para poder usarlas dentro de una transacción.

use sqlx::{AssertSqlSafe, SqliteConnection, SqlitePool};

use super::{Content, ContentKind, MediaFile};
use crate::db::now;

const CONTENT_COLUMNS: &str =
    "id, library_id, kind, parent_id, number, title, year, overview, dir, created_at";

/// Películas y series, opcionalmente filtradas por biblioteca y tipo.
pub async fn list_top_level(
    db: &SqlitePool,
    library_id: Option<i64>,
    kind: Option<ContentKind>,
) -> sqlx::Result<Vec<Content>> {
    // `? IS NULL OR ...`: si el filtro no viene, la condición se cumple siempre.
    sqlx::query_as(
        "SELECT id, library_id, kind, parent_id, number, title, year, overview, dir, created_at \
         FROM contents \
         WHERE parent_id IS NULL \
           AND (?1 IS NULL OR library_id = ?1) \
           AND (?2 IS NULL OR kind = ?2) \
         ORDER BY title COLLATE NOCASE",
    )
    .bind(library_id)
    .bind(kind)
    .fetch_all(db)
    .await
}

/// Un contenido y todos sus descendientes, con una consulta recursiva.
pub async fn with_descendants(db: &SqlitePool, id: i64) -> sqlx::Result<Vec<Content>> {
    let sql = format!(
        "WITH RECURSIVE tree(id) AS ( \
           SELECT id FROM contents WHERE id = ? \
           UNION ALL \
           SELECT c.id FROM contents c JOIN tree t ON c.parent_id = t.id \
         ) \
         SELECT {CONTENT_COLUMNS} FROM contents WHERE id IN (SELECT id FROM tree)"
    );
    sqlx::query_as(AssertSqlSafe(sql))
        .bind(id)
        .fetch_all(db)
        .await
}

/// Los archivos de varios contenidos.
pub async fn files_of(db: &SqlitePool, content_ids: &[i64]) -> sqlx::Result<Vec<MediaFile>> {
    if content_ids.is_empty() {
        return Ok(Vec::new());
    }
    // Un `?` por id: (?, ?, ?)
    let placeholders = vec!["?"; content_ids.len()].join(", ");
    let sql = format!(
        "SELECT id, content_id, rel_path, size, duration, video_codec, width, height, source_path, imported_at \
         FROM media_files WHERE content_id IN ({placeholders}) ORDER BY id"
    );
    let mut query = sqlx::query_as(AssertSqlSafe(sql));
    for id in content_ids {
        query = query.bind(id);
    }
    query.fetch_all(db).await
}

pub struct NewContent {
    pub library_id: i64,
    pub kind: ContentKind,
    pub parent_id: Option<i64>,
    pub number: Option<i64>,
    pub title: String,
    pub year: Option<i64>,
    pub dir: Option<String>,
}

pub async fn insert_content(conn: &mut SqliteConnection, new: &NewContent) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO contents (library_id, kind, parent_id, number, title, year, dir, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(new.library_id)
    .bind(new.kind)
    .bind(new.parent_id)
    .bind(new.number)
    .bind(&new.title)
    .bind(new.year)
    .bind(&new.dir)
    .bind(now())
    .fetch_one(conn)
    .await
}

pub struct NewMediaFile {
    pub content_id: i64,
    pub rel_path: String,
    pub size: i64,
    pub duration: Option<f64>,
    pub video_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub probe_json: String,
    pub source_path: String,
}

pub async fn insert_media_file(
    conn: &mut SqliteConnection,
    new: &NewMediaFile,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO media_files \
         (content_id, rel_path, size, duration, video_codec, width, height, probe_json, source_path, imported_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(new.content_id)
    .bind(&new.rel_path)
    .bind(new.size)
    .bind(new.duration)
    .bind(&new.video_codec)
    .bind(new.width)
    .bind(new.height)
    .bind(&new.probe_json)
    .bind(&new.source_path)
    .bind(now())
    .fetch_one(conn)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn episode_numbers_are_unique_within_a_season() {
        let db = connect_in_memory().await;
        let mut conn = db.acquire().await.unwrap();
        sqlx::query("INSERT INTO libraries (name, dir) VALUES ('Anime', 'Anime')")
            .execute(&mut *conn)
            .await
            .unwrap();

        let content = |kind, parent_id, number| NewContent {
            library_id: 1,
            kind,
            parent_id,
            number,
            title: "x".to_string(),
            year: None,
            dir: None,
        };
        let show = insert_content(&mut conn, &content(ContentKind::Show, None, None))
            .await
            .unwrap();
        let season = insert_content(
            &mut conn,
            &content(ContentKind::Season, Some(show), Some(1)),
        )
        .await
        .unwrap();
        insert_content(
            &mut conn,
            &content(ContentKind::Episode, Some(season), Some(1)),
        )
        .await
        .unwrap();

        let repeated = insert_content(
            &mut conn,
            &content(ContentKind::Episode, Some(season), Some(1)),
        )
        .await;
        assert!(repeated.is_err());
    }
}
