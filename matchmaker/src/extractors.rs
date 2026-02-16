use std::net::{IpAddr, SocketAddr};

use axum::{extract::{ConnectInfo, FromRef, FromRequestParts}, http::request::Parts};
use base64::Engine;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{errors::HttpError, state::AppState};

const VERSION_CODE_HEADER: &str = "X-Version-Code";

fn client_ip(parts: &Parts) -> Option<IpAddr> {
    if let Some(forwarded) = parts.headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
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

pub struct VersionCode;

impl<S> FromRequestParts<S> for VersionCode
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let version_code = parts
            .headers
            .get(VERSION_CODE_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or(HttpError::InvalidVersionCode)?;

        let app_state = AppState::from_ref(state);
        validate_version_code(&version_code, app_state.version_hash)?;

        Ok(VersionCode)
    }
}

fn validate_version_code(
    version_code: &str,
    expected_hash: [u8; 32],
) -> Result<(), HttpError> {
    let version_bytes = base64::engine::general_purpose::STANDARD
        .decode(version_code.trim())
        .map_err(|_| HttpError::InvalidVersionCode)?;

    let computed_hash: [u8; 32] = Sha256::digest(&version_bytes).into();
    if !bool::from(computed_hash.ct_eq(&expected_hash)) {
        return Err(HttpError::VersionMismatch);
    }

    Ok(())
}
