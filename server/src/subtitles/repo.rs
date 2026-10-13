//! Consultas a la tabla `subtitles`.

use sqlx::{AssertSqlSafe, SqlitePool};

use super::SubtitleTrack;

pub struct NewSubtitle {
    pub media_file_id: i64,
    pub rel_path: String,
    pub original_rel_path: Option<String>,
    pub language: Option<String>,
    pub srclang: Option<String>,
    pub label: String,
    pub is_default: bool,
    pub forced: bool,
    pub source_name: Option<String>,
}

pub async fn insert(db: &SqlitePool, new: &NewSubtitle) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO subtitles \
         (media_file_id, rel_path, original_rel_path, language, srclang, label, is_default, forced, source_name) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(new.media_file_id)
    .bind(&new.rel_path)
    .bind(&new.original_rel_path)
    .bind(&new.language)
    .bind(&new.srclang)
    .bind(&new.label)
    .bind(new.is_default)
    .bind(new.forced)
    .bind(&new.source_name)
    .fetch_one(db)
    .await
}

/// Los subtítulos de varios archivos, en el orden en que se importaron.
pub async fn for_files(db: &SqlitePool, file_ids: &[i64]) -> sqlx::Result<Vec<SubtitleTrack>> {
    if file_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; file_ids.len()].join(", ");
    let sql = format!(
        "SELECT id, media_file_id, language, srclang, label, is_default, forced, source_name \
         FROM subtitles WHERE media_file_id IN ({placeholders}) ORDER BY id"
    );
    let mut query = sqlx::query_as(AssertSqlSafe(sql));
    for id in file_ids {
        query = query.bind(id);
    }
    query.fetch_all(db).await
}

/// Carpeta de la biblioteca y ruta relativa del .vtt, para ubicarlo en el disco.
pub async fn location(db: &SqlitePool, id: i64) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as(
        "SELECT l.dir, s.rel_path FROM subtitles s \
         JOIN media_files f ON f.id = s.media_file_id \
         JOIN contents c ON c.id = f.content_id \
         JOIN libraries l ON l.id = c.library_id \
         WHERE s.id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

/// Los nombres que ya usan los subtítulos de un archivo.
pub async fn labels_of(db: &SqlitePool, media_file_id: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT label FROM subtitles WHERE media_file_id = ?")
        .bind(media_file_id)
        .fetch_all(db)
        .await
}

/// Cambia el nombre. Devuelve `false` si no existe.
pub async fn rename(db: &SqlitePool, id: i64, label: &str) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE subtitles SET label = ? WHERE id = ?")
        .bind(label)
        .bind(id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn get(db: &SqlitePool, id: i64) -> sqlx::Result<Option<SubtitleTrack>> {
    sqlx::query_as(
        "SELECT id, media_file_id, language, srclang, label, is_default, forced, source_name \
         FROM subtitles WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

/// Lo que hay que borrar del disco al quitar un subtítulo.
pub struct SubtitleFiles {
    pub media_file_id: i64,
    pub library_dir: String,
    pub rel_path: String,
    pub original_rel_path: Option<String>,
}

/// Borra el registro y devuelve sus archivos, o `None` si no existía.
pub async fn delete(db: &SqlitePool, id: i64) -> sqlx::Result<Option<SubtitleFiles>> {
    let found: Option<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT s.media_file_id, l.dir, s.rel_path, s.original_rel_path FROM subtitles s \
         JOIN media_files f ON f.id = s.media_file_id \
         JOIN contents c ON c.id = f.content_id \
         JOIN libraries l ON l.id = c.library_id \
         WHERE s.id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    let Some((media_file_id, library_dir, rel_path, original_rel_path)) = found else {
        return Ok(None);
    };
    sqlx::query("DELETE FROM subtitles WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(Some(SubtitleFiles {
        media_file_id,
        library_dir,
        rel_path,
        original_rel_path,
    }))
}
