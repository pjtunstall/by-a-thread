use std::net::{SocketAddr, ToSocketAddrs};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::config;
use common::constants::VERSION_CODE_HEADER;

fn client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

#[derive(Serialize)]
struct CreateGameRequest {
    player_count: u8,
}

#[derive(Deserialize)]
pub struct CreateGameResponse {
    pub port: u16,
    pub connect_token: String,
    pub passcode: String,
}

#[derive(Deserialize)]
pub struct JoinGameResponse {
    pub port: u16,
    pub connect_token: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub enum ApiError {
    Http(reqwest::Error),
    Json(serde_json::Error),
    Api { code: String, message: String },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Http(e) => write!(f, "{}", e),
            ApiError::Json(e) => write!(f, "{}", e),
            ApiError::Api { message, .. } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for ApiError {}

fn server_addr_from_port(port: u16) -> SocketAddr {
    let mut addrs = (config::game_server_host().as_str(), port)
        .to_socket_addrs()
        .expect("failed to resolve game server host");
    addrs.next().expect("no addresses for game server host")
}

pub fn create_game(player_count: u8) -> Result<(CreateGameResponse, SocketAddr), ApiError> {
    let url = format!("https://{}/games", config::api_server_host());
    let response = client()
        .post(&url)
        .header(VERSION_CODE_HEADER, config::version_code())
        .json(&CreateGameRequest { player_count })
        .send()
        .map_err(ApiError::Http)?;

    let status = response.status();
    let body = response.text().map_err(ApiError::Http)?;

    if !status.is_success() {
        let error_body: ApiErrorBody = serde_json::from_str(&body).unwrap_or(ApiErrorBody {
            code: "UNKNOWN".to_string(),
            message: body,
        });
        return Err(ApiError::Api {
            code: error_body.code,
            message: error_body.message,
        });
    }

    let create_response: CreateGameResponse =
        serde_json::from_str(&body).map_err(ApiError::Json)?;
    let addr = server_addr_from_port(create_response.port);
    Ok((create_response, addr))
}

pub fn join_game(passcode: &str) -> Result<(JoinGameResponse, SocketAddr), ApiError> {
    let url = format!("https://{}/games/{}/join", config::api_server_host(), passcode);
    let response = client()
        .post(&url)
        .header(VERSION_CODE_HEADER, config::version_code())
        .json(&serde_json::json!({}))
        .send()
        .map_err(ApiError::Http)?;

    let status = response.status();
    let body = response.text().map_err(ApiError::Http)?;

    if !status.is_success() {
        let error_body: ApiErrorBody = serde_json::from_str(&body).unwrap_or(ApiErrorBody {
            code: "UNKNOWN".to_string(),
            message: body,
        });
        return Err(ApiError::Api {
            code: error_body.code,
            message: error_body.message,
        });
    }

    let join_response: JoinGameResponse = serde_json::from_str(&body).map_err(ApiError::Json)?;
    let addr = server_addr_from_port(join_response.port);
    Ok((join_response, addr))
}
