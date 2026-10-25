//! Autenticación: una cuenta (la del dueño), con usuario y contraseña, y sesiones por equipo.
//!
//! - Toda la API pide sesión, salvo `/api/health` y `/api/auth/*`. Las páginas (el HTML y los
//!   scripts) son públicas: sin sesión muestran la pantalla de entrada.
//! - La sesión es un token al azar que el navegador guarda en una cookie `HttpOnly` (JavaScript
//!   no la ve). Para la app de la TV también sirve `Authorization: Bearer <token>`.
//! - La cuenta se crea una sola vez, y solo desde la casa: la misma compu o la red local, nunca
//!   por el link público (Funnel). Si no, cualquiera podría adueñarse de Kinora antes que vos.
//! - Los intentos de contraseña tienen freno (`limit`).

mod handlers;
mod limit;
pub mod repo;
mod secret;

use std::net::{IpAddr, SocketAddr};

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use limit::LoginLimiter;

use crate::app::AppState;
use crate::error::AppError;

/// Nombre de la cookie de sesión.
const COOKIE: &str = "kinora_session";

/// La sesión del pedido: la deja el middleware para los handlers que la necesitan.
#[derive(Clone, Copy, Debug)]
pub struct Session {
    pub id: i64,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct AuthState {
    /// Todavía no hay cuenta: hay que crearla.
    pub setup_needed: bool,
    /// Si la cuenta se puede crear desde donde se pide (la casa, no internet).
    pub can_setup_here: bool,
    pub logged_in: bool,
    pub username: Option<String>,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Para crear la cuenta: el usuario, la contraseña y el nombre del primer perfil.
#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Setup {
    pub username: String,
    pub password: String,
    /// "Fran": el perfil principal (lo visto hasta ahora queda en él).
    pub profile: String,
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PasswordChange {
    pub current: String,
    pub new: String,
}

/// Una sesión abierta, para la lista de equipos en el Admin.
#[derive(Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct SessionInfo {
    pub id: i64,
    /// "Chrome en macOS", "Safari en iPhone"…
    pub device: Option<String>,
    pub client: Option<String>,
    pub created_at: i64,
    pub last_seen_at: i64,
    /// Si es la de quien pide la lista.
    pub current: bool,
}

/// Rutas públicas (sin sesión). Se montan bajo /api.
pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/state", get(handlers::state))
        .route("/auth/setup", post(handlers::setup))
        .route("/auth/login", post(handlers::login))
        .route("/auth/logout", post(handlers::logout))
}

/// Rutas del Admin. Se montan bajo /api/admin.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/account/password", put(handlers::change_password))
        .route("/account/sessions", get(handlers::list_sessions))
        .route("/account/sessions/{id}", delete(handlers::close_session))
}

/// Middleware: deja pasar solo pedidos con una sesión válida (y la anota en el pedido).
pub async fn require_session(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = token_of(request.headers()) else {
        log_rejected(&request, "no session");
        return AppError::Unauthorized("iniciá sesión".to_string()).into_response();
    };
    match repo::touch_session(&state.db, &secret::token_hash(&token)).await {
        Ok(Some(id)) => {
            request.extensions_mut().insert(Session { id });
            next.run(request).await
        }
        Ok(None) => {
            log_rejected(&request, "expired session");
            AppError::Unauthorized("la sesión venció: iniciá sesión de nuevo".to_string())
                .into_response()
        }
        Err(err) => AppError::from(err).into_response(),
    }
}

/// Un pedido rechazado por no tener sesión, en el registro: quién y desde qué navegador. Así se
/// ve, por ejemplo, un reproductor de TV que pide el video sin la cookie.
fn log_rejected(request: &Request, reason: &str) {
    let client = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map_or_else(|| "?".to_string(), |info| info.0.ip().to_string());
    let agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    tracing::warn!(
        "401 {reason}: {} {} from {client} ({agent})",
        request.method(),
        request.uri().path()
    );
}

/// El token del pedido: la cookie de sesión o el encabezado `Authorization: Bearer`.
fn token_of(headers: &HeaderMap) -> Option<String> {
    if let Some(bearer) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(bearer.trim().to_string());
    }
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_string())
}

/// Si el pedido llega por un proxy de Tailscale (`serve` o el link público, Funnel). Esos
/// pedidos llegan desde 127.0.0.1, así que la IP sola no alcanza: se miran los encabezados
/// que agrega el proxy.
fn proxied(headers: &HeaderMap) -> bool {
    [
        "x-forwarded-for",
        "tailscale-funnel-request",
        "tailscale-user-login",
    ]
    .iter()
    .any(|name| headers.contains_key(*name))
}

/// Desde dónde viene el pedido, para el freno de intentos y la lista de sesiones: la IP real
/// si pasó por un proxy, o la de la conexión.
fn client_of(headers: &HeaderMap, peer: &ConnectInfo<SocketAddr>) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|ip| ip.trim().to_string())
        .unwrap_or_else(|| peer.0.ip().to_string())
}

/// Si el pedido viene de la casa: esta compu o la red local, directo (sin proxy). Las IPs de
/// Tailscale (100.64.0.0/10) no cuentan: pueden ser equipos compartidos con otras personas.
fn from_home(headers: &HeaderMap, peer: &ConnectInfo<SocketAddr>) -> bool {
    if proxied(headers) {
        return false;
    }
    match peer.0.ip() {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|v4| v4.is_loopback() || v4.is_private())
                || (ip.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

/// Si el pedido viene de la compu donde corre Kinora (no de otro equipo, ni por un proxy de
/// Tailscale, que también llega desde 127.0.0.1).
pub fn same_machine(headers: &HeaderMap, peer: &ConnectInfo<SocketAddr>) -> bool {
    let ip = peer.0.ip();
    !proxied(headers)
        && (ip.is_loopback()
            || matches!(ip, IpAddr::V6(v6) if v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())))
}

/// "Chrome en macOS", a partir del User-Agent: lo justo para reconocer una sesión en la lista.
fn device_of(headers: &HeaderMap) -> Option<String> {
    let agent = headers.get(header::USER_AGENT)?.to_str().ok()?;
    let browser = if agent.contains("Edg/") {
        "Edge"
    } else if agent.contains("Firefox/") {
        "Firefox"
    } else if agent.contains("Chrome/") {
        "Chrome"
    } else if agent.contains("Safari/") {
        "Safari"
    } else if agent.contains("curl/") {
        "curl"
    } else {
        "Navegador"
    };
    let system = if agent.contains("Tizen") {
        Some("TV Samsung")
    } else if agent.contains("iPhone") {
        Some("iPhone")
    } else if agent.contains("iPad") {
        Some("iPad")
    } else if agent.contains("Android") {
        Some("Android")
    } else if agent.contains("Mac OS X") {
        Some("macOS")
    } else if agent.contains("Windows") {
        Some("Windows")
    } else if agent.contains("Linux") {
        Some("Linux")
    } else {
        None
    };
    Some(match system {
        Some(system) => format!("{browser} en {system}"),
        None => browser.to_string(),
    })
}

/// La cookie de sesión. `Secure` (solo por HTTPS) si el pedido llegó por HTTPS (el Funnel);
/// en la red de casa Kinora habla HTTP y una cookie `Secure` no se guardaría.
fn session_cookie(token: &str, headers: &HeaderMap) -> String {
    let secure = headers
        .get("x-forwarded-proto")
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"https"));
    format!(
        "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        repo::SESSION_IDLE_SECS,
        if secure { "; Secure" } else { "" }
    )
}

fn clear_cookie() -> String {
    format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn peer(ip: &str) -> ConnectInfo<SocketAddr> {
        ConnectInfo(format!("{ip}:5000").parse().unwrap())
    }

    #[test]
    fn same_machine_is_only_this_computer_without_proxies() {
        let none = HeaderMap::new();
        assert!(same_machine(&none, &peer("127.0.0.1")));
        assert!(same_machine(
            &none,
            &ConnectInfo("[::1]:5000".parse().unwrap())
        ));
        assert!(!same_machine(&none, &peer("192.168.0.145")));
        let mut funnel = HeaderMap::new();
        funnel.insert("tailscale-funnel-request", HeaderValue::from_static("?1"));
        assert!(!same_machine(&funnel, &peer("127.0.0.1")));
    }

    #[test]
    fn home_is_this_computer_or_the_local_network_without_proxies() {
        let none = HeaderMap::new();
        assert!(from_home(&none, &peer("127.0.0.1")));
        assert!(from_home(&none, &peer("192.168.1.20")));
        assert!(from_home(&none, &peer("10.0.0.5")));
        assert!(
            !from_home(&none, &peer("100.101.102.103")),
            "Tailscale no es la casa"
        );
        assert!(!from_home(&none, &peer("181.10.20.30")));

        // El Funnel llega desde 127.0.0.1, pero con encabezados del proxy.
        let mut funnel = HeaderMap::new();
        funnel.insert("tailscale-funnel-request", HeaderValue::from_static("?1"));
        assert!(!from_home(&funnel, &peer("127.0.0.1")));
        let mut forwarded = HeaderMap::new();
        forwarded.insert("x-forwarded-for", HeaderValue::from_static("181.10.20.30"));
        assert!(!from_home(&forwarded, &peer("127.0.0.1")));
        assert_eq!(client_of(&forwarded, &peer("127.0.0.1")), "181.10.20.30");
    }

    #[test]
    fn reads_the_token_from_the_cookie_or_the_bearer_header() {
        let mut cookie = HeaderMap::new();
        cookie.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; kinora_session=abc; b=2"),
        );
        assert_eq!(token_of(&cookie).as_deref(), Some("abc"));
        let mut bearer = HeaderMap::new();
        bearer.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer xyz"),
        );
        assert_eq!(token_of(&bearer).as_deref(), Some("xyz"));
        assert_eq!(token_of(&HeaderMap::new()), None);
    }

    #[test]
    fn names_the_device() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36",
            ),
        );
        assert_eq!(device_of(&headers).as_deref(), Some("Chrome en macOS"));
    }
}
