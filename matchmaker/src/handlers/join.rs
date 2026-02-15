use axum::extract::{Path, State};
use axum::{Json, http::StatusCode};

use crate::{errors::HttpError, extractors::VersionCode, state::AppState};
use common::auth::Passcode;

#[derive(serde::Serialize)]
pub struct JoinGameOkBody {
    port: u16,
    connect_token: String,
}

impl JoinGameOkBody {
    fn to_resonse(self) -> JoinGameOk {
        (StatusCode::OK, Json(self))
    }
}

type JoinGameOk = (StatusCode, Json<JoinGameOkBody>);

type JoinGameResult = Result<(StatusCode, Json<JoinGameOkBody>), HttpError>;

pub async fn join_game(
    State(state): State<AppState>,
    _version_code: VersionCode,
    Path(passcode): Path<String>,
) -> JoinGameResult {
    let passcode = Passcode::from_string(&passcode).ok_or(HttpError::InvalidPassCode)?;

    let mut games = state.games.lock().await;
    let game = games
        .get_mut(&passcode.bytes)
        .ok_or(HttpError::GameNotFound)?;

    let connect_token = game.get_token().ok_or(HttpError::LimitsExceeded)?;
    let response_body = JoinGameOkBody {
        port: game.port,
        connect_token,
    };

    Ok(response_body.to_resonse())
}
