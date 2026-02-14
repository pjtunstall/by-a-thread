use axum::extract::{Path, State};

use crate::ports::AppState;

pub async fn join_game(
    State(_state): State<AppState>,
    Path(passcode): Path<String>,
) -> &'static str {
    println!("Joining game with passcode: {}", passcode);
    ""
}
