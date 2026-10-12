//! Deducir título, año, temporada y episodio a partir del nombre de un archivo.
//! Es solo una sugerencia: el usuario la corrige en el formulario.

use std::sync::LazyLock;

use regex::Regex;

use super::Guess;

// `LazyLock` compila cada expresión regular una sola vez, la primera vez que se usa.
static BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\((19\d{2}|20\d{2})\)").unwrap());
static SEASON_EPISODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bS(\d{1,2})E(\d{1,3})\b").unwrap());
static DASH_EPISODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s-\s(\d{1,3})\b").unwrap());
static RELEASE_INFO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(2160p|1080p|720p|480p|x26[45]|h\.?26[45]|hevc|web-?dl|bluray)\b.*").unwrap()
});

pub fn guess(file_name: &str) -> Guess {
    // Sin extensión y sin grupos entre corchetes ([Erai-raws], [1080p], ...).
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let mut text = BRACKETS.replace_all(stem, " ").into_owned();

    // Nombres con puntos o guiones bajos en lugar de espacios: The.Office.S02E03
    if !text.contains(' ') {
        text = text.replace(['.', '_'], " ");
    }

    let mut guess = Guess::default();

    if let Some(captures) = YEAR.captures(&text) {
        guess.year = captures[1].parse().ok();
    }

    // El título es lo que está antes de la temporada/episodio o del año.
    let mut title_end = text.len();
    if let Some(captures) = SEASON_EPISODE.captures(&text) {
        guess.season = captures[1].parse().ok();
        guess.episode = captures[2].parse().ok();
        title_end = captures.get(0).unwrap().start();
    } else if let Some(captures) = DASH_EPISODE.captures(&text) {
        guess.episode = captures[1].parse().ok();
        title_end = captures.get(0).unwrap().start();
    }
    if let Some(year) = YEAR.find(&text) {
        title_end = title_end.min(year.start());
    }

    let title = RELEASE_INFO.replace(&text[..title_end], "");
    let title = title.trim().trim_end_matches(['-', ' ']).trim();
    if !title.is_empty() {
        guess.title = Some(title.split_whitespace().collect::<Vec<_>>().join(" "));
    }

    guess
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(title: &str, year: Option<i64>, season: Option<i64>, episode: Option<i64>) -> Guess {
        Guess {
            title: Some(title.to_string()),
            year,
            season,
            episode,
        }
    }

    #[test]
    fn anime_release_with_dash_episode() {
        assert_eq!(
            guess("[Erai-raws] Sousou no Frieren - 01 [1080p][Multiple Subtitle][CA2CAE77].mkv"),
            g("Sousou no Frieren", None, None, Some(1))
        );
    }

    #[test]
    fn dotted_name_with_season_and_episode() {
        assert_eq!(
            guess("The.Office.US.S02E03.720p.WEB-DL.mkv"),
            g("The Office US", None, Some(2), Some(3))
        );
    }

    #[test]
    fn movie_with_year() {
        assert_eq!(
            guess("Interstellar (2014).mp4"),
            g("Interstellar", Some(2014), None, None)
        );
        assert_eq!(
            guess("Interstellar (2014) 2160p HEVC.mkv"),
            g("Interstellar", Some(2014), None, None)
        );
    }

    #[test]
    fn plain_name() {
        assert_eq!(guess("Mi video.mp4"), g("Mi video", None, None, None));
    }
}
