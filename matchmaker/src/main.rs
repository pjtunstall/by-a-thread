use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, sync::Arc};

use axum::{Router, routing::post};

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
    dotenvy::from_path(".env.matchmaker").ok();

    let server_host = match std::env::var("DOMAIN") {
        Ok(domain) => resolve_server_host(&format!("api.{}", domain)),
        Err(_) => resolve_server_host("127.0.0.1"),
    };

    let version_hash = hex::decode(
        std::env::var("VERSION_HASH").expect("`VERSION_HASH` must be set in .env.matchmaker"),
    )
    .ok()
    .and_then(|v| v.try_into().ok())
    .expect("`VERSION_HASH` must be a 64-character hex string (32 bytes)");

    let state = AppState {
        port_pool: Arc::new(PortPool::new()),
        server_host,
        games: Arc::new(Games::new()),
        version_hash,
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
