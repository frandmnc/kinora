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
    /// Bits por segundo de todo el archivo.
    pub bit_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProbeStream {
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub codec_tag_string: Option<String>,
    pub profile: Option<String>,
    pub level: Option<i64>,
    pub pix_fmt: Option<String>,
    /// Función de transferencia: `bt709` (SDR), `smpte2084` (HDR10/PQ), `arib-std-b67` (HLG)…
    pub color_transfer: Option<String>,
    pub color_primaries: Option<String>,
    pub color_space: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Cuadros por segundo como fracción: "24000/1001" (23,976).
    pub avg_frame_rate: Option<String>,
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
                    if video.is_hdr() {
                        // Kinora nunca recrea HDR: convertirlo bien es otro proyecto.
                        blockers.push(format!(
                            "es HDR en {codec}: Kinora solo conserva HDR que ya viene en HEVC"
                        ));
                    } else {
                        changes.push(format!("video {codec} → HEVC 10 bits"));
                    }
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

/// Lo técnico de un archivo, para mostrar en el Admin.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct TechInfo {
    /// "2160p", "1080p", "720p"…
    pub resolution: Option<String>,
    /// "HEVC", "H.264"…
    pub video_codec: Option<String>,
    /// "Main 10", "High"…
    pub video_profile: Option<String>,
    pub bit_depth: Option<i64>,
    /// "SDR", "HDR10", "HLG", "Dolby Vision 8".
    pub dynamic_range: Option<String>,
    /// Cuadros por segundo (23,976, 25, 60…).
    pub frame_rate: Option<f64>,
    /// Bits por segundo de todo el archivo (video y audio).
    pub bitrate: Option<i64>,
    pub audio: Vec<AudioInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct AudioInfo {
    /// "AAC", "AC3", "Opus"…
    pub codec: String,
    /// "2.0", "5.1", "7.1"…
    pub channels: String,
    /// Código ISO 639 tal como viene en el archivo ("jpn", "spa").
    pub language: Option<String>,
}

impl Probe {
    /// Lo técnico, en palabras para mostrar.
    pub fn tech_info(&self) -> TechInfo {
        let video = self.video();
        TechInfo {
            resolution: video.and_then(|v| v.height).map(resolution_label),
            video_codec: video.and_then(|v| v.codec_name.as_deref()).map(codec_label),
            video_profile: video.and_then(|v| v.profile.clone()),
            bit_depth: video.map(ProbeStream::bit_depth),
            dynamic_range: video.map(|v| {
                match (v.dolby_vision_profile(), v.color_transfer.as_deref()) {
                    (Some(profile), _) => format!("Dolby Vision {profile}"),
                    (None, Some("smpte2084")) => "HDR10".to_string(),
                    (None, Some("arib-std-b67")) => "HLG".to_string(),
                    _ => "SDR".to_string(),
                }
            }),
            frame_rate: video
                .and_then(|v| v.avg_frame_rate.as_deref())
                .and_then(parse_rate),
            bitrate: self.format.bit_rate.as_deref().and_then(|b| b.parse().ok()),
            audio: self
                .audios()
                .map(|a| AudioInfo {
                    codec: codec_label(a.codec_name.as_deref().unwrap_or("?")),
                    channels: channels_label(a.channels.unwrap_or(0)),
                    language: a.language().map(str::to_string),
                })
                .collect(),
        }
    }
}

/// "2160p", "1080p"… por el alto (con margen: 1920×800 es "1080p" recortado, no "800p").
pub fn resolution_label(height: i64) -> String {
    match height {
        h if h >= 1800 => "2160p".to_string(),
        h if h >= 800 => "1080p".to_string(),
        h if h >= 600 => "720p".to_string(),
        h if h >= 400 => "480p".to_string(),
        h => format!("{h}p"),
    }
}

fn codec_label(codec: &str) -> String {
    match codec {
        "hevc" => "HEVC",
        "h264" => "H.264",
        "av1" => "AV1",
        "vp9" => "VP9",
        "mpeg4" => "MPEG-4",
        "mpeg2video" => "MPEG-2",
        "aac" => "AAC",
        "ac3" => "AC3",
        "eac3" => "E-AC3",
        "dts" => "DTS",
        "truehd" => "TrueHD",
        "opus" => "Opus",
        "flac" => "FLAC",
        "mp3" => "MP3",
        "vorbis" => "Vorbis",
        other => return other.to_uppercase(),
    }
    .to_string()
}

fn channels_label(channels: i64) -> String {
    match channels {
        1 => "1.0".to_string(),
        2 => "2.0".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        n => format!("{n} canales"),
    }
}

/// "24000/1001" → 23,976. "0/0" (desconocido) → nada.
fn parse_rate(rate: &str) -> Option<f64> {
    let (num, den) = rate.split_once('/')?;
    let (num, den): (f64, f64) = (num.parse().ok()?, den.parse().ok()?);
    (den > 0.0 && num > 0.0).then(|| num / den)
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
    /// El video no es HEVC: se convierte con x265. Minutos u horas.
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

    /// PQ (HDR10, HDR10+) o HLG.
    pub fn is_hdr(&self) -> bool {
        matches!(
            self.color_transfer.as_deref(),
            Some("smpte2084" | "arib-std-b67")
        )
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
    fn h264_is_converted_but_hdr_is_not() {
        let plan = Probe::parse(ANIME_MKV).unwrap().plan(Path::new("a.mkv"));
        assert_eq!(plan.operation, Operation::Video);
        assert_eq!(
            plan.changes,
            ["contenedor matroska → MP4", "video h264 → HEVC 10 bits"]
        );
        assert!(plan.blockers.is_empty());

        // HDR en otro códec no se convierte: Kinora nunca recrea HDR.
        let hdr = ANIME_MKV.replace(
            r#""pix_fmt": "yuv420p""#,
            r#""pix_fmt": "yuv420p10le", "color_transfer": "smpte2084""#,
        );
        let plan = Probe::parse(&hdr).unwrap().plan(Path::new("a.mkv"));
        assert_eq!(
            plan.blockers,
            ["es HDR en h264: Kinora solo conserva HDR que ya viene en HEVC"]
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
    fn describes_the_technical_details() {
        let json = KINORA_MP4
            .replace(
                r#""duration": "1559.98""#,
                r#""duration": "1559.98", "bit_rate": "2150000""#,
            )
            .replace(
                r#""height": 1080"#,
                r#""height": 1080, "avg_frame_rate": "24000/1001""#,
            );
        let tech = Probe::parse(&json).unwrap().tech_info();
        assert_eq!(tech.resolution.as_deref(), Some("1080p"));
        assert_eq!(tech.video_codec.as_deref(), Some("HEVC"));
        assert_eq!(tech.video_profile.as_deref(), Some("Main 10"));
        assert_eq!(tech.bit_depth, Some(10));
        assert_eq!(tech.dynamic_range.as_deref(), Some("SDR"));
        assert!((tech.frame_rate.unwrap() - 23.976).abs() < 0.001);
        assert_eq!(tech.bitrate, Some(2_150_000));
        assert_eq!(
            tech.audio,
            [AudioInfo {
                codec: "AAC".into(),
                channels: "2.0".into(),
                language: Some("jpn".into())
            }]
        );
        assert_eq!(
            resolution_label(800),
            "1080p",
            "1920×800 es 1080p con bandas"
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
            probe.plan(Path::new("a.mp4")).blockers,
            ["tiene Dolby Vision perfil 5"]
        );
    }
}
