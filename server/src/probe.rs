//! Análisis de archivos de video con ffprobe y comparación con el formato de Kinora.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

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

    /// Primer video real: las carátulas incrustadas también figuran como streams de video.
    pub fn video(&self) -> Option<&ProbeStream> {
        self.streams
            .iter()
            .find(|s| s.is("video") && s.disposition.get("attached_pic") != Some(&1))
    }

    pub fn audios(&self) -> impl Iterator<Item = &ProbeStream> {
        self.streams.iter().filter(|s| s.is("audio"))
    }

    /// Diferencias con el formato de Kinora. Si está vacía, el archivo se puede copiar tal cual.
    pub fn problems(&self, path: &Path) -> Vec<String> {
        let mut problems = Vec::new();

        let mp4_extension = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
        if !mp4_extension || !self.format.format_name.contains("mp4") {
            let container = self.format.format_name.split(',').next().unwrap_or("?");
            problems.push(format!("el contenedor es {container}, no MP4"));
        }

        match self.video() {
            None => problems.push("no tiene video".to_string()),
            Some(video) => {
                let codec = video.codec_name.as_deref().unwrap_or("desconocido");
                if codec != "hevc" {
                    problems.push(format!("el video es {codec}, no HEVC"));
                } else if video.codec_tag_string.as_deref() != Some("hvc1") {
                    let tag = video.codec_tag_string.as_deref().unwrap_or("desconocida");
                    problems.push(format!("el video HEVC tiene la etiqueta {tag}, no hvc1"));
                }
                if let Some(profile) = video.dolby_vision_profile() {
                    problems.push(format!("tiene Dolby Vision perfil {profile}"));
                }
            }
        }

        let mut has_audio = false;
        for audio in self.audios() {
            has_audio = true;
            let codec = audio.codec_name.as_deref().unwrap_or("desconocido");
            if codec != "aac" {
                problems.push(format!("el audio es {codec}, no AAC"));
            }
        }
        if !has_audio {
            problems.push("no tiene audio".to_string());
        }

        problems
    }
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
        assert!(probe.problems(Path::new("a.mp4")).is_empty());
        assert_eq!(probe.duration(), Some(1559.98));
        let video = probe.video().unwrap();
        assert_eq!(video.bit_depth(), 10);
        assert_eq!(video.codec_string().unwrap(), "hvc1.2.4.L120.B0");
    }

    #[test]
    fn mkv_with_h264_lists_each_problem() {
        let probe = Probe::parse(ANIME_MKV).unwrap();
        assert_eq!(
            probe.problems(Path::new("a.mkv")),
            [
                "el contenedor es matroska, no MP4",
                "el video es h264, no HEVC"
            ]
        );
    }

    #[test]
    fn detects_dolby_vision() {
        let json = KINORA_MP4.replace(
            r#""height": 1080"#,
            r#""height": 1080, "side_data_list": [{ "side_data_type": "DOVI configuration record", "dv_profile": 5 }]"#,
        );
        let probe = Probe::parse(&json).unwrap();
        assert_eq!(
            probe.problems(Path::new("a.mp4")),
            ["tiene Dolby Vision perfil 5"]
        );
    }
}
