use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};
use tokio::sync::Mutex;

pub async fn join_game(Path(passcode): Path<String>) -> &'static str {
    println!("Joining game with passcode: {}", passcode);
    ""
}
