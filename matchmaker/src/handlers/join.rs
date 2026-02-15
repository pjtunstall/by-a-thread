use axum::extract::{Path, State};

use crate::{extractors::VersionCode, state::AppState};

pub async fn join_game(
    State(_state): State<AppState>,
    _version_code: VersionCode,
    Path(passcode): Path<String>,
) -> &'static str {
    println!("Joining game with passcode: {}", passcode);
    ""
}
