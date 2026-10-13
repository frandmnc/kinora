//! Consultas a `contents` y `media_files`.
//!
//! Las funciones reciben una conexión (`&mut SqliteConnection`) en lugar del pool,
//! para poder usarlas dentro de una transacción.

use sqlx::{AssertSqlSafe, SqliteConnection, SqlitePool};

use super::{Content, ContentKind, MediaFile, Progress};
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

/// El progreso de varios contenidos.
pub async fn progress_of(db: &SqlitePool, content_ids: &[i64]) -> sqlx::Result<Vec<Progress>> {
    if content_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; content_ids.len()].join(", ");
    let sql = format!(
        "SELECT content_id, position, duration, watched, updated_at \
         FROM progress WHERE content_id IN ({placeholders})"
    );
    let mut query = sqlx::query_as(AssertSqlSafe(sql));
    for id in content_ids {
        query = query.bind(id);
    }
    query.fetch_all(db).await
}

/// Guarda el progreso: inserta la fila o, si ya existe, la actualiza ("upsert").
pub async fn save_progress(
    db: &SqlitePool,
    content_id: i64,
    position: f64,
    duration: Option<f64>,
    watched: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO progress (content_id, position, duration, watched, updated_at) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT (content_id) DO UPDATE SET \
           position = excluded.position, duration = excluded.duration, \
           watched = excluded.watched, updated_at = excluded.updated_at",
    )
    .bind(content_id)
    .bind(position)
    .bind(duration)
    .bind(watched)
    .bind(now())
    .execute(db)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct TitleParts {
    title: String,
    kind: ContentKind,
    number: Option<i64>,
    season_number: Option<i64>,
    show_title: Option<String>,
}

/// Nombre para mostrar: "Interstellar" o "Sousou no Frieren · T1 E1".
pub async fn display_title(db: &SqlitePool, content_id: i64) -> sqlx::Result<Option<String>> {
    let parts: Option<TitleParts> = sqlx::query_as(
        "SELECT c.title, c.kind, c.number, \
                season.number AS season_number, show.title AS show_title \
         FROM contents c \
         LEFT JOIN contents season ON season.id = c.parent_id \
         LEFT JOIN contents show ON show.id = season.parent_id \
         WHERE c.id = ?",
    )
    .bind(content_id)
    .fetch_optional(db)
    .await?;

    Ok(parts.map(|p| match (p.kind, p.show_title) {
        (ContentKind::Episode, Some(show)) => format!(
            "{show} · T{} E{}",
            p.season_number.unwrap_or_default(),
            p.number.unwrap_or_default()
        ),
        _ => p.title,
    }))
}

/// Carpeta de la biblioteca y ruta relativa de un archivo, para ubicarlo en el disco.
pub async fn file_location(
    db: &SqlitePool,
    file_id: i64,
) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as(
        "SELECT l.dir, f.rel_path FROM media_files f \
         JOIN contents c ON c.id = f.content_id \
         JOIN libraries l ON l.id = c.library_id \
         WHERE f.id = ?",
    )
    .bind(file_id)
    .fetch_optional(db)
    .await
}

/// Un contenido por id.
pub async fn find(conn: &mut SqliteConnection, id: i64) -> sqlx::Result<Option<Content>> {
    let sql = format!("SELECT {CONTENT_COLUMNS} FROM contents WHERE id = ?");
    sqlx::query_as(AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// El hijo con ese número (la temporada 2 de una serie, el episodio 5 de una temporada).
pub async fn child_with_number(
    conn: &mut SqliteConnection,
    parent_id: i64,
    number: i64,
) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar("SELECT id FROM contents WHERE parent_id = ? AND number = ?")
        .bind(parent_id)
        .bind(number)
        .fetch_optional(conn)
        .await
}

/// Los archivos de un contenido: id y ruta.
pub async fn file_ids_of(conn: &mut SqliteConnection, content_id: i64) -> sqlx::Result<Vec<i64>> {
    sqlx::query_scalar("SELECT id FROM media_files WHERE content_id = ? ORDER BY id")
        .bind(content_id)
        .fetch_all(conn)
        .await
}

/// Si algún archivo ya usa esa ruta.
pub async fn rel_path_taken(conn: &mut SqliteConnection, rel_path: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM media_files WHERE rel_path = ?)")
        .bind(rel_path)
        .fetch_one(conn)
        .await
}

/// Todo lo que un archivo ocupa en el disco: el video, sus .vtt y los .ass originales.
pub async fn paths_of_file(conn: &mut SqliteConnection, file_id: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT rel_path FROM media_files WHERE id = ?1 \
         UNION ALL SELECT rel_path FROM subtitles WHERE media_file_id = ?1 \
         UNION ALL SELECT original_rel_path FROM subtitles \
           WHERE media_file_id = ?1 AND original_rel_path IS NOT NULL",
    )
    .bind(file_id)
    .fetch_all(conn)
    .await
}

/// Borra un archivo de la base, con sus subtítulos. Los archivos del disco no se tocan.
pub async fn delete_media_file(conn: &mut SqliteConnection, file_id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM subtitles WHERE media_file_id = ?")
        .bind(file_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM media_files WHERE id = ?")
        .bind(file_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// La película o serie a la que pertenece un archivo (subiendo desde el episodio).
pub async fn top_of_file(db: &SqlitePool, file_id: i64) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar(
        "WITH RECURSIVE up(id, parent_id) AS ( \
           SELECT c.id, c.parent_id FROM contents c \
             JOIN media_files f ON f.content_id = c.id WHERE f.id = ? \
           UNION ALL \
           SELECT c.id, c.parent_id FROM contents c JOIN up ON c.id = up.parent_id \
         ) \
         SELECT id FROM up WHERE parent_id IS NULL",
    )
    .bind(file_id)
    .fetch_optional(db)
    .await
}

/// La carpeta de una biblioteca, relativa a la carpeta media.
pub async fn library_dir(db: &SqlitePool, library_id: i64) -> sqlx::Result<String> {
    sqlx::query_scalar("SELECT dir FROM libraries WHERE id = ?")
        .bind(library_id)
        .fetch_one(db)
        .await
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
