use std::sync::Arc;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{Router, routing::post};
use tokio::sync::Mutex;

use matchmaker::handlers::{create_game, join_game};
use matchmaker::ports::PortPool;

const BINDING_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080);

#[tokio::main]
async fn main() {
    let port_pool = Arc::new(Mutex::new(PortPool::new()));
    let app = Router::new()
        .route("/games", post(create_game))
        .route("/games/{passcode}/join", post(join_game))
        .with_state(port_pool);

    let listener = tokio::net::TcpListener::bind(BINDING_ADDRESS)
        .await
        .expect("failed to bind HTTP listener to port 8080");

    axum::serve(listener, app)
        .await
        .expect("HTTP server failed");
}
