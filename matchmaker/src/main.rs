use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{Router, routing::post};

use common::domain;
use matchmaker::{
    addressing::resolve_server_host,
    cleanup,
    games::Games,
    handlers::{create_game, join_game},
    ports::PortPool,
    rate_limiter,
    state::AppState,
};

const BINDING_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080);

#[tokio::main]
async fn main() {
    if let Err(e) = cleanup::cleanup_zombies().await {
        eprintln!("zombie container cleanup error: {}", e);
    }

    dotenvy::from_path(".env.matchmaker").ok();

    let server_host = resolve_server_host(&domain::game_server_host());

    let client_proof_hash = hex::decode(
        std::env::var("CLIENT_PROOF_HASH")
            .expect("`CLIENT_PROOF_HASH` must be set in .env.matchmaker"),
    )
    .ok()
    .and_then(|v| v.try_into().ok())
    .expect("`CLIENT_PROOF_HASH` must be a 64-character hex string (32 bytes)");

    let expected_version = env!("CARGO_PKG_VERSION").to_string();

    let state = AppState {
        port_pool: Arc::new(PortPool::new()),
        server_host,
        games: Arc::new(Games::new()),
        client_proof_hash,
        expected_version,
        rate_limiter: Arc::new(rate_limiter::RateLimiter::new()),
    };

    cleanup::spawn_cleanup_task(state.clone());

    let app = Router::new()
        .route("/games", post(create_game))
        .route("/games/{passcode}/join", post(join_game))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(BINDING_ADDRESS)
        .await
        .expect("failed to bind HTTP listener to port 8080");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("HTTP server failed");
}
