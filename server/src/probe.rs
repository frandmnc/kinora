//! Análisis de archivos de video con ffprobe y comparación con el formato de Kinora.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Lo que nos interesa de la salida de `ffprobe -show_format -show_streams`.
/// Los campos que faltan en el JSON quedan en `None` o vacíos.
#[derive(Debug, Deserialize)]
pub struct Probe {
    pub format: ProbeFormat,
    #[serde(default)]
    pub streams: Vec<ProbeStream>,
}

#[derive(Debug, Deserialize)]
pub struct ProbeFormat {
    pub format_name: String,
    pub duration: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProbeStream {
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub codec_tag_string: Option<String>,
    pub profile: Option<String>,
    pub level: Option<i64>,
    pub pix_fmt: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub channels: Option<i64>,
    /// En segundos. En MKV no viene: está en la etiqueta `DURATION`.
    pub duration: Option<String>,
    #[serde(default)]
    pub disposition: HashMap<String, i64>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
    #[serde(default)]
    pub side_data_list: Vec<serde_json::Value>,
}

/// Resultado de analizar un archivo: los datos interpretados y el JSON original para guardarlo.
pub struct Analysis {
    pub probe: Probe,
    pub json: String,
}

/// Ejecuta ffprobe sobre un archivo.
pub async fn run(path: &Path) -> Result<Analysis> {
    let output = tokio::process::Command::new("ffprobe")
        .args(["-v", "error", "-print_format", "json"])
        .args(["-show_format", "-show_streams"])
        .arg(path)
        .output()
        .await
        .context("no se pudo ejecutar ffprobe (¿está instalado?)")?;

    if !output.status.success() {
        bail!(
            "ffprobe falló: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let json = String::from_utf8(output.stdout).context("ffprobe devolvió texto inválido")?;
    let probe = Probe::parse(&json)?;
    Ok(Analysis { probe, json })
}

impl Probe {
    pub fn parse(json: &str) -> Result<Probe> {
        serde_json::from_str(json).context("no se pudo interpretar la salida de ffprobe")
    }

    pub fn duration(&self) -> Option<f64> {
        self.format.duration.as_deref()?.parse().ok()
    }

    /// Lo que dura la imagen y el sonido: la pista de video o audio más larga. Puede ser menos
    /// que `duration`, que en un MKV incluye subtítulos que terminan después del video.
    pub fn media_duration(&self) -> Option<f64> {
        self.streams
            .iter()
            .filter(|s| s.is("video") || s.is("audio"))
            .filter_map(ProbeStream::duration)
            .reduce(f64::max)
            .or_else(|| self.duration())
    }

    /// Primer video real: las carátulas incrustadas también figuran como streams de video.
    pub fn video(&self) -> Option<&ProbeStream> {
        self.streams
            .iter()
            .find(|s| s.is("video") && s.disposition.get("attached_pic") != Some(&1))
    }

    pub fn audios(&self) -> impl Iterator<Item = &ProbeStream> {
        self.streams.iter().filter(|s| s.is("audio"))
    }

    /// Qué hay que hacerle al archivo para que cumpla el formato de Kinora, y qué impide
    /// importarlo por ahora.
    pub fn plan(&self, path: &Path) -> FormatPlan {
        let mut changes = Vec::new();
        let mut blockers = Vec::new();
        let mut convert_video = false;
        let mut convert_audio = false;

        let mp4_extension = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
        if !mp4_extension || !self.format.format_name.contains("mp4") {
            let container = self.format.format_name.split(',').next().unwrap_or("?");
            changes.push(format!("contenedor {container} → MP4"));
        }

        match self.video() {
            None => blockers.push("no tiene video".to_string()),
            Some(video) => {
                let codec = video.codec_name.as_deref().unwrap_or("desconocido");
                if codec != "hevc" {
                    convert_video = true;
                    blockers.push(format!(
                        "el video es {codec}: convertirlo a HEVC llega en la Etapa 4"
                    ));
                } else if changes.is_empty() && video.codec_tag_string.as_deref() != Some("hvc1") {
                    // Solo importa en un MP4: al cambiar de caja, la etiqueta se pone igual.
                    let tag = video.codec_tag_string.as_deref().unwrap_or("desconocida");
                    changes.push(format!("etiqueta {tag} → hvc1"));
                }
                if let Some(profile) = video.dolby_vision_profile() {
                    blockers.push(format!("tiene Dolby Vision perfil {profile}"));
                }
            }
        }

        let mut has_audio = false;
        for audio in self.audios() {
            has_audio = true;
            let codec = audio.codec_name.as_deref().unwrap_or("desconocido");
            if codec != "aac" {
                convert_audio = true;
                changes.push(format!("audio {codec} → AAC"));
            }
        }
        if !has_audio {
            blockers.push("no tiene audio".to_string());
        }

        let operation = if convert_video {
            Operation::Video
        } else if convert_audio {
            Operation::Audio
        } else if !changes.is_empty() {
            Operation::Remux
        } else {
            Operation::Copy
        };
        FormatPlan {
            operation,
            changes,
            blockers,
        }
    }
}

/// La operación mínima para llevar un archivo al formato de Kinora.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, sqlx::Type, TS)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
#[ts(export)]
pub enum Operation {
    /// Ya cumple: se mueve tal cual.
    Copy,
    /// Video HEVC y audio AAC en otra caja (MKV) o con otra etiqueta: se cambia la caja, sin
    /// tocar la imagen ni el sonido. Tarda lo que tarda copiar el archivo.
    Remux,
    /// Como el remux, pero además el audio se convierte a AAC. Unos segundos más.
    Audio,
    /// El video no es HEVC: hay que convertirlo (Etapa 4).
    Video,
}

pub struct FormatPlan {
    pub operation: Operation,
    /// Lo que se va a cambiar, para mostrar: "contenedor matroska → MP4", "audio opus → AAC".
    pub changes: Vec<String>,
    /// Lo que impide importarlo por ahora.
    pub blockers: Vec<String>,
}

impl ProbeStream {
    fn is(&self, codec_type: &str) -> bool {
        self.codec_type.as_deref() == Some(codec_type)
    }

    pub fn dolby_vision_profile(&self) -> Option<i64> {
        self.side_data_list
            .iter()
            .find(|d| d["side_data_type"] == "DOVI configuration record")
            .and_then(|d| d["dv_profile"].as_i64())
    }

    /// Bits por color según el formato de píxel (`yuv420p10le` → 10).
    pub fn bit_depth(&self) -> i64 {
        match self.pix_fmt.as_deref() {
            Some(fmt) if fmt.contains("p10") => 10,
            Some(fmt) if fmt.contains("p12") => 12,
            _ => 8,
        }
    }

    /// Códec en el formato que entiende `video.canPlayType()` del navegador (RFC 6381).
    pub fn codec_string(&self) -> Option<String> {
        match (self.codec_tag_string.as_deref(), self.profile.as_deref()) {
            (Some("hvc1"), Some("Main")) => Some(format!("hvc1.1.6.L{}.B0", self.level?)),
            (Some("hvc1"), Some("Main 10")) => Some(format!("hvc1.2.4.L{}.B0", self.level?)),
            _ => self.codec_name.clone(),
        }
    }

    /// Segundos de esta pista: el campo `duration`, o la etiqueta `DURATION` de MKV ("00:02:30.935").
    fn duration(&self) -> Option<f64> {
        if let Some(seconds) = self.duration.as_deref().and_then(|d| d.parse().ok()) {
            return Some(seconds);
        }
        let tag = self.tags.get("DURATION")?;
        let mut parts = tag.splitn(3, ':');
        let hours: f64 = parts.next()?.parse().ok()?;
        let minutes: f64 = parts.next()?.parse().ok()?;
        let seconds: f64 = parts.next()?.parse().ok()?;
        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    }

    pub fn language(&self) -> Option<&str> {
        self.tags.get("language").map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINORA_MP4: &str = r#"{
        "format": { "format_name": "mov,mp4,m4a,3gp,3g2,mj2", "duration": "1559.98" },
        "streams": [
            { "codec_type": "video", "codec_name": "hevc", "codec_tag_string": "hvc1",
              "profile": "Main 10", "level": 120, "pix_fmt": "yuv420p10le", "width": 1920, "height": 1080 },
            { "codec_type": "audio", "codec_name": "aac", "channels": 2, "tags": { "language": "jpn" } }
        ]
    }"#;

    const ANIME_MKV: &str = r#"{
        "format": { "format_name": "matroska,webm", "duration": "1559.98" },
        "streams": [
            { "codec_type": "video", "codec_name": "h264", "profile": "High", "pix_fmt": "yuv420p" },
            { "codec_type": "audio", "codec_name": "aac" },
            { "codec_type": "subtitle", "codec_name": "ass" }
        ]
    }"#;

    #[test]
    fn kinora_format_has_no_problems() {
        let probe = Probe::parse(KINORA_MP4).unwrap();
        let plan = probe.plan(Path::new("a.mp4"));
        assert_eq!(plan.operation, Operation::Copy);
        assert!(plan.changes.is_empty() && plan.blockers.is_empty());
        assert_eq!(probe.duration(), Some(1559.98));
        let video = probe.video().unwrap();
        assert_eq!(video.bit_depth(), 10);
        assert_eq!(video.codec_string().unwrap(), "hvc1.2.4.L120.B0");
    }

    #[test]
    fn h264_needs_the_video_conversion() {
        let plan = Probe::parse(ANIME_MKV).unwrap().plan(Path::new("a.mkv"));
        assert_eq!(plan.operation, Operation::Video);
        assert_eq!(plan.changes, ["contenedor matroska → MP4"]);
        assert_eq!(
            plan.blockers,
            ["el video es h264: convertirlo a HEVC llega en la Etapa 4"]
        );
    }

    #[test]
    fn hevc_in_mkv_is_a_remux_and_opus_converts_the_audio() {
        let mkv = KINORA_MP4
            .replace("mov,mp4,m4a,3gp,3g2,mj2", "matroska,webm")
            .replace(r#""codec_tag_string": "hvc1","#, "");
        let plan = Probe::parse(&mkv).unwrap().plan(Path::new("a.mkv"));
        assert_eq!(plan.operation, Operation::Remux);
        assert_eq!(plan.changes, ["contenedor matroska → MP4"]);

        let opus = mkv.replace(r#""codec_name": "aac""#, r#""codec_name": "opus""#);
        let plan = Probe::parse(&opus).unwrap().plan(Path::new("a.mkv"));
        assert_eq!(plan.operation, Operation::Audio);
        assert_eq!(
            plan.changes,
            ["contenedor matroska → MP4", "audio opus → AAC"]
        );
        assert!(plan.blockers.is_empty());
    }

    #[test]
    fn media_duration_ignores_subtitles_that_end_later() {
        let mkv = r#"{
            "format": { "format_name": "matroska,webm", "duration": "153.010000" },
            "streams": [
                { "codec_type": "video", "tags": { "DURATION": "00:02:30.935000000" } },
                { "codec_type": "audio", "tags": { "DURATION": "00:02:31.932000000" } },
                { "codec_type": "subtitle", "tags": { "DURATION": "00:02:33.010000000" } }
            ]
        }"#;
        let probe = Probe::parse(mkv).unwrap();
        assert!((probe.media_duration().unwrap() - 151.932).abs() < 0.001);
    }

    #[test]
    fn detects_dolby_vision() {
        let json = KINORA_MP4.replace(
            r#""height": 1080"#,
            r#""height": 1080, "side_data_list": [{ "side_data_type": "DOVI configuration record", "dv_profile": 5 }]"#,
        );
        let probe = Probe::parse(&json).unwrap();
        assert_eq!(
            probe.plan(Path::new("a.mp4")).blockers,
            ["tiene Dolby Vision perfil 5"]
        );
    }
}
