use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorBody {
    pub kind: &'static str,
    pub message: &'static str,
}

#[derive(Debug)]
pub enum HttpError {
    InvalidPlayerCount,
    LimitsExceeded,
    InvalidVersionCode,
    VersionMismatch,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            HttpError::InvalidPlayerCount => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    kind: "INVALID_PLAYER_COUNT",
                    message: "Player count must be between 1 and 10.",
                },
            ),
            HttpError::LimitsExceeded => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorBody {
                    kind: "LIMITS_EXCEEDED",
                    message: "No capacity for new games at the moment.",
                },
            ),
            HttpError::InvalidVersionCode => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    kind: "INVALID_VERSION_CODE",
                    message: "Version code header is missing or invalid.",
                },
            ),
            HttpError::VersionMismatch => (
                StatusCode::UNAUTHORIZED,
                ErrorBody {
                    kind: "VERSION_MISMATCH",
                    message: "Client version is not supported.",
                },
            ),
        };
        (status, Json(body)).into_response()
    }
}
