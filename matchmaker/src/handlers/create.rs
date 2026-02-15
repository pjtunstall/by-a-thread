use axum::{Json, extract::State, http::StatusCode};

use crate::{auth, errors::HttpError, extractors::VersionCode, game::Game, state::AppState};
use common::auth::Passcode;

#[derive(serde::Deserialize)]
pub struct CreateGameRequest {
    pub player_count: u8,
}

#[derive(serde::Serialize)]
pub struct CreateGameSuccessBody {
    port: u16,
    connect_token: String,
    passcode: String,
}

pub async fn create_game(
    State(state): State<AppState>,
    _version_code: VersionCode,
    Json(request_body): Json<CreateGameRequest>,
) -> CreateGameResult {
    let player_count = request_body.player_count;
    check_player_count(player_count)?;

    let port = state
        .port_pool
        .lock()
        .await
        .get()
        .ok_or(HttpError::LimitsExceeded)?;
    let passcode = Passcode::generate(6).string;
    let private_key = auth::private_key();

    let mut new_game = Game::new(state.server_host, port, player_count, private_key);
    let connect_token = new_game.get_token().ok_or(HttpError::LimitsExceeded)?;

    state.games.lock().await.insert(passcode.clone(), new_game);

    let response_body = CreateGameSuccessBody {
        port,
        connect_token,
        passcode: passcode,
    };

    Ok((StatusCode::OK, Json(response_body)))
}

type CreateGameResult = Result<(StatusCode, Json<CreateGameSuccessBody>), HttpError>;

fn check_player_count(player_count: u8) -> Result<(), HttpError> {
    if player_count < 1 || player_count > 10 {
        return Err(HttpError::InvalidPlayerCount);
    }
    Ok(())
}
