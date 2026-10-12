//! Nombres de carpetas y archivos que crea Kinora.

/// Reemplaza los caracteres que macOS, Linux o Windows no aceptan en nombres de archivo,
/// junta espacios repetidos y quita puntos iniciales (un nombre con "." inicial queda oculto).
pub fn safe_name(text: &str) -> String {
    let replaced: String = text
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_matches('.').trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_forbidden_characters() {
        assert_eq!(
            safe_name("Mission: Impossible / Fallout"),
            "Mission Impossible Fallout"
        );
        assert_eq!(safe_name("...oculta"), "oculta");
        assert_eq!(safe_name("  Anime  "), "Anime");
    }
}
