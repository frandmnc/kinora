//! Encontrar, convertir y registrar subtítulos: los que vienen dentro del video al importarlo,
//! y los archivos que se agregan después desde el Admin.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use sqlx::SqlitePool;
use tracing::warn;

use super::SubtitleTrack;
use super::repo::{self, NewSubtitle};
use crate::probe::Probe;

/// Extensiones de archivos de subtítulos que se pueden agregar a un video.
pub const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt"];

/// Códecs de subtítulos de texto. Los de imagen (PGS de Blu-ray, DVD) no se pueden
/// convertir a texto sin reconocimiento de caracteres: por ahora se ignoran.
const TEXT_CODECS: &[&str] = &["subrip", "ass", "ssa", "mov_text", "webvtt", "text"];

/// Una pista encontrada, todavía sin convertir.
#[derive(Debug, PartialEq)]
pub struct FoundTrack {
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub forced: bool,
    pub source: TrackSource,
}

#[derive(Debug, PartialEq)]
pub enum TrackSource {
    /// Dentro del video: el número de pista entre los subtítulos (`-map 0:s:N`) y su códec.
    Embedded { index: usize, codec: String },
    /// Un archivo subido desde el Admin. Se consume: después de convertirlo ya no está.
    File { path: PathBuf, extension: String },
}

/// Las pistas de texto dentro del video.
pub fn find_tracks(probe: &Probe) -> Vec<FoundTrack> {
    probe
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("subtitle"))
        .enumerate()
        .filter_map(|(index, stream)| {
            let codec = stream.codec_name.clone().unwrap_or_default();
            TEXT_CODECS.contains(&codec.as_str()).then(|| FoundTrack {
                language: stream.language().map(str::to_string),
                title: stream.tags.get("title").cloned(),
                is_default: stream.disposition.get("default") == Some(&1),
                forced: stream.disposition.get("forced") == Some(&1),
                source: TrackSource::Embedded { index, codec },
            })
        })
        .collect()
}

/// Cuántas pistas de subtítulos de imagen tiene el video (se avisan, pero no se usan).
fn image_tracks(probe: &Probe) -> usize {
    probe
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("subtitle"))
        .filter(|s| !TEXT_CODECS.contains(&s.codec_name.as_deref().unwrap_or("")))
        .count()
}

/// Descripción corta para la pantalla de análisis: "Español (ASS)", "Inglés (SRT)".
pub fn describe(probe: &Probe) -> Vec<String> {
    let mut lines: Vec<String> = find_tracks(probe)
        .iter()
        .map(|track| {
            let (_, name) = language_info(track.language.as_deref());
            let TrackSource::Embedded { codec, .. } = &track.source else {
                unreachable!("find_tracks solo devuelve pistas internas")
            };
            format!("{name} ({})", codec_name(codec))
        })
        .collect();
    let images = image_tracks(probe);
    if images > 0 {
        lines.push(format!("{images} de imagen (no se usan todavía)"));
    }
    lines
}

/// Convierte cada pista a WebVTT al lado del video ya importado y la registra en la base.
/// `source` es el archivo del que salen las pistas internas: el video mismo, o el original si
/// hubo remux (el MP4 no lleva los subtítulos del MKV).
/// Si una pista falla, se anota en la terminal y se sigue con las demás.
/// Devuelve cuántas quedaron.
pub async fn attach_subtitles(
    db: &SqlitePool,
    root: &Path,
    media_file_id: i64,
    video_rel: &str,
    source: &Path,
    tracks: Vec<FoundTrack>,
) -> usize {
    let mut attached = 0;
    for track in tracks {
        match attach(db, root, media_file_id, video_rel, source, track).await {
            Ok(_) => attached += 1,
            Err(err) => warn!("could not add subtitle to {video_rel}: {err:#}"),
        }
    }
    attached
}

/// Agrega un archivo de subtítulos subido desde el Admin a un video ya importado.
/// `path` es el archivo subido (se consume); `language`, el código ISO 639 elegido.
pub async fn attach_file(
    db: &SqlitePool,
    root: &Path,
    media_file_id: i64,
    video_rel: &str,
    path: PathBuf,
    language: Option<String>,
    forced: bool,
) -> Result<SubtitleTrack> {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    // ffmpeg espera UTF-8; muchos .srt en castellano vienen en Latin-1 (Windows-1252).
    let bytes = tokio::fs::read(&path).await?;
    if let Cow::Owned(utf8) = to_utf8(&bytes) {
        tokio::fs::write(&path, utf8).await?;
    }
    let track = FoundTrack {
        language,
        title: None,
        is_default: false,
        forced,
        source: TrackSource::File { path, extension },
    };
    let id = attach(
        db,
        root,
        media_file_id,
        video_rel,
        &root.join(video_rel),
        track,
    )
    .await?;
    repo::get(db, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("subtitle {id} vanished after insert"))
}

/// Convierte una pista a `<video>.<idioma>.vtt` y la registra. Devuelve el id.
async fn attach(
    db: &SqlitePool,
    root: &Path,
    media_file_id: i64,
    video_rel: &str,
    source: &Path,
    track: FoundTrack,
) -> Result<i64> {
    let base = video_rel.strip_suffix(".mp4").unwrap_or(video_rel);
    let language = track.language.clone().unwrap_or_else(|| "und".to_string());
    // Base.spa.vtt, y si se repite el idioma: Base.spa.2.vtt, Base.spa.3.vtt…
    let name = (1..)
        .map(|n| match n {
            1 => format!("{base}.{language}"),
            n => format!("{base}.{language}.{n}"),
        })
        .find(|name| {
            !root.join(format!("{name}.vtt")).exists() && !root.join(format!("{name}.ass")).exists()
        })
        .expect("siempre hay un nombre libre");

    let original = convert(source, root, &name, &track.source).await?;
    let (srclang, _) = language_info(track.language.as_deref());
    // Dos pistas "Español" no se distinguen en el menú: la segunda pasa a "Español 2".
    let taken = repo::labels_of(db, media_file_id).await?;
    let label = unique_label(label(&track), &taken);
    let subtitle = NewSubtitle {
        media_file_id,
        rel_path: format!("{name}.vtt"),
        original_rel_path: original,
        language: track.language.clone(),
        srclang,
        label,
        is_default: track.is_default,
        forced: track.forced,
    };
    Ok(repo::insert(db, &subtitle).await?)
}

/// Genera `<name>.vtt` y, si la pista es ASS, conserva el original como `<name>.ass`.
/// Devuelve la ruta relativa del original conservado.
async fn convert(
    video: &Path,
    root: &Path,
    name: &str,
    source: &TrackSource,
) -> Result<Option<String>> {
    let vtt = root.join(format!("{name}.vtt"));
    let ass = root.join(format!("{name}.ass"));
    let ass_rel = Some(format!("{name}.ass"));

    match source {
        TrackSource::Embedded { index, codec } => {
            let map = format!("0:s:{index}");
            ffmpeg(&[
                &video.to_string_lossy(),
                "-map",
                &map,
                "-c:s",
                "webvtt",
                &vtt.to_string_lossy(),
            ])
            .await?;
            if codec == "ass" || codec == "ssa" {
                ffmpeg(&[
                    &video.to_string_lossy(),
                    "-map",
                    &map,
                    "-c:s",
                    "copy",
                    &ass.to_string_lossy(),
                ])
                .await?;
                return Ok(ass_rel);
            }
            Ok(None)
        }
        TrackSource::File { path, extension } => {
            if extension == "vtt" {
                tokio::fs::copy(path, &vtt).await?;
            } else {
                ffmpeg(&[&path.to_string_lossy(), &vtt.to_string_lossy()]).await?;
            }
            // El ASS se conserva junto al video (copiado: el temporal puede estar en otro disco).
            let keep = extension == "ass" || extension == "ssa";
            if keep {
                tokio::fs::copy(path, &ass).await?;
            }
            tokio::fs::remove_file(path).await?;
            Ok(keep.then_some(ass_rel).flatten())
        }
    }
}

/// `ffmpeg -i <entrada> <argumentos…>`. El primer elemento es la entrada.
async fn ffmpeg(args: &[&str]) -> Result<()> {
    let (input, rest) = args.split_first().expect("falta la entrada");
    let output = tokio::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i", input])
        .args(rest)
        .output()
        .await?;
    if !output.status.success() {
        bail!(
            "ffmpeg falló: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// El texto en UTF-8. Si no lo es, se asume Windows-1252 (Latin-1 con comillas y guiones
/// tipográficos), que es como suelen venir los .srt viejos en castellano.
fn to_utf8(bytes: &[u8]) -> Cow<'_, [u8]> {
    if std::str::from_utf8(bytes).is_ok() {
        return Cow::Borrowed(bytes);
    }
    // 0x80–0x9F son los únicos bytes en los que Windows-1252 difiere de Latin-1.
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    let text: String = bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9f => HIGH[usize::from(b - 0x80)],
            _ => char::from(b),
        })
        .collect();
    Cow::Owned(text.into_bytes())
}

/// `label` si nadie lo usa; si no, `label 2`, `label 3`…
fn unique_label(label: String, taken: &[String]) -> String {
    if !taken.contains(&label) {
        return label;
    }
    (2..)
        .map(|n| format!("{label} {n}"))
        .find(|candidate| !taken.contains(candidate))
        .expect("siempre hay un nombre libre")
}

/// Nombre para mostrar: el idioma, y el título de la pista si dice algo más ("Español · Latin America").
fn label(track: &FoundTrack) -> String {
    let (_, name) = language_info(track.language.as_deref());
    let mut label = name.to_string();
    if let Some(title) = &track.title {
        let title = title
            .replace('_', " ")
            .replace('(', " (")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let only_language = [
            "spanish",
            "english",
            "portuguese",
            "japanese",
            "french",
            "german",
            "italian",
            "russian",
            "arabic",
        ]
        .iter()
        .any(|word| title.eq_ignore_ascii_case(word));
        if !title.is_empty() && !only_language && !title.eq_ignore_ascii_case(name) {
            label = format!("{label} · {title}");
        }
    }
    if track.forced {
        label.push_str(" (forzados)");
    }
    label
}

/// Código para el navegador y nombre en castellano de un código de idioma ISO 639.
pub fn language_info(code: Option<&str>) -> (Option<String>, &'static str) {
    let code = code.unwrap_or("").to_lowercase();
    let (short, name) = match code.as_str() {
        "spa" | "es" | "esp" => ("es", "Español"),
        "eng" | "en" => ("en", "Inglés"),
        "por" | "pt" => ("pt", "Portugués"),
        "jpn" | "ja" | "jp" => ("ja", "Japonés"),
        "fre" | "fra" | "fr" => ("fr", "Francés"),
        "ger" | "deu" | "de" => ("de", "Alemán"),
        "ita" | "it" => ("it", "Italiano"),
        "rus" | "ru" => ("ru", "Ruso"),
        "ara" | "ar" => ("ar", "Árabe"),
        "chi" | "zho" | "zh" => ("zh", "Chino"),
        "kor" | "ko" => ("ko", "Coreano"),
        "" | "und" => return (None, "Subtítulos"),
        _ => return (Some(code), "Otro idioma"),
    };
    (Some(short.to_string()), name)
}

fn codec_name(codec: &str) -> &str {
    match codec {
        "subrip" => "SRT",
        "ass" | "ssa" => "ASS",
        "mov_text" => "MP4",
        "webvtt" => "VTT",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_labels_get_a_number() {
        let taken = ["Español".to_string(), "Español 2".to_string()];
        assert_eq!(unique_label("Inglés".into(), &taken), "Inglés");
        assert_eq!(unique_label("Español".into(), &taken), "Español 3");
    }

    #[test]
    fn reads_latin1_subtitles() {
        assert!(matches!(to_utf8("Ñandú".as_bytes()), Cow::Borrowed(_)));
        let latin1 = b"\xbfQu\xe9 pas\xf3? \x93Nada\x94";
        assert_eq!(
            String::from_utf8(to_utf8(latin1).into_owned()).unwrap(),
            "¿Qué pasó? “Nada”"
        );
    }

    #[test]
    fn labels_in_spanish() {
        let track = |language: &str, title: Option<&str>, forced| FoundTrack {
            language: Some(language.to_string()),
            title: title.map(str::to_string),
            is_default: false,
            forced,
            source: TrackSource::Embedded {
                index: 0,
                codec: "ass".into(),
            },
        };
        assert_eq!(
            label(&track("spa", Some("Spanish(Latin_America)"), false)),
            "Español · Spanish (Latin America)"
        );
        assert_eq!(label(&track("spa", Some("Spanish"), false)), "Español");
        assert_eq!(label(&track("eng", None, true)), "Inglés (forzados)");
        assert_eq!(language_info(Some("spa")).0.as_deref(), Some("es"));
    }
}
