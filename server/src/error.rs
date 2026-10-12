use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Errores que un handler puede devolver. Cada uno se convierte en una respuesta HTTP.
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
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
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, message),
            AppError::Conflict(message) => (StatusCode::CONFLICT, message),
            AppError::Internal(err) => {
                // El detalle va al registro del servidor, no al cliente.
                eprintln!("Error interno: {err:#}");
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
