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
    let name = collapsed.trim_matches('.').trim().to_string();
    // Windows no acepta estos nombres (ni con extensión: "con.mp4"), aunque los demás sí: se
    // evitan en todos lados, por si la carpeta media se copia o se comparte con una PC.
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim()
        .to_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.ends_with(|c: char| c.is_ascii_digit() && c != '0'));
    if !reserved {
        return name;
    }
    // El guion bajo va antes de la extensión, para no cambiarla: "nul.mp4" → "nul_.mp4".
    match name.find('.') {
        Some(dot) => format!("{}_{}", &name[..dot], &name[dot..]),
        None => format!("{name}_"),
    }
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
        assert_eq!(safe_name("Con"), "Con_");
        assert_eq!(safe_name("nul.mp4"), "nul_.mp4");
        assert_eq!(safe_name("COM1"), "COM1_");
        assert_eq!(safe_name("Con Air"), "Con Air");
        assert_eq!(safe_name("Nul Points"), "Nul Points");
    }
}
