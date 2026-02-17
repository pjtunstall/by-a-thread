use std::net::{SocketAddr, ToSocketAddrs};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::config;
use common::constants::VERSION_CODE_HEADER;

fn client(insecure: bool) -> Client {
    let mut builder = Client::builder().timeout(std::time::Duration::from_secs(30));
    if insecure {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder.build().expect("failed to build HTTP client")
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

fn server_addr_from_host_port(host: &str, port: u16) -> SocketAddr {
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .expect("failed to resolve game server host")
        .collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .copied()
        .or_else(|| addrs.into_iter().next())
        .expect("no addresses for game server host")
}

pub fn create_game(
    player_count: u8,
    matchmaker_host: Option<&str>,
) -> Result<(CreateGameResponse, SocketAddr), ApiError> {
    let api_host = match matchmaker_host {
        Some(h) => h.to_string(),
        None => config::api_server_host(),
    };
    let insecure = api_host == config::LOCAL_MATCHMAKER_HOST;
    let url = format!("https://{}/games", api_host);
    let response = client(insecure)
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
    let game_host = match matchmaker_host {
        Some(h) if h == config::LOCAL_MATCHMAKER_HOST => config::game_server_host(),
        Some(h) => h.to_string(),
        None => config::game_server_host(),
    };
    let addr = server_addr_from_host_port(&game_host, create_response.port);
    Ok((create_response, addr))
}

pub fn join_game(
    passcode: &str,
    matchmaker_host: Option<&str>,
) -> Result<(JoinGameResponse, SocketAddr), ApiError> {
    let api_host = match matchmaker_host {
        Some(h) => h.to_string(),
        None => config::api_server_host(),
    };
    let insecure = api_host == config::LOCAL_MATCHMAKER_HOST;
    let url = format!("https://{}/games/{}/join", api_host, passcode);
    let response = client(insecure)
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
    let game_host = match matchmaker_host {
        Some(h) if h == config::LOCAL_MATCHMAKER_HOST => config::game_server_host(),
        Some(h) => h.to_string(),
        None => config::game_server_host(),
    };
    let addr = server_addr_from_host_port(&game_host, join_response.port);
    Ok((join_response, addr))
}
