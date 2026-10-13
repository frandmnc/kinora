use std::net::SocketAddr;

use axum::Json;
use axum::extract::{ConnectInfo, Extension, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tracing::{info, warn};

use super::{
    AuthState, Credentials, PasswordChange, Session, SessionInfo, Setup, clear_cookie, client_of,
    device_of, from_home, repo, secret, session_cookie, token_of,
};
use crate::app::AppState;
use crate::error::{AppError, AppResult};
use crate::profiles;

const MIN_PASSWORD: usize = 8;

/// GET /api/auth/state
/// Qué tiene que mostrar la app: crear la cuenta, entrar, o seguir.
pub async fn state(
    State(state): State<AppState>,
    peer: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> AppResult<Json<AuthState>> {
    let account = repo::account(&state.db).await?;
    let logged_in = match token_of(&headers) {
        Some(token) => repo::touch_session(&state.db, &secret::token_hash(&token))
            .await?
            .is_some(),
        None => false,
    };
    Ok(Json(AuthState {
        setup_needed: account.is_none(),
        can_setup_here: account.is_none() && from_home(&headers, &peer),
        username: logged_in
            .then(|| account.map(|(username, _)| username))
            .flatten(),
        logged_in,
    }))
}

/// POST /api/auth/setup
/// Crea la cuenta (una sola vez, y solo desde la casa) y deja la sesión abierta.
pub async fn setup(
    State(state): State<AppState>,
    peer: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<Setup>,
) -> AppResult<Response> {
    if !from_home(&headers, &peer) {
        return Err(AppError::Forbidden(
            "la cuenta se crea desde la compu donde corre Kinora o desde la red de casa"
                .to_string(),
        ));
    }
    if repo::account_exists(&state.db).await? {
        return Err(AppError::Conflict(
            "la cuenta ya existe: iniciá sesión".to_string(),
        ));
    }
    let username = request.username.trim();
    if username.is_empty() {
        return Err(AppError::BadRequest("falta el usuario".to_string()));
    }
    check_password(&request.password)?;
    let profile = profiles::handlers::clean_name(&request.profile)?;
    let hash = secret::hash_password(&request.password)?;
    repo::create_account(&state.db, username, &hash).await?;
    // El primer perfil existe desde siempre (con lo visto hasta ahora): toma el nombre elegido.
    profiles::repo::rename(&state.db, 1, &profile).await?;
    info!("account created for {username}");
    open_session(&state, &peer, &headers).await
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<AppState>,
    peer: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<Credentials>,
) -> AppResult<Response> {
    let client = client_of(&headers, &peer);
    if let Err(wait) = state.login_limiter.check(&client) {
        let minutes = wait.as_secs().div_ceil(60);
        return Err(AppError::TooManyRequests(format!(
            "demasiados intentos: probá de nuevo en {minutes} minuto{}",
            if minutes == 1 { "" } else { "s" }
        )));
    }
    let Some((username, hash)) = repo::account(&state.db).await? else {
        return Err(AppError::Conflict(
            "todavía no hay cuenta: creala primero".to_string(),
        ));
    };
    // Argon2 tarda a propósito (para que probar contraseñas sea lento): fuera del hilo async.
    let password = request.password.clone();
    let correct =
        tokio::task::spawn_blocking(move || secret::verify_password(&password, &hash)).await?;
    if !correct || !request.username.trim().eq_ignore_ascii_case(&username) {
        state.login_limiter.fail(&client);
        warn!("failed login from {client}");
        return Err(AppError::Unauthorized(
            "usuario o contraseña incorrectos".to_string(),
        ));
    }
    state.login_limiter.succeed(&client);
    info!("login from {client}");
    open_session(&state, &peer, &headers).await
}

/// POST /api/auth/logout
/// Cierra la sesión de este equipo.
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(token) = token_of(&headers)
        && let Some(id) = repo::touch_session(&state.db, &secret::token_hash(&token)).await?
    {
        repo::delete_session(&state.db, id).await?;
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, clear_cookie())],
    )
        .into_response())
}

/// PUT /api/admin/account/password
/// Cambia la contraseña y cierra las demás sesiones (por si la anterior se filtró).
pub async fn change_password(
    State(state): State<AppState>,
    Extension(session): Extension<Session>,
    Json(change): Json<PasswordChange>,
) -> AppResult<StatusCode> {
    let Some((_, hash)) = repo::account(&state.db).await? else {
        return Err(AppError::NotFound("no hay cuenta".to_string()));
    };
    let current = change.current.clone();
    if !tokio::task::spawn_blocking(move || secret::verify_password(&current, &hash)).await? {
        return Err(AppError::BadRequest(
            "la contraseña actual no es esa".to_string(),
        ));
    }
    check_password(&change.new)?;
    repo::set_password(&state.db, &secret::hash_password(&change.new)?).await?;
    let closed = repo::delete_other_sessions(&state.db, session.id).await?;
    info!("password changed; closed {closed} other session(s)");
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/admin/account/sessions
pub async fn list_sessions(
    State(state): State<AppState>,
    Extension(session): Extension<Session>,
) -> AppResult<Json<Vec<SessionInfo>>> {
    let mut sessions = repo::sessions(&state.db).await?;
    for s in &mut sessions {
        s.current = s.id == session.id;
    }
    Ok(Json(sessions))
}

/// DELETE /api/admin/account/sessions/{id}
/// Cierra la sesión de otro equipo (por ejemplo, uno que ya no está en la casa).
pub async fn close_session(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    if !repo::delete_session(&state.db, id).await? {
        return Err(AppError::NotFound(format!("no existe la sesión {id}")));
    }
    info!("closed session {id}");
    Ok(StatusCode::NO_CONTENT)
}

fn check_password(password: &str) -> AppResult<()> {
    if password.chars().count() < MIN_PASSWORD {
        return Err(AppError::BadRequest(format!(
            "la contraseña tiene que tener al menos {MIN_PASSWORD} caracteres"
        )));
    }
    Ok(())
}

/// Abre una sesión para este equipo y la manda en la cookie.
async fn open_session(
    state: &AppState,
    peer: &ConnectInfo<SocketAddr>,
    headers: &HeaderMap,
) -> AppResult<Response> {
    let token = secret::new_token();
    repo::create_session(
        &state.db,
        &secret::token_hash(&token),
        device_of(headers).as_deref(),
        &client_of(headers, peer),
    )
    .await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, session_cookie(&token, headers))],
    )
        .into_response())
}
