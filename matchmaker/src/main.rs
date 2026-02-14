use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{Router, routing::post};
use tokio::sync::Mutex;

use matchmaker::{
    handlers::{create_game, join_game},
    ports::{AppState, PortPool, resolve_server_host},
};

const BINDING_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080);

#[tokio::main]
async fn main() {
    let server_host = match std::env::var("DOMAIN") {
        Ok(domain) => resolve_server_host(&format!("api.{}", domain)),
        Err(_) => resolve_server_host("127.0.0.1"),
    };

    let state = AppState {
        port_pool: Arc::new(Mutex::new(PortPool::new())),
        server_host,
    };
    let app = Router::new()
        .route("/games", post(create_game))
        .route("/games/{passcode}/join", post(join_game))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(BINDING_ADDRESS)
        .await
        .expect("failed to bind HTTP listener to port 8080");

    axum::serve(listener, app)
        .await
        .expect("HTTP server failed");
}
