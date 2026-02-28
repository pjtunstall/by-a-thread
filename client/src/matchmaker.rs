use std::net::{SocketAddr, ToSocketAddrs};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::config;
use common::constants::{CLIENT_PROOF_HEADER, VERSION_HEADER};

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
pub struct MatchmakerErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub enum MatchmakerError {
    Http(reqwest::Error),
    Json(serde_json::Error),
    InvalidClientProof { message: String },
    VersionMismatch { message: String },
    Unauthorized { message: String },
    Response { code: String, message: String },
}

impl std::fmt::Display for MatchmakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchmakerError::Http(e) => write!(f, "{}", e),
            MatchmakerError::Json(e) => write!(f, "{}", e),
            MatchmakerError::InvalidClientProof { message } => write!(f, "{}", message),
            MatchmakerError::VersionMismatch { message } => write!(f, "{}", message),
            MatchmakerError::Unauthorized { message } => write!(f, "{}", message),
            MatchmakerError::Response { message, .. } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for MatchmakerError {}

fn socket_address_from_host_and_port(host: &str, port: u16) -> SocketAddr {
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

fn auth_error_message(kind: &str, message: &str) -> String {
    if !message.is_empty() {
        return message.to_string();
    }
    match kind {
        "INVALID_CLIENT_PROOF" => "Client proof is missing or invalid.".to_string(),
        _ => "Client version is not supported. Please download the current version.".to_string(),
    }
}

fn matchmaker_error_from_response(
    status: u16,
    error_body: MatchmakerErrorBody,
) -> MatchmakerError {
    match error_body.code.as_str() {
        "INVALID_CLIENT_PROOF" => MatchmakerError::InvalidClientProof {
            message: auth_error_message("INVALID_CLIENT_PROOF", &error_body.message),
        },
        "VERSION_MISMATCH" => MatchmakerError::VersionMismatch {
            message: auth_error_message("VERSION_MISMATCH", &error_body.message),
        },
        _ if status == 401 => MatchmakerError::Unauthorized {
            message: auth_error_message("_", &error_body.message),
        },
        _ => MatchmakerError::Response {
            code: error_body.code,
            message: if error_body.message.is_empty() {
                "An error occurred.".to_string()
            } else {
                error_body.message
            },
        },
    }
}

pub fn create_game(
    player_count: u8,
    matchmaker_host: Option<&str>,
) -> Result<(CreateGameResponse, SocketAddr), MatchmakerError> {
    let host = match matchmaker_host {
        Some(host) => host.to_string(),
        None => config::matchmaker_host(),
    };
    let insecure = host == config::LOCAL_MATCHMAKER_HOST;
    let url = format!("https://{}/games", host);
    let response = client(insecure)
        .post(&url)
        .header(CLIENT_PROOF_HEADER, config::client_proof())
        .header(VERSION_HEADER, env!("CARGO_PKG_VERSION"))
        .json(&CreateGameRequest { player_count })
        .send()
        .map_err(MatchmakerError::Http)?;

    let status = response.status();
    let body = response.text().map_err(MatchmakerError::Http)?;

    if !status.is_success() {
        let error_body: MatchmakerErrorBody =
            serde_json::from_str(&body).unwrap_or(MatchmakerErrorBody {
                code: "UNKNOWN".to_string(),
                message: body,
            });
        return Err(matchmaker_error_from_response(status.as_u16(), error_body));
    }

    let create_response: CreateGameResponse =
        serde_json::from_str(&body).map_err(MatchmakerError::Json)?;
    let game_host = match matchmaker_host {
        Some(host) if host == config::LOCAL_MATCHMAKER_HOST => config::game_server_host(),
        Some(host) => host.to_string(),
        None => config::game_server_host(),
    };
    let server_address = socket_address_from_host_and_port(&game_host, create_response.port);
    Ok((create_response, server_address))
}

pub fn join_game(
    passcode: &str,
    matchmaker_host: Option<&str>,
) -> Result<(JoinGameResponse, SocketAddr), MatchmakerError> {
    let host = match matchmaker_host {
        Some(host) => host.to_string(),
        None => config::matchmaker_host(),
    };
    let insecure = host == config::LOCAL_MATCHMAKER_HOST;
    let url = format!("https://{}/games/{}/join", host, passcode);
    let response = client(insecure)
        .post(&url)
        .header(CLIENT_PROOF_HEADER, config::client_proof())
        .header(VERSION_HEADER, env!("CARGO_PKG_VERSION"))
        .json(&serde_json::json!({}))
        .send()
        .map_err(MatchmakerError::Http)?;

    let status = response.status();
    let body = response.text().map_err(MatchmakerError::Http)?;

    if !status.is_success() {
        let error_body: MatchmakerErrorBody =
            serde_json::from_str(&body).unwrap_or(MatchmakerErrorBody {
                code: "UNKNOWN".to_string(),
                message: body,
            });
        return Err(matchmaker_error_from_response(status.as_u16(), error_body));
    }

    let join_response: JoinGameResponse =
        serde_json::from_str(&body).map_err(MatchmakerError::Json)?;
    let game_host = match matchmaker_host {
        Some(host) if host == config::LOCAL_MATCHMAKER_HOST => config::game_server_host(),
        Some(host) => host.to_string(),
        None => config::game_server_host(),
    };
    let server_address = socket_address_from_host_and_port(&game_host, join_response.port);
    Ok((join_response, server_address))
}
