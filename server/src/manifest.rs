//! `kinora.json`: una copia del catálogo de cada película o serie, guardada en su carpeta.
//!
//! La base de datos es la fuente de verdad; este archivo es su respaldo en el disco. Dice qué es
//! el contenido (tipo, título, año, temporadas, episodios) y qué archivos lo reproducen, con sus
//! subtítulos. Si se pierde la base, el catálogo se puede reconstruir leyendo estos archivos
//! (Etapa 5). Las rutas son relativas a la carpeta del contenido, así sobreviven a que se mueva
//! la carpeta media o se renombre la biblioteca.
//!
//! Lo que se puede volver a calcular (ffprobe, tamaños) no se guarda. El progreso y el
//! historial tampoco: son de quien mira, no del catálogo.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::contents::{Content, ContentKind, MediaFile, repo};

pub const FILE_NAME: &str = "kinora.json";

/// Versión del formato. Si cambia, la reconstrucción sabe cómo leer los viejos.
const VERSION: u32 = 1;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub kinora: u32,
    pub kind: ContentKind,
    pub title: String,
    pub year: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    /// Solo películas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<ManifestFile>,
    /// Solo series.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<ManifestSeason>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestSeason {
    pub number: i64,
    pub title: String,
    pub episodes: Vec<ManifestEpisode>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestEpisode {
    pub number: i64,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    /// Vacía si el episodio existe pero todavía no tiene archivo.
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestFile {
    /// Relativa a la carpeta del contenido: "Season 01/Frieren (2023) - S01E01.mp4".
    pub path: String,
    /// Nombre del archivo del que se importó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Segundos desde 1970 (UTC).
    pub imported_at: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtitles: Vec<ManifestSubtitle>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct ManifestSubtitle {
    /// El .vtt, relativo a la carpeta del contenido.
    pub path: String,
    /// El .ass original, si lo había.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    pub language: Option<String>,
    pub label: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub forced: bool,
    /// El archivo que se subió; no está si venía dentro del video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Reescribe el `kinora.json` de una película o serie. Si falla, lo anota en la terminal y
/// sigue: el catálogo ya está guardado en la base, el archivo es solo el respaldo.
pub async fn refresh(db: &SqlitePool, media_dir: &Path, top_id: i64) {
    if let Err(err) = write(db, media_dir, top_id).await {
        warn!("could not write {FILE_NAME} for content {top_id}: {err:#}");
    }
}

/// Igual que `refresh`, a partir de un archivo (después de agregarle o quitarle subtítulos).
pub async fn refresh_for_file(db: &SqlitePool, media_dir: &Path, file_id: i64) {
    match repo::top_of_file(db, file_id).await {
        Ok(Some(top_id)) => refresh(db, media_dir, top_id).await,
        Ok(None) => {}
        Err(err) => warn!("could not find the content of file {file_id}: {err}"),
    }
}

/// Reescribe todos. Se usa al arrancar: así los contenidos importados antes de que existiera
/// `kinora.json`, o con un formato viejo, quedan al día.
pub async fn refresh_all(db: &SqlitePool, media_dir: &Path) {
    let tops = match repo::list_top_level(db, None, None).await {
        Ok(tops) => tops,
        Err(err) => return warn!("could not list contents to write {FILE_NAME}: {err}"),
    };
    let mut written = 0;
    for top in &tops {
        match write(db, media_dir, top.id).await {
            Ok(()) => written += 1,
            Err(err) => warn!("could not write {FILE_NAME} for \"{}\": {err:#}", top.title),
        }
    }
    if written > 0 {
        info!("{FILE_NAME} up to date for {written} title(s)");
    }
}

async fn write(db: &SqlitePool, media_dir: &Path, top_id: i64) -> Result<()> {
    let (library_dir, manifest, dir) = build(db, top_id).await?;
    let folder = media_dir.join(library_dir).join(dir);
    if !folder.is_dir() {
        // Un contenido sin carpeta todavía (por ejemplo, si falló mover todos sus archivos).
        return Ok(());
    }
    let mut json = serde_json::to_string_pretty(&manifest)?;
    json.push('\n');
    // Primero a un temporal y después se renombra: nunca queda un kinora.json a medias.
    let tmp = folder.join(format!(".{FILE_NAME}.tmp"));
    tokio::fs::write(&tmp, json).await?;
    tokio::fs::rename(&tmp, folder.join(FILE_NAME)).await?;
    Ok(())
}

/// Arma el manifiesto desde la base. Devuelve también la carpeta de la biblioteca y la del contenido.
async fn build(db: &SqlitePool, top_id: i64) -> Result<(String, Manifest, String)> {
    let contents = repo::with_descendants(db, top_id).await?;
    let ids: Vec<i64> = contents.iter().map(|c| c.id).collect();
    let top = contents
        .iter()
        .find(|c| c.id == top_id)
        .ok_or_else(|| anyhow!("content {top_id} does not exist"))?;
    let dir = top
        .dir
        .clone()
        .ok_or_else(|| anyhow!("content {top_id} has no folder"))?;
    let library_dir = repo::library_dir(db, top.library_id).await?;

    // El prefijo "<carpeta del contenido>/" se saca de todas las rutas.
    let prefix = format!("{dir}/");
    let relative = |path: &str| path.strip_prefix(&prefix).unwrap_or(path).to_string();

    let mut subtitles: HashMap<i64, Vec<ManifestSubtitle>> = HashMap::new();
    for (file_id, mut subtitle) in subtitle_rows(db, &ids).await? {
        subtitle.path = relative(&subtitle.path);
        subtitle.original = subtitle.original.as_deref().map(relative);
        subtitles.entry(file_id).or_default().push(subtitle);
    }
    let mut files: HashMap<i64, Vec<ManifestFile>> = HashMap::new();
    for file in repo::files_of(db, &ids).await? {
        let MediaFile {
            id,
            content_id,
            rel_path,
            source_path,
            imported_at,
            ..
        } = file;
        files.entry(content_id).or_default().push(ManifestFile {
            path: relative(&rel_path),
            source: source_path,
            imported_at,
            subtitles: subtitles.remove(&id).unwrap_or_default(),
        });
    }

    let children_of = |parent: i64| {
        let mut kids: Vec<&Content> = contents
            .iter()
            .filter(|c| c.parent_id == Some(parent))
            .collect();
        kids.sort_by_key(|c| c.number);
        kids
    };
    let seasons = children_of(top_id)
        .into_iter()
        .filter(|s| s.kind == ContentKind::Season)
        .map(|season| ManifestSeason {
            number: season.number.unwrap_or_default(),
            title: season.title.clone(),
            episodes: children_of(season.id)
                .into_iter()
                .map(|episode| ManifestEpisode {
                    number: episode.number.unwrap_or_default(),
                    title: episode.title.clone(),
                    overview: episode.overview.clone(),
                    files: files.remove(&episode.id).unwrap_or_default(),
                })
                .collect(),
        })
        .collect();

    let manifest = Manifest {
        kinora: VERSION,
        kind: top.kind,
        title: top.title.clone(),
        year: top.year,
        overview: top.overview.clone(),
        files: files.remove(&top_id).unwrap_or_default(),
        seasons,
    };
    Ok((library_dir, manifest, dir))
}

/// Los subtítulos de los archivos de estos contenidos, con el id del archivo al que pertenecen.
async fn subtitle_rows(
    db: &SqlitePool,
    content_ids: &[i64],
) -> Result<Vec<(i64, ManifestSubtitle)>> {
    if content_ids.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(sqlx::FromRow)]
    struct Row {
        media_file_id: i64,
        #[sqlx(flatten)]
        subtitle: ManifestSubtitle,
    }
    let placeholders = vec!["?"; content_ids.len()].join(", ");
    let sql = format!(
        "SELECT s.media_file_id, s.rel_path AS path, s.original_rel_path AS original, \
                s.language, s.label, s.is_default AS \"default\", s.forced, \
                s.source_name AS source \
         FROM subtitles s JOIN media_files f ON f.id = s.media_file_id \
         WHERE f.content_id IN ({placeholders}) ORDER BY s.id"
    );
    let mut query = sqlx::query_as::<_, Row>(sqlx::AssertSqlSafe(sql));
    for id in content_ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(db).await.context("reading subtitles")?;
    Ok(rows
        .into_iter()
        .map(|r| (r.media_file_id, r.subtitle))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contents::repo::{NewContent, NewMediaFile, insert_content, insert_media_file};
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn describes_a_show_with_paths_relative_to_its_folder() {
        let db = connect_in_memory().await;
        let mut conn = db.acquire().await.unwrap();
        sqlx::query("INSERT INTO libraries (name, dir) VALUES ('Anime', 'Anime')")
            .execute(&mut *conn)
            .await
            .unwrap();
        let new = |kind, parent_id, number, title: &str, dir: Option<&str>| NewContent {
            library_id: 1,
            kind,
            parent_id,
            number,
            title: title.to_string(),
            year: dir.map(|_| 2023),
            dir: dir.map(str::to_string),
        };
        let show = insert_content(
            &mut conn,
            &new(
                ContentKind::Show,
                None,
                None,
                "Frieren",
                Some("Frieren (2023)"),
            ),
        )
        .await
        .unwrap();
        let season = insert_content(
            &mut conn,
            &new(
                ContentKind::Season,
                Some(show),
                Some(1),
                "Temporada 1",
                None,
            ),
        )
        .await
        .unwrap();
        let first = insert_content(
            &mut conn,
            &new(
                ContentKind::Episode,
                Some(season),
                Some(1),
                "Episodio 1",
                None,
            ),
        )
        .await
        .unwrap();
        insert_content(
            &mut conn,
            &new(
                ContentKind::Episode,
                Some(season),
                Some(2),
                "Episodio 2",
                None,
            ),
        )
        .await
        .unwrap();
        let file = insert_media_file(
            &mut conn,
            &NewMediaFile {
                content_id: first,
                rel_path: "Frieren (2023)/Season 01/Frieren (2023) - S01E01.mp4".into(),
                size: 1,
                duration: None,
                video_codec: None,
                width: None,
                height: None,
                probe_json: "{}".into(),
                source_path: "[Erai-raws] Frieren - 01.mkv".into(),
            },
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO subtitles (media_file_id, rel_path, original_rel_path, language, label) \
             VALUES (?, 'Frieren (2023)/Season 01/Frieren (2023) - S01E01.spa.vtt', \
                     'Frieren (2023)/Season 01/Frieren (2023) - S01E01.spa.ass', 'spa', 'Español')",
        )
        .bind(file)
        .execute(&mut *conn)
        .await
        .unwrap();
        drop(conn);

        let (library_dir, manifest, dir) = build(&db, show).await.unwrap();
        assert_eq!(
            (library_dir.as_str(), dir.as_str()),
            ("Anime", "Frieren (2023)")
        );
        let episodes = &manifest.seasons[0].episodes;
        assert_eq!(episodes.len(), 2);
        assert!(episodes[1].files.is_empty());
        let file = &episodes[0].files[0];
        assert_eq!(file.path, "Season 01/Frieren (2023) - S01E01.mp4");
        assert_eq!(file.source.as_deref(), Some("[Erai-raws] Frieren - 01.mkv"));
        assert_eq!(
            file.subtitles[0].path,
            "Season 01/Frieren (2023) - S01E01.spa.vtt"
        );
        assert_eq!(
            file.subtitles[0].original.as_deref(),
            Some("Season 01/Frieren (2023) - S01E01.spa.ass")
        );

        // Lo que se escribe se puede volver a leer igual (para la reconstrucción).
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&json).unwrap(), manifest);
    }
}
