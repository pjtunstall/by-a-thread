use axum::{Json, extract::State, http::StatusCode};

use crate::{
    auth,
    errors::HttpError,
    extractors::{ClientProof, RateLimitCreate},
    game::Game,
    state::AppState,
};
use common::auth::Passcode;

#[derive(serde::Deserialize)]
pub struct CreateGameRequest {
    pub player_count: u8,
}

#[derive(serde::Serialize)]
pub struct CreateGameOkBody {
    port: u16,
    connect_token: String,
    passcode: String,
}

impl CreateGameOkBody {
    fn to_response(self) -> CreateGameOk {
        (StatusCode::OK, Json(self))
    }
}

type CreateGameOk = (StatusCode, Json<CreateGameOkBody>);

type CreateGameResult = Result<(StatusCode, Json<CreateGameOkBody>), HttpError>;

pub async fn create_game(
    State(state): State<AppState>,
    _rate_limit: RateLimitCreate,
    _client_proof: ClientProof,
    Json(request_body): Json<CreateGameRequest>,
) -> CreateGameResult {
    let player_count = request_body.player_count;
    check_player_count(player_count)?;

    let port = state
        .port_pool
        .get()
        .await
        .ok_or(HttpError::LimitsExceeded)?;
    let passcode = Passcode::generate();
    let passcode_string = passcode.string.clone();
    let passcode_bytes = passcode.bytes.clone();
    let private_key = auth::private_key();

    let mut new_game = Game::new(state.server_host, port, player_count, private_key, passcode);
    let connect_token = new_game.get_token().ok_or(HttpError::LimitsExceeded)?;

    if let Err(e) = new_game
        .start_server_container(private_key, state.server_host)
        .await
    {
        state.port_pool.release(port).await;
        return Err(e);
    }

    let container_name = new_game.container_name.as_deref().unwrap_or("?");
    println!(
        "New game created on port: {} (container: {})",
        port, container_name
    );
    state.games.insert(passcode_bytes, new_game).await;

    let response_body = CreateGameOkBody {
        port,
        connect_token,
        passcode: passcode_string,
    };

    Ok(response_body.to_response())
}

fn check_player_count(player_count: u8) -> Result<(), HttpError> {
    if player_count < 1 || player_count > 10 {
        return Err(HttpError::InvalidPlayerCount);
    }
    Ok(())
}
