use std::{net::SocketAddr, time::SystemTime, time::UNIX_EPOCH};

use base64::Engine;

use axum::{Json, extract::State, http::StatusCode};
use rand::TryRngCore;
use renet_netcode::ConnectToken;
use serde::Serialize;

use super::ErrorBody;
use crate::ports::AppState;
use common::{auth::Passcode, constants::PRE_GAME_TIMER_SECS};

pub async fn create_game(State(state): State<AppState>) -> CreateGameResult {
    let port = state
        .port_pool
        .lock()
        .await
        .get()
        .ok_or_else(limits_exceeded)?;
    let body = new_game_data(state.server_host, port);

    Ok((StatusCode::OK, Json(body)))
}

#[derive(Serialize)]
pub struct CreateGameSuccessBody {
    port: u16,
    connect_token: String,
    passcode: String,
}

type CreateGameResult =
    Result<(StatusCode, Json<CreateGameSuccessBody>), (StatusCode, Json<ErrorBody>)>;

fn limits_exceeded() -> (StatusCode, Json<ErrorBody>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorBody {
            kind: "LIMITS_EXCEEDED",
            message: "No capacity for new games at the moment.",
        }),
    )
}

fn new_game_data(server_host: std::net::IpAddr, port: u16) -> CreateGameSuccessBody {
    let private_key = private_key();

    let connect_token = create_connect_token(server_host, port, &private_key);
    let mut bytes = Vec::new();
    connect_token
        .write(&mut bytes)
        .expect("failed to write token");
    let connect_token_str = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let passcode = Passcode::generate(6).string;

    CreateGameSuccessBody {
        port,
        connect_token: connect_token_str,
        passcode,
    }
}

fn private_key() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("`getrandom` failed; failed to generate private key");
    bytes
}

fn create_connect_token(
    server_host: std::net::IpAddr,
    port: u16,
    private_key: &[u8; 32],
) -> ConnectToken {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before unix epoch");
    let protocol_id = common::protocol::version();
    let server_addr = SocketAddr::new(server_host, port);

    ConnectToken::generate(
        current_time,
        protocol_id,
        PRE_GAME_TIMER_SECS,
        1,
        15, // Timeout after 15 seconds.
        vec![server_addr],
        None,
        private_key,
    )
    .expect("failed to generate token")
}
