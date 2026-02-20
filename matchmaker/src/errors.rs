use axum::{http::{header::RETRY_AFTER, HeaderValue, StatusCode}, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug)]
pub enum HttpError {
    RateLimited { retry_after: u64 },
    InvalidPlayerCount,
    LimitsExceeded,
    InvalidVersionCode,
    VersionMismatch { message: String },
    InvalidPassCode,
    GameNotFound,
    LobbyFull,
    GameAlreadyStarted,
    ServerError,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let (status, body, extra_headers) = match self {
            HttpError::RateLimited { retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                ErrorBody {
                    code: "RATE_LIMITED",
                    message: "Too many requests. Try again later.".to_string(),
                },
                Some((
                    RETRY_AFTER,
                    HeaderValue::from_str(&retry_after.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("60")),
                )),
            ),
            HttpError::InvalidPlayerCount => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "INVALID_PLAYER_COUNT",
                    message: "player_count must be between 1 and 10.".to_string(),
                },
                None,
            ),
            HttpError::LimitsExceeded => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorBody {
                    code: "LIMITS_EXCEEDED",
                    message: "No capacity for new games at the moment.".to_string(),
                },
                None,
            ),
            HttpError::InvalidVersionCode => (
                StatusCode::UNAUTHORIZED,
                ErrorBody {
                    code: "VERSION_MISMATCH",
                    message: "Client version is not supported.".to_string(),
                },
                None,
            ),
            HttpError::VersionMismatch { message } => (
                StatusCode::UNAUTHORIZED,
                ErrorBody {
                    code: "VERSION_MISMATCH",
                    message,
                },
                None,
            ),
            HttpError::InvalidPassCode => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "INVALID_PASSCODE_FORMAT",
                    message: "passcode must be a six-digit number.".to_string(),
                },
                None,
            ),
            HttpError::GameNotFound => (
                StatusCode::NOT_FOUND,
                ErrorBody {
                    code: "GAME_NOT_FOUND",
                    message: "No game with that passcode.".to_string(),
                },
                None,
            ),
            HttpError::LobbyFull => (
                StatusCode::CONFLICT,
                ErrorBody {
                    code: "LOBBY_FULL",
                    message: "No slots left. All connect tokens have been claimed.".to_string(),
                },
                None,
            ),
            HttpError::GameAlreadyStarted => (
                StatusCode::CONFLICT,
                ErrorBody {
                    code: "GAME_ALREADY_STARTED",
                    message: "The game has already started.".to_string(),
                },
                None,
            ),
            HttpError::ServerError => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorBody {
                    code: "SERVER_ERROR",
                    message: "Failed to start game server. Please try again.".to_string(),
                },
                None,
            ),
        };
        let mut response = (status, Json(body)).into_response();
        if let Some((name, value)) = extra_headers {
            response.headers_mut().insert(name, value);
        }
        response
    }
}
