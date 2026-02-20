use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::{ConnectInfo, FromRef, FromRequestParts},
    http::request::Parts,
};
use base64::Engine;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{errors::HttpError, state::AppState};
use common::constants::{CLIENT_PROOF_HEADER, VERSION_HEADER};

fn client_ip(parts: &Parts) -> Option<IpAddr> {
    if let Some(forwarded) = parts
        .headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first) = forwarded.split(',').next() {
            if let Ok(ip) = first.trim().parse() {
                return Some(ip);
            }
        }
    }
    if let Some(real_ip) = parts.headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if let Ok(ip) = real_ip.trim().parse() {
            return Some(ip);
        }
    }
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|addr| addr.ip())
}

pub struct RateLimitCreate;

impl<S> FromRequestParts<S> for RateLimitCreate
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ip = client_ip(parts).unwrap_or(IpAddr::from([0, 0, 0, 0]));
        let app_state = AppState::from_ref(state);
        if let Err(retry_after) = app_state.rate_limiter.check_create(ip).await {
            return Err(HttpError::RateLimited { retry_after });
        }
        Ok(Self)
    }
}

pub struct RateLimitJoin;

impl<S> FromRequestParts<S> for RateLimitJoin
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ip = client_ip(parts).unwrap_or(IpAddr::from([0, 0, 0, 0]));
        let app_state = AppState::from_ref(state);
        if let Err(retry_after) = app_state.rate_limiter.check_join(ip).await {
            return Err(HttpError::RateLimited { retry_after });
        }
        Ok(Self)
    }
}

pub struct ClientProof;

impl<S> FromRequestParts<S> for ClientProof
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let client_proof = parts
            .headers
            .get(CLIENT_PROOF_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or(HttpError::InvalidClientProof)?;

        let client_version = parts
            .headers
            .get(VERSION_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .ok_or(HttpError::InvalidClientProof)?;

        let app_state = AppState::from_ref(state);
        validate_client_proof(&client_proof, app_state.client_proof_hash)?;
        validate_version(&client_version, &app_state.expected_version)?;

        Ok(ClientProof)
    }
}

fn validate_client_proof(client_proof: &str, expected_hash: [u8; 32]) -> Result<(), HttpError> {
    let client_proof_bytes = base64::engine::general_purpose::STANDARD
        .decode(client_proof.trim())
        .map_err(|_| HttpError::InvalidClientProof)?;

    let computed_hash: [u8; 32] = Sha256::digest(&client_proof_bytes).into();
    if !bool::from(computed_hash.ct_eq(&expected_hash)) {
        return Err(HttpError::InvalidClientProof);
    }

    Ok(())
}

fn validate_version(client_version: &str, expected_version: &str) -> Result<(), HttpError> {
    if client_version != expected_version {
        return Err(HttpError::VersionMismatch {
            message: format!(
                "Client version {} is not supported. Please download the current version: {}.",
                client_version, expected_version
            ),
        });
    }
    Ok(())
}
