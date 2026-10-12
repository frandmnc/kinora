//! Consultas genéricas sobre cualquier tabla.
//!
//! El nombre de una tabla no se puede pasar con `bind`, así que se arma el SQL con texto.
//! Es seguro porque los handlers solo aceptan nombres que devuelve `table_names`,
//! y además `quote` escapa el nombre.

use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row, SqlitePool, TypeInfo, ValueRef};

use super::ColumnInfo;

pub async fn table_names(db: &SqlitePool) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT name FROM sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(db)
    .await
}

pub async fn count_rows(db: &SqlitePool, table: &str) -> sqlx::Result<i64> {
    let sql = format!("SELECT COUNT(*) FROM {}", quote(table));
    sqlx::query_scalar(AssertSqlSafe(sql)).fetch_one(db).await
}

pub async fn columns(db: &SqlitePool, table: &str) -> sqlx::Result<Vec<ColumnInfo>> {
    sqlx::query_as("SELECT name, type AS sql_type FROM pragma_table_info(?) ORDER BY cid")
        .bind(table)
        .fetch_all(db)
        .await
}

pub async fn rows(
    db: &SqlitePool,
    table: &str,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<Vec<Value>>> {
    let sql = format!("SELECT * FROM {} LIMIT ? OFFSET ?", quote(table));
    let rows = sqlx::query(AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    rows.iter().map(row_to_json).collect()
}

/// Convierte una fila de cualquier tabla en valores JSON, según el tipo de cada valor.
fn row_to_json(row: &SqliteRow) -> sqlx::Result<Vec<Value>> {
    (0..row.len())
        .map(|i| {
            let raw = row.try_get_raw(i)?;
            if raw.is_null() {
                return Ok(Value::Null);
            }
            let value = match raw.type_info().name() {
                "INTEGER" | "BOOLEAN" => Value::from(row.try_get_unchecked::<i64, _>(i)?),
                "REAL" => Value::from(row.try_get_unchecked::<f64, _>(i)?),
                "BLOB" => {
                    let bytes: Vec<u8> = row.try_get_unchecked(i)?;
                    Value::from(format!("<{} bytes>", bytes.len()))
                }
                // SQLite convierte cualquier otro valor a texto.
                _ => Value::from(row.try_get_unchecked::<String, _>(i)?),
            };
            Ok(value)
        })
        .collect()
}

/// Encierra un nombre entre comillas dobles, escapando las que tenga adentro.
fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn reads_any_table_as_json() {
        let db = connect_in_memory().await;
        sqlx::query("INSERT INTO libraries (name, dir) VALUES ('Anime', 'Anime')")
            .execute(&db)
            .await
            .unwrap();

        assert!(
            table_names(&db)
                .await
                .unwrap()
                .contains(&"libraries".to_string())
        );
        let names: Vec<String> = columns(&db, "libraries")
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, ["id", "name", "dir"]);
        assert_eq!(
            rows(&db, "libraries", 10, 0).await.unwrap(),
            vec![vec![
                Value::from(1),
                Value::from("Anime"),
                Value::from("Anime")
            ]]
        );
    }

    #[test]
    fn quote_escapes_double_quotes() {
        assert_eq!(quote(r#"a"b"#), r#""a""b""#);
    }
}
