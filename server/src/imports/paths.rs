//! Nombres de carpetas y archivos dentro de una biblioteca, compatibles con la convención de Plex:
//!
//! ```text
//! Interstellar (2014)/Interstellar (2014).mp4
//! Frieren (2023)/Season 01/Frieren (2023) - S01E01.mp4
//! ```

use crate::naming::safe_name;

/// "Título (Año)", o solo "Título" si no hay año. Sin caracteres prohibidos en nombres de archivo.
pub fn content_dir(title: &str, year: Option<i64>) -> String {
    let name = match year {
        Some(year) => format!("{title} ({year})"),
        None => title.to_string(),
    };
    safe_name(&name)
}

pub fn movie_file(dir: &str) -> String {
    format!("{dir}/{dir}.mp4")
}

pub fn episode_file(show_dir: &str, season: i64, episode: i64) -> String {
    format!("{show_dir}/Season {season:02}/{show_dir} - S{season:02}E{episode:02}.mp4")
}

/// Otra versión del mismo archivo: `… - S01E05.mp4` → `… - S01E05 - v2.mp4`.
pub fn version_file(rel_path: &str, version: u32) -> String {
    let base = rel_path.strip_suffix(".mp4").unwrap_or(rel_path);
    format!("{base} - v{version}.mp4")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_plex_style_paths() {
        let dir = content_dir("Frieren", Some(2023));
        assert_eq!(dir, "Frieren (2023)");
        assert_eq!(
            episode_file(&dir, 1, 5),
            "Frieren (2023)/Season 01/Frieren (2023) - S01E05.mp4"
        );
        assert_eq!(
            movie_file(&content_dir("Interstellar", Some(2014))),
            "Interstellar (2014)/Interstellar (2014).mp4"
        );
    }

    #[test]
    fn versions_get_a_suffix() {
        assert_eq!(
            version_file("Frieren (2023)/Season 01/Frieren (2023) - S01E05.mp4", 2),
            "Frieren (2023)/Season 01/Frieren (2023) - S01E05 - v2.mp4"
        );
    }

    #[test]
    fn content_dir_uses_safe_names() {
        assert_eq!(
            content_dir("Mission: Impossible", Some(1996)),
            "Mission Impossible (1996)"
        );
    }
}
