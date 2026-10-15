use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Errores que un handler puede devolver. Cada uno se convierte en una respuesta HTTP.
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    /// Falta iniciar sesión, o el usuario o la contraseña no coinciden (401).
    Unauthorized(String),
    /// Hay sesión (o no hace falta), pero esto no se puede hacer desde acá (403).
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    /// Demasiados intentos seguidos (429).
    TooManyRequests(String),
    /// Falló un servicio de afuera del que depende el pedido, como TMDB (502).
    Upstream(String),
    Internal(anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            AppError::Unauthorized(message) => (StatusCode::UNAUTHORIZED, message),
            AppError::Forbidden(message) => (StatusCode::FORBIDDEN, message),
            AppError::TooManyRequests(message) => (StatusCode::TOO_MANY_REQUESTS, message),
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            AppError::Conflict(message) => (StatusCode::CONFLICT, message),
            AppError::Upstream(message) => (StatusCode::BAD_GATEWAY, message),
            AppError::Internal(err) => {
                // El detalle va al registro del servidor, no al cliente.
                tracing::error!("internal error: {err:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "error interno del servidor".to_string(),
                )
            }
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}

// Permite usar `?` con cualquier error común (sqlx, io, anyhow...): se convierte en Internal.
impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        AppError::Internal(err.into())
    }
}
