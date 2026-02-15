use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Debug)]
pub enum HttpError {
    InvalidPlayerCount,
    LimitsExceeded,
    InvalidVersionCode,
    VersionMismatch,
    InvalidPassCode,
    GameNotFound,
    LobbyFull,
    GameAlreadyStarted,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            HttpError::InvalidPlayerCount => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "INVALID_PLAYER_COUNT",
                    message: "Player count must be between 1 and 10.",
                },
            ),
            HttpError::LimitsExceeded => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorBody {
                    code: "LIMITS_EXCEEDED",
                    message: "No capacity for new games at the moment.",
                },
            ),
            HttpError::InvalidVersionCode => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "INVALID_VERSION_CODE",
                    message: "Version code header is missing or invalid.",
                },
            ),
            HttpError::VersionMismatch => (
                StatusCode::UNAUTHORIZED,
                ErrorBody {
                    code: "VERSION_MISMATCH",
                    message: "Client version is not supported.",
                },
            ),
            HttpError::InvalidPassCode => (
                StatusCode::BAD_REQUEST,
                ErrorBody {
                    code: "INVALID_PASSCODE_FORMAT",
                    message: "Passcode must be a six-digit number.",
                },
            ),
            HttpError::GameNotFound => (
                StatusCode::NOT_FOUND,
                ErrorBody {
                    code: "GAME_NOT_FOUND",
                    message: "No game with that passcode.",
                },
            ),
            HttpError::LobbyFull => (
                StatusCode::CONFLICT,
                ErrorBody {
                    code: "LOBBY_FULL",
                    message: "No slots left. All connect tokens have been claimed.",
                },
            ),
            HttpError::GameAlreadyStarted => (
                StatusCode::CONFLICT,
                ErrorBody {
                    code: "GAME_ALREADY_STARTED",
                    message: "The game has already started.",
                },
            ),
        };
        (status, Json(body)).into_response()
    }
}
