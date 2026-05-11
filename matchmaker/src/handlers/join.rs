use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    errors::HttpError,
    extractors::{ClientProof, RateLimitJoin},
    state::AppState,
};
use common::auth::Passcode;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinGameRequest {}

#[derive(serde::Serialize)]
pub struct JoinGameOkBody {
    port: u16,
    connect_token: String,
}

impl JoinGameOkBody {
    fn to_response(self) -> JoinGameOk {
        (StatusCode::OK, Json(self))
    }
}

type JoinGameOk = (StatusCode, Json<JoinGameOkBody>);

type JoinGameResult = Result<(StatusCode, Json<JoinGameOkBody>), HttpError>;

pub async fn join_game(
    State(state): State<AppState>,
    _rate_limit: RateLimitJoin,
    _client_proof: ClientProof,
    Path(passcode): Path<String>,
    Json(_body): Json<JoinGameRequest>,
) -> JoinGameResult {
    let passcode = Passcode::from_string(&passcode).ok_or(HttpError::InvalidPassCode)?;

    let (port, connect_token) = state.games.try_join(passcode.bytes).await?;

    let response_body = JoinGameOkBody {
        port,
        connect_token,
    };

    Ok(response_body.to_response())
}
