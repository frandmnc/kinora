//! Cliente de la API de TMDB (themoviedb.org): buscar películas y series y traer su metadata.
//!
//! Acepta las dos claves que da TMDB: la "API Key" (v3, va en la URL) y el "API Read Access
//! Token" (v4, va en el encabezado `Authorization`). Las dos sirven para la API v3.

use std::path::Path;
use std::time::Duration;

use reqwest::{Client, RequestBuilder, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::AppError;

const API: &str = "https://api.themoviedb.org/3";
const IMAGES: &str = "https://image.tmdb.org/t/p";

/// Español latino para sinopsis y episodios. Si algo no está traducido, se completa con el inglés.
/// Los títulos de películas y series van en su idioma original (ver `preferred_title`).
pub const LANGUAGE: &str = "es-MX";
const FALLBACK_LANGUAGE: &str = "en-US";

pub struct Tmdb {
    client: Client,
    key: String,
}

/// Un resultado de búsqueda: una película o una serie.
#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub id: i64,
    /// Películas: `title`; series: `name`.
    #[serde(alias = "name")]
    pub title: String,
    #[serde(alias = "original_name")]
    pub original_title: Option<String>,
    /// Películas: `release_date`; series: `first_air_date`. "2023-09-29", o vacío.
    #[serde(alias = "first_air_date")]
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

#[derive(Deserialize)]
struct SearchPage {
    results: Vec<SearchResult>,
}

impl SearchResult {
    /// El título a mostrar: el original si está en nuestro alfabeto; si no, el traducido.
    pub fn preferred_title(&self) -> &str {
        match &self.original_title {
            Some(original) if is_latin(original) => original,
            _ => &self.title,
        }
    }
}

/// El detalle de una película o una serie. Tiene la misma forma que un resultado de búsqueda,
/// más la imagen de fondo.
#[derive(Debug, Deserialize)]
pub struct Details {
    #[serde(alias = "name")]
    pub title: String,
    #[serde(alias = "original_name")]
    pub original_title: Option<String>,
    /// Código ISO 639-1: "en", "ja".
    pub original_language: Option<String>,
    #[serde(alias = "first_air_date")]
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Season {
    pub name: Option<String>,
    pub overview: Option<String>,
    #[serde(default)]
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Deserialize)]
pub struct Episode {
    pub episode_number: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
}

#[derive(Clone, Copy)]
pub enum Kind {
    Movie,
    Show,
}

impl Kind {
    fn path(self) -> &'static str {
        match self {
            Kind::Movie => "movie",
            Kind::Show => "tv",
        }
    }
}

impl Tmdb {
    pub fn new(key: &str) -> Tmdb {
        let client = Client::builder()
            .user_agent(concat!("Kinora/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("the HTTP client could not be built");
        Tmdb {
            client,
            key: key.trim().to_string(),
        }
    }

    /// Comprueba que TMDB acepte la clave.
    pub async fn check(&self) -> Result<(), AppError> {
        match self.get::<serde_json::Value>("/authentication", &[]).await {
            Ok(_) => Ok(()),
            Err(AppError::BadRequest(_)) => Err(AppError::BadRequest(
                "TMDB no acepta esa clave: copiala de nuevo desde themoviedb.org".to_string(),
            )),
            Err(err) => Err(err),
        }
    }

    pub async fn search(
        &self,
        kind: Kind,
        query: &str,
        year: Option<i64>,
    ) -> Result<Vec<SearchResult>, AppError> {
        let year = year.map(|y| y.to_string());
        let mut params = vec![
            ("query", query),
            ("language", LANGUAGE),
            ("include_adult", "false"),
        ];
        if let Some(year) = &year {
            params.push((
                match kind {
                    Kind::Movie => "year",
                    Kind::Show => "first_air_date_year",
                },
                year,
            ));
        }
        let page: SearchPage = self
            .get(&format!("/search/{}", kind.path()), &params)
            .await?;
        Ok(page.results)
    }

    /// El detalle, con la sinopsis en español (o en inglés si no está traducida) y el título
    /// original. Si el original está en otro alfabeto ("葬送のフリーレン"), el título en inglés: los
    /// títulos traducidos al español suelen cambiar demasiado ("Proyecto Fin del Mundo"). La
    /// carátula y el fondo van en el mismo idioma que el título (las carátulas tienen texto).
    pub async fn details(&self, kind: Kind, id: i64) -> Result<Details, AppError> {
        let path = format!("/{}/{id}", kind.path());
        let mut details: Details = self.get(&path, &[("language", LANGUAGE)]).await?;
        let original = details.original_title.clone().filter(|t| is_latin(t));
        let title_language = match (&original, details.original_language.as_deref()) {
            (Some(_), Some(language)) => language.to_string(),
            _ => FALLBACK_LANGUAGE.to_string(),
        };

        if !title_language.starts_with("es") {
            let other: Details = self.get(&path, &[("language", &title_language)]).await?;
            if original.is_none() {
                details.title = other.title;
            }
            details.poster_path = other.poster_path.or(details.poster_path);
            details.backdrop_path = other.backdrop_path.or(details.backdrop_path);
        }
        if let Some(original) = original {
            details.title = original;
        }
        if is_blank(&details.overview) {
            let english: Details = self.get(&path, &[("language", FALLBACK_LANGUAGE)]).await?;
            details.overview = english.overview;
        }
        Ok(details)
    }

    /// Una temporada con sus episodios. Lo que no está traducido (sinopsis vacías, o títulos
    /// genéricos como "Episodio 5") se completa con el inglés. `None` si TMDB no la tiene.
    pub async fn season(&self, show_id: i64, number: i64) -> Result<Option<Season>, AppError> {
        let path = format!("/tv/{show_id}/season/{number}");
        let Some(mut season) = self
            .get_optional::<Season>(&path, &[("language", LANGUAGE)])
            .await?
        else {
            return Ok(None);
        };
        let untranslated = season
            .episodes
            .iter()
            .any(|e| is_blank(&e.overview) || is_generic_title(&e.name, e.episode_number));
        if untranslated
            && let Some(english) = self
                .get_optional::<Season>(&path, &[("language", FALLBACK_LANGUAGE)])
                .await?
        {
            for episode in &mut season.episodes {
                let Some(other) = english
                    .episodes
                    .iter()
                    .find(|e| e.episode_number == episode.episode_number)
                else {
                    continue;
                };
                if is_blank(&episode.overview) {
                    episode.overview = other.overview.clone();
                }
                if is_generic_title(&episode.name, episode.episode_number) && !is_blank(&other.name)
                {
                    episode.name = other.name.clone();
                }
            }
        }
        Ok(Some(season))
    }

    /// Baja una imagen (`poster_path` o `backdrop_path`) al archivo `to`. Primero a un temporal
    /// y después se renombra: nunca queda una imagen a medias.
    pub async fn download_image(
        &self,
        image_path: &str,
        size: &str,
        to: &Path,
    ) -> anyhow::Result<()> {
        let url = format!("{IMAGES}/{size}{image_path}");
        let bytes = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let tmp = to.with_extension("jpg.tmp");
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, to).await?;
        Ok(())
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, AppError> {
        self.get_optional(path, params)
            .await?
            .ok_or_else(|| AppError::NotFound("TMDB no tiene ese contenido".to_string()))
    }

    /// Como `get`, pero un 404 de TMDB es `None`.
    async fn get_optional<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Option<T>, AppError> {
        let request = self.authorize(self.client.get(format!("{API}{path}")).query(params));
        let response = request.send().await.map_err(|err| {
            AppError::Upstream(format!(
                "no se pudo conectar con TMDB: {}",
                without_url(&err)
            ))
        })?;
        match response.status() {
            StatusCode::NOT_FOUND => return Ok(None),
            StatusCode::UNAUTHORIZED => {
                return Err(AppError::BadRequest(
                    "TMDB no acepta la clave: revisala en Configuración".to_string(),
                ));
            }
            StatusCode::TOO_MANY_REQUESTS => {
                return Err(AppError::Upstream(
                    "TMDB recibió demasiados pedidos: probá de nuevo en un rato".to_string(),
                ));
            }
            status if !status.is_success() => {
                return Err(AppError::Upstream(format!("TMDB respondió {status}")));
            }
            _ => {}
        }
        let value = response.json().await.map_err(|err| {
            AppError::Upstream(format!(
                "respuesta inesperada de TMDB: {}",
                without_url(&err)
            ))
        })?;
        Ok(Some(value))
    }

    /// El token v4 es un JWT ("eyJ…" con puntos) y va como `Bearer`; la clave v3, en la URL.
    fn authorize(&self, request: RequestBuilder) -> RequestBuilder {
        if self.key.starts_with("eyJ") && self.key.contains('.') {
            request.bearer_auth(&self.key)
        } else {
            request.query(&[("api_key", &self.key)])
        }
    }
}

/// La URL de una carátula chica de TMDB, para mostrar los resultados de una búsqueda.
pub fn thumbnail_url(poster_path: &str) -> String {
    format!("{IMAGES}/w185{poster_path}")
}

/// El año de una fecha "2023-09-29".
pub fn year_of(date: &Option<String>) -> Option<i64> {
    date.as_deref()?.get(..4)?.parse().ok()
}

pub fn is_blank(text: &Option<String>) -> bool {
    text.as_deref().is_none_or(|t| t.trim().is_empty())
}

/// Si un texto está escrito con letras latinas (con acentos y todo): "Amélie" sí, "葬送のフリーレン" no.
fn is_latin(text: &str) -> bool {
    text.chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| (c as u32) <= 0x024F || ('\u{1E00}'..='\u{1EFF}').contains(&c))
}

/// "Episodio 5", "Episode 5": lo que pone TMDB cuando no hay título traducido.
fn is_generic_title(name: &Option<String>, number: i64) -> bool {
    match name.as_deref().map(str::trim) {
        None | Some("") => true,
        Some(name) => name == format!("Episodio {number}") || name == format!("Episode {number}"),
    }
}

/// Los errores de reqwest incluyen la URL, que puede llevar la clave v3: se saca.
fn without_url(err: &reqwest::Error) -> String {
    let mut err = err.to_string();
    if let Some(start) = err.find(" for url (") {
        err.truncate(start);
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_comes_from_the_date() {
        assert_eq!(year_of(&Some("2023-09-29".to_string())), Some(2023));
        assert_eq!(year_of(&Some(String::new())), None);
        assert_eq!(year_of(&None), None);
    }

    #[test]
    fn latin_titles_are_kept_and_other_scripts_are_not() {
        assert!(is_latin("Project Hail Mary"));
        assert!(is_latin("Le Fabuleux Destin d'Amélie Poulain"));
        assert!(is_latin("Ðoàn"));
        assert!(!is_latin("葬送のフリーレン"));
        assert!(!is_latin("Брат"));
    }

    #[test]
    fn search_prefers_the_original_title_when_it_is_latin() {
        let result = |title: &str, original: &str| SearchResult {
            id: 1,
            title: title.to_string(),
            original_title: Some(original.to_string()),
            release_date: None,
            overview: None,
            poster_path: None,
        };
        assert_eq!(
            result("Proyecto Fin del Mundo", "Project Hail Mary").preferred_title(),
            "Project Hail Mary"
        );
        assert_eq!(
            result("Frieren", "葬送のフリーレン").preferred_title(),
            "Frieren"
        );
    }

    #[test]
    fn generic_episode_titles_are_detected() {
        assert!(is_generic_title(&Some("Episodio 5".to_string()), 5));
        assert!(is_generic_title(&Some("Episode 12".to_string()), 12));
        assert!(is_generic_title(&None, 1));
        assert!(!is_generic_title(&Some("El fin del viaje".to_string()), 1));
    }

    #[test]
    fn search_results_read_movies_and_shows() {
        let movie: SearchResult = serde_json::from_str(
            r#"{"id": 1, "title": "Dune", "original_title": "Dune", "release_date": "2021-09-15", "overview": "", "poster_path": "/a.jpg"}"#,
        )
        .unwrap();
        assert_eq!(movie.title, "Dune");
        let show: SearchResult = serde_json::from_str(
            r#"{"id": 2, "name": "Frieren", "original_name": "葬送のフリーレン", "first_air_date": "2023-09-29", "poster_path": null}"#,
        )
        .unwrap();
        assert_eq!(show.title, "Frieren");
        assert_eq!(year_of(&show.release_date), Some(2023));
    }
}
