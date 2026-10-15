//! Consultas para guardar metadata en `contents`.

use sqlx::{SqliteConnection, SqlitePool};

use crate::db::now;

pub use crate::contents::repo::child_with_number;

/// Lo que se guarda de TMDB en una película o serie.
pub struct TopMetadata<'a> {
    pub title: &'a str,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub tmdb_id: i64,
    pub poster_at: Option<i64>,
    pub backdrop_at: Option<i64>,
}

pub async fn save_top(
    conn: &mut SqliteConnection,
    id: i64,
    top: &TopMetadata<'_>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE contents SET title = ?, year = ?, overview = ?, tmdb_id = ?, metadata_at = ?, \
           poster_at = ?, backdrop_at = ? \
         WHERE id = ?",
    )
    .bind(top.title)
    .bind(top.year)
    .bind(&top.overview)
    .bind(top.tmdb_id)
    .bind(now())
    .bind(top.poster_at)
    .bind(top.backdrop_at)
    .bind(id)
    .execute(conn)
    .await?;
    Ok(())
}

/// El título y la sinopsis de una temporada o un episodio.
pub async fn save_text(
    conn: &mut SqliteConnection,
    id: i64,
    title: &str,
    overview: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE contents SET title = ?, overview = ? WHERE id = ?")
        .bind(title)
        .bind(overview)
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Corrección a mano: título, año y sinopsis.
pub async fn edit(
    db: &SqlitePool,
    id: i64,
    title: &str,
    year: Option<i64>,
    overview: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE contents SET title = ?, year = ?, overview = ? WHERE id = ?")
        .bind(title)
        .bind(year)
        .bind(overview)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Las temporadas de una serie: (id, número).
pub async fn seasons_of(db: &SqlitePool, show_id: i64) -> sqlx::Result<Vec<(i64, i64)>> {
    sqlx::query_as(
        "SELECT id, number FROM contents \
         WHERE parent_id = ? AND kind = 'season' AND number IS NOT NULL ORDER BY number",
    )
    .bind(show_id)
    .fetch_all(db)
    .await
}

/// La película o serie de un contenido (subiendo desde un episodio o una temporada).
pub async fn top_of(db: &SqlitePool, content_id: i64) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar(
        "WITH RECURSIVE up(id, parent_id) AS ( \
           SELECT id, parent_id FROM contents WHERE id = ? \
           UNION ALL \
           SELECT c.id, c.parent_id FROM contents c JOIN up ON c.id = up.parent_id \
         ) \
         SELECT id FROM up WHERE parent_id IS NULL",
    )
    .bind(content_id)
    .fetch_optional(db)
    .await
}
