use axum::extract::{Path, State};

use crate::ports::PortPoolState;

pub async fn join_game(
    State(_port_pool): State<PortPoolState>,
    Path(passcode): Path<String>,
) -> &'static str {
    println!("Joining game with passcode: {}", passcode);
    ""
}
