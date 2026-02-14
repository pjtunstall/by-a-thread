use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use tokio::sync::Mutex;

use super::ErrorBody;
use crate::ports::PortPool;

pub async fn create_game(State(port_pool): State<PortPoolState>) -> CreateGameResult {
    let port = port_pool.lock().await.get().ok_or_else(limits_exceeded)?;
    let body = new_game_data(port);

    Ok((StatusCode::OK, Json(body)))
}

type PortPoolState = Arc<Mutex<PortPool>>;

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

fn new_game_data(port: u16) -> CreateGameSuccessBody {
    CreateGameSuccessBody {
        port,
        connect_token: r#"{"client_id":1,"protocol_id":0}"#.to_string(),
        passcode: "123456".to_string(),
    }
}
