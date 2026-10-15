//! Cliente de la API REST de OpenSubtitles (opensubtitles.com).
//!
//! Buscar alcanza con la clave de la API; descargar pide además iniciar sesión con una cuenta
//! (la cuenta gratis permite unas pocas descargas por día). La sesión dura un día: se guarda en
//! memoria y se renueva sola cuando vence.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::error::AppError;

const API_HOST: &str = "api.opensubtitles.com";

/// La sesión abierta: con qué usuario, su token y a qué servidor hablarle (los VIP tienen otro).
struct Session {
    username: String,
    token: String,
    base: String,
    until: Instant,
}

/// Una sola sesión para todo Kinora. Se pide de nuevo al cambiar la cuenta o al vencer.
static SESSION: Mutex<Option<Session>> = Mutex::new(None);
/// OpenSubtitles da tokens de 24 horas; se renuevan un poco antes.
const SESSION_LENGTH: Duration = Duration::from_secs(20 * 3600);

pub struct OpenSubtitles {
    client: Client,
    key: String,
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchItem {
    pub attributes: Attributes,
}

#[derive(Debug, Deserialize)]
pub struct Attributes {
    pub language: Option<String>,
    pub release: Option<String>,
    // OpenSubtitles a veces manda `null` en lugar de 0 o false: `nullable` lo toma como el valor
    // por defecto.
    #[serde(default, deserialize_with = "nullable")]
    pub download_count: i64,
    #[serde(default, deserialize_with = "nullable")]
    pub hearing_impaired: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub foreign_parts_only: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub ai_translated: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub machine_translated: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub from_trusted: bool,
    pub uploader: Option<Uploader>,
    pub feature_details: Option<FeatureDetails>,
    #[serde(default, deserialize_with = "nullable")]
    pub files: Vec<SubFile>,
}

/// Un campo que puede faltar o venir `null`: en los dos casos, el valor por defecto.
fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
pub struct Uploader {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FeatureDetails {
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SubFile {
    pub file_id: i64,
    pub file_name: Option<String>,
}

#[derive(Deserialize)]
struct SearchPage {
    #[serde(default, deserialize_with = "nullable")]
    data: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct LoginResponse {
    token: String,
    base_url: Option<String>,
}

#[derive(Deserialize)]
struct DownloadLink {
    link: Option<String>,
    remaining: Option<i64>,
}

/// Lo que dice OpenSubtitles cuando algo sale mal.
#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
    reset_time_utc: Option<String>,
}

#[derive(Deserialize)]
struct UserInfo {
    data: UserData,
}

#[derive(Deserialize)]
struct UserData {
    remaining_downloads: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct Language {
    pub language_code: String,
    pub language_name: String,
}

#[derive(Deserialize)]
struct Languages {
    data: Vec<Language>,
}

/// Un subtítulo bajado, en SRT.
pub struct Downloaded {
    pub bytes: Vec<u8>,
    /// Descargas que quedan hoy en la cuenta.
    pub remaining: Option<i64>,
}

impl OpenSubtitles {
    pub fn new(key: &str, username: &str, password: &str) -> OpenSubtitles {
        let client = Client::builder()
            // OpenSubtitles pide que cada aplicación se identifique con nombre y versión.
            .user_agent(concat!("Kinora v", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("the HTTP client could not be built");
        OpenSubtitles {
            client,
            key: key.trim().to_string(),
            username: username.trim().to_string(),
            password: password.to_string(),
        }
    }

    /// Comprueba la clave y la cuenta: inicia sesión de cero.
    pub async fn check(&self) -> Result<(), AppError> {
        *SESSION.lock().unwrap() = None;
        self.session().await.map(|_| ())
    }

    /// Busca subtítulos. `params` va tal cual a `GET /subtitles` (OpenSubtitles pide los
    /// parámetros en orden alfabético y en minúscula: si no, redirige).
    pub async fn search(
        &self,
        mut params: Vec<(&str, String)>,
    ) -> Result<Vec<SearchItem>, AppError> {
        params.sort_by(|a, b| a.0.cmp(b.0));
        let params: Vec<(&str, String)> = params
            .into_iter()
            .map(|(k, v)| (k, v.to_lowercase()))
            .collect();
        let base = self.session().await?.1;
        let response = self
            .send(self.client.get(format!("{base}/subtitles")).query(&params))
            .await?;
        let page: SearchPage = read(response).await?;
        Ok(page.data)
    }

    /// Baja un subtítulo en SRT. Gasta una de las descargas del día de la cuenta.
    pub async fn download(&self, file_id: i64) -> Result<Downloaded, AppError> {
        let link = match self.request_link(file_id).await {
            // El token pudo haber vencido antes de tiempo: una vez más con sesión nueva.
            Err(AppError::Unauthorized(_)) => {
                *SESSION.lock().unwrap() = None;
                self.request_link(file_id).await?
            }
            other => other?,
        };
        let url = link.link.ok_or_else(|| {
            AppError::Upstream("OpenSubtitles no dio el link de descarga".to_string())
        })?;
        let bytes = self
            .client
            .get(url)
            .send()
            .await
            .and_then(Response::error_for_status)
            .map_err(|err| AppError::Upstream(format!("no se pudo bajar el subtítulo: {err}")))?
            .bytes()
            .await
            .map_err(|err| AppError::Upstream(format!("no se pudo bajar el subtítulo: {err}")))?;
        Ok(Downloaded {
            bytes: bytes.to_vec(),
            remaining: link.remaining,
        })
    }

    async fn request_link(&self, file_id: i64) -> Result<DownloadLink, AppError> {
        let (token, base) = self.session().await?;
        let request = self
            .client
            .post(format!("{base}/download"))
            .bearer_auth(token)
            .json(&json!({ "file_id": file_id, "sub_format": "srt" }));
        let response = self.send(request).await?;
        if response.status() == StatusCode::NOT_ACCEPTABLE {
            let body: Option<ErrorBody> = response.json().await.ok();
            let when = body
                .and_then(|b| b.reset_time_utc)
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
                .map(|t| {
                    t.with_timezone(&chrono::Local)
                        .format(" (se renueva a las %H:%M)")
                        .to_string()
                })
                .unwrap_or_default();
            return Err(AppError::TooManyRequests(format!(
                "llegaste al límite de descargas de hoy de OpenSubtitles{when}"
            )));
        }
        read(response).await
    }

    /// Cuántas descargas le quedan hoy a la cuenta.
    pub async fn remaining(&self) -> Result<Option<i64>, AppError> {
        let (token, base) = self.session().await?;
        let response = self
            .send(
                self.client
                    .get(format!("{base}/infos/user"))
                    .bearer_auth(token),
            )
            .await?;
        let info: UserInfo = read(response).await?;
        Ok(info.data.remaining_downloads)
    }

    pub async fn languages(&self) -> Result<Vec<Language>, AppError> {
        let response = self
            .send(
                self.client
                    .get(format!("https://{API_HOST}/api/v1/infos/languages")),
            )
            .await?;
        let languages: Languages = read(response).await?;
        Ok(languages.data)
    }

    /// El token y la dirección a la que hablarle: los de la sesión abierta, o de una nueva.
    async fn session(&self) -> Result<(String, String), AppError> {
        if let Some(session) = SESSION.lock().unwrap().as_ref()
            && session.username == self.username
            && session.until > Instant::now()
        {
            return Ok((session.token.clone(), session.base.clone()));
        }
        let request = self
            .client
            .post(format!("https://{API_HOST}/api/v1/login"))
            .json(&json!({ "username": self.username, "password": self.password }));
        let response = self.send(request).await?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(AppError::BadRequest(
                "OpenSubtitles no acepta el usuario o la contraseña: revisalos en Configuración"
                    .to_string(),
            ));
        }
        let login: LoginResponse = read(response).await?;
        let base = format!(
            "https://{}/api/v1",
            login.base_url.as_deref().unwrap_or(API_HOST)
        );
        *SESSION.lock().unwrap() = Some(Session {
            username: self.username.clone(),
            token: login.token.clone(),
            base: base.clone(),
            until: Instant::now() + SESSION_LENGTH,
        });
        Ok((login.token, base))
    }

    /// Manda el pedido con la clave de la API. Los errores de red y de clave salen ya explicados.
    async fn send(&self, request: RequestBuilder) -> Result<Response, AppError> {
        let response = request
            .header("Api-Key", &self.key)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|err| {
                AppError::Upstream(format!(
                    "no se pudo conectar con OpenSubtitles: {}",
                    err.without_url()
                ))
            })?;
        if response.status() == StatusCode::FORBIDDEN {
            return Err(AppError::BadRequest(
                "OpenSubtitles no acepta la clave de la API: revisala en Configuración".to_string(),
            ));
        }
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Upstream(
                "OpenSubtitles recibió demasiados pedidos: probá de nuevo en un rato".to_string(),
            ));
        }
        Ok(response)
    }
}

/// Lee una respuesta exitosa; si no lo es, devuelve el mensaje de OpenSubtitles.
async fn read<T: DeserializeOwned>(response: Response) -> Result<T, AppError> {
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED {
        return Err(AppError::Unauthorized(
            "la sesión de OpenSubtitles venció".to_string(),
        ));
    }
    if !status.is_success() {
        let message = response
            .json::<ErrorBody>()
            .await
            .ok()
            .and_then(|b| b.message)
            .unwrap_or_else(|| status.to_string());
        return Err(AppError::Upstream(format!(
            "OpenSubtitles respondió: {message}"
        )));
    }
    let text = response.text().await.map_err(|err| {
        AppError::Upstream(format!(
            "no se pudo leer la respuesta de OpenSubtitles: {}",
            err.without_url()
        ))
    })?;
    serde_json::from_str(&text).map_err(|err| {
        // El detalle (qué campo no encajó) va a la terminal; en la pantalla, algo corto.
        let start: String = text.chars().take(300).collect();
        tracing::warn!("unexpected OpenSubtitles response: {err} · {start}");
        AppError::Upstream("respuesta inesperada de OpenSubtitles".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_results_are_read() {
        let page: SearchPage = serde_json::from_str(
            r#"{"total_pages": 1, "page": 1, "data": [{"id": "1", "type": "subtitle", "attributes": {
                "language": "es", "release": "Frieren.S01E01.1080p", "download_count": 1234,
                "hearing_impaired": false, "foreign_parts_only": false, "ai_translated": false,
                "machine_translated": false, "from_trusted": true, "fps": 23.976,
                "uploader": {"uploader_id": 1, "name": "alguien", "rank": "trusted"},
                "feature_details": {"feature_type": "Episode", "season_number": 1, "episode_number": 1},
                "files": [{"file_id": 99, "cd_number": 1, "file_name": "frieren.s01e01.srt"}]}}]}"#,
        )
        .unwrap();
        let item = &page.data[0].attributes;
        let with_nulls: SearchPage = serde_json::from_str(
            r#"{"data": [{"attributes": {"language": "es", "release": null, "download_count": null,
                "hearing_impaired": null, "foreign_parts_only": null, "ai_translated": null,
                "machine_translated": null, "from_trusted": null, "uploader": null,
                "feature_details": null, "files": [{"file_id": 1, "file_name": null}]}}]}"#,
        )
        .unwrap();
        assert!(!with_nulls.data[0].attributes.hearing_impaired);
        assert_eq!(item.language.as_deref(), Some("es"));
        assert_eq!(item.files[0].file_id, 99);
        assert_eq!(
            item.feature_details.as_ref().unwrap().episode_number,
            Some(1)
        );
    }
}
