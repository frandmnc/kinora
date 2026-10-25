//! Actualizar Kinora desde el Admin, con las versiones publicadas en GitHub.
//!
//! El servidor busca la última versión y baja el archivo de su sistema (el instalador de
//! Windows o el zip de Kinora.app), verificado con el SHA-256 que publica GitHub. No se puede
//! reemplazar a sí mismo mientras corre: lo deja en la carpeta que le indica la app que lo
//! supervisa (la bandeja de Windows o Kinora.app), anota qué bajó en `pending.json` y se apaga
//! con el código de salida `UPDATE_EXIT_CODE`. La app instala y lo vuelve a abrir.
//!
//! Solo se ofrece si lo corre una de esas apps (`KINORA_SUPERVISOR` y `KINORA_UPDATE_DIR`):
//! uno arrancado a mano, desde una terminal, se actualiza a mano.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;
use tracing::info;
use ts_rs::TS;

use crate::app::AppState;
use crate::error::{AppError, AppResult};

/// El código con el que el servidor se apaga para que la app que lo supervisa lo actualice.
pub const UPDATE_EXIT_CODE: i32 = 75;

/// De dónde salen las versiones. `KINORA_UPDATE_REPO` lo cambia (para probar con otro repo).
const REPO: &str = "frandmnc/kinora";

/// Se dispara cuando la actualización ya está bajada: el servidor se apaga ordenado.
static READY: Notify = Notify::const_new();
static INSTALLING: AtomicBool = AtomicBool::new(false);

/// Espera a que haya una actualización lista para instalar.
pub async fn ready() {
    READY.notified().await;
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/updates", get(check))
        .route("/updates/install", post(install))
}

/// La versión actual, la última publicada y si se puede actualizar desde acá.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct UpdateStatus {
    pub current: String,
    pub latest: Option<ReleaseInfo>,
    pub update_available: bool,
    /// Por qué no se puede actualizar desde el Admin (`None` si se puede).
    pub cannot_install: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ReleaseInfo {
    pub version: String,
    pub prerelease: bool,
    /// Cuándo se publicó (AAAA-MM-DD).
    pub published: String,
    /// Las notas de la versión (Markdown, las del CHANGELOG).
    pub notes: String,
    pub url: String,
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct UpdateQuery {
    /// Contar también las versiones de prueba.
    #[serde(default)]
    pub prerelease: bool,
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct InstallUpdate {
    pub version: String,
}

/// GET /api/admin/updates?prerelease=true
async fn check(Query(query): Query<UpdateQuery>) -> AppResult<Json<UpdateStatus>> {
    let current = env!("CARGO_PKG_VERSION");
    let releases = fetch_releases().await?;
    let latest = latest(&releases, query.prerelease);
    let update_available = latest.is_some_and(|release| newer(&release.version(), current));
    Ok(Json(UpdateStatus {
        current: current.to_string(),
        cannot_install: cannot_install(),
        update_available,
        latest: latest.map(|release| ReleaseInfo {
            version: release.version(),
            prerelease: release.prerelease,
            published: release.published_at.chars().take(10).collect(),
            notes: release.body.clone().unwrap_or_default(),
            url: release.html_url.clone(),
        }),
    }))
}

/// POST /api/admin/updates/install
/// Baja y verifica la versión pedida, la deja para la app que supervisa el servidor y lo apaga.
/// Responde antes de apagarse.
async fn install(Json(request): Json<InstallUpdate>) -> AppResult<StatusCode> {
    if let Some(reason) = cannot_install() {
        return Err(AppError::Conflict(reason));
    }
    let (Some(dir), Some(suffix)) = (update_dir(), asset_suffix()) else {
        return Err(AppError::Conflict(
            "no se puede actualizar desde acá".to_string(),
        ));
    };
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err(AppError::Conflict("ya se está actualizando".to_string()));
    }
    let result = download(&request.version, &dir, suffix).await;
    if result.is_err() {
        INSTALLING.store(false, Ordering::SeqCst);
    }
    let file = result?;
    info!(
        "update {} downloaded to {}: restarting",
        request.version,
        file.display()
    );

    // Un segundo para que la respuesta llegue al Admin, y a apagarse.
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        READY.notify_one();
    });
    Ok(StatusCode::ACCEPTED)
}

async fn download(version: &str, dir: &std::path::Path, suffix: &str) -> AppResult<PathBuf> {
    let releases = fetch_releases().await?;
    let release = releases
        .iter()
        .find(|release| release.version() == version && !release.draft)
        .ok_or_else(|| AppError::NotFound(format!("no está publicada la versión {version}")))?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name.ends_with(suffix))
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "la versión {version} no trae el archivo para este equipo"
            ))
        })?;
    let expected = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .ok_or_else(|| AppError::Upstream("GitHub no publicó el SHA-256 del archivo".to_string()))?
        .to_lowercase();

    let bytes = client()?
        .get(&asset.browser_download_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| AppError::Upstream(format!("no se pudo bajar {}: {error}", asset.name)))?
        .bytes()
        .await
        .map_err(|error| {
            AppError::Upstream(format!("se cortó la descarga de {}: {error}", asset.name))
        })?;
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != expected {
        return Err(AppError::Upstream(format!(
            "{} no coincide con el que publicó GitHub (SHA-256 distinto): no se instala",
            asset.name
        )));
    }

    let write = || -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let file = dir.join(&asset.name);
        std::fs::write(&file, &bytes)?;
        let pending = serde_json::json!({ "version": version, "file": file });
        std::fs::write(dir.join("pending.json"), pending.to_string())?;
        Ok(file)
    };
    write()
        .map_err(|error| AppError::Internal(anyhow::anyhow!("could not save the update: {error}")))
}

/// Por qué no se puede actualizar desde el Admin, o `None` si se puede.
fn cannot_install() -> Option<String> {
    if asset_suffix().is_none() {
        return Some("las actualizaciones desde el Admin son para Windows y la Mac".to_string());
    }
    if update_dir().is_none() {
        return Some(
            "Kinora no está corriendo desde Kinora.app ni desde la bandeja de Windows: se actualiza a mano"
                .to_string(),
        );
    }
    None
}

/// Dónde deja la actualización: la carpeta que indica la app que supervisa el servidor.
fn update_dir() -> Option<PathBuf> {
    std::env::var_os("KINORA_SUPERVISOR")?;
    std::env::var_os("KINORA_UPDATE_DIR").map(PathBuf::from)
}

/// El final del nombre del archivo de cada sistema en los Releases.
fn asset_suffix() -> Option<&'static str> {
    if cfg!(windows) {
        Some("-setup.exe")
    } else if cfg!(target_os = "macos") {
        Some("-macos.zip")
    } else {
        None
    }
}

// MARK: GitHub

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: String,
    body: Option<String>,
    html_url: String,
    assets: Vec<Asset>,
}

impl Release {
    /// "v0.2.0" → "0.2.0".
    fn version(&self) -> String {
        self.tag_name.trim_start_matches('v').to_string()
    }
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    /// "sha256:…"
    digest: Option<String>,
}

fn client() -> AppResult<Client> {
    Client::builder()
        .user_agent(concat!("Kinora/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| AppError::Internal(error.into()))
}

async fn fetch_releases() -> AppResult<Vec<Release>> {
    let repo = std::env::var("KINORA_UPDATE_REPO").unwrap_or_else(|_| REPO.to_string());
    client()?
        .get(format!(
            "https://api.github.com/repos/{repo}/releases?per_page=30"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| AppError::Upstream(format!("no se pudo consultar GitHub: {error}")))?
        .json()
        .await
        .map_err(|error| AppError::Upstream(format!("GitHub respondió algo inesperado: {error}")))
}

/// La versión más alta publicada (sin borradores; las de prueba, solo si se piden).
fn latest(releases: &[Release], prerelease: bool) -> Option<&Release> {
    releases
        .iter()
        .filter(|release| !release.draft && (prerelease || !release.prerelease))
        .filter_map(|release| parse(&release.version()).map(|v| (v, release)))
        .max_by_key(|(version, _)| *version)
        .map(|(_, release)| release)
}

/// "0.10.2" → (0, 10, 2).
fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.').map(|part| part.parse().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// Si `candidate` es más nueva que `current`.
fn newer(candidate: &str, current: &str) -> bool {
    matches!((parse(candidate), parse(current)), (Some(a), Some(b)) if a > b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool, draft: bool) -> Release {
        Release {
            tag_name: tag.to_string(),
            draft,
            prerelease,
            published_at: "2026-10-24T12:00:00Z".to_string(),
            body: None,
            html_url: String::new(),
            assets: Vec::new(),
        }
    }

    #[test]
    fn compares_versions_by_number_not_by_text() {
        assert!(newer("0.10.0", "0.9.3"));
        assert!(newer("1.0.0", "0.99.99"));
        assert!(!newer("0.2.0", "0.2.0"));
        assert!(!newer("0.1.9", "0.2.0"));
        assert!(!newer("basura", "0.2.0"));
    }

    #[test]
    fn picks_the_highest_release_skipping_drafts_and_maybe_prereleases() {
        let releases = [
            release("v0.2.0", false, false),
            release("v0.4.0", false, true),
            release("v0.3.0", true, false),
            release("v0.10.0-rc", false, false),
        ];
        assert_eq!(latest(&releases, false).unwrap().version(), "0.2.0");
        assert_eq!(latest(&releases, true).unwrap().version(), "0.3.0");
        assert!(latest(&[], true).is_none());
    }
}
