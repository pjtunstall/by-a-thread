use std::io::stdout;
use std::time::Instant;

use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    execute,
    terminal::{Clear, ClearType},
};

use crate::{
    net::ServerNetworkHandle,
    state::{Countdown, InGame, ServerState},
};

pub fn handle_countdown(
    _network: &mut dyn ServerNetworkHandle,
    state: &mut Countdown,
) -> Option<ServerState> {
    let server_time = Instant::now();

    if server_time < state.end_time {
        execute!(
            stdout(),
            MoveUp(1),
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
        )
        .expect("failed to clear line");
        println!(
            "Game starting in {:}s.",
            (state.end_time - server_time).as_secs()
        );
        None
    } else {
        Some(ServerState::InGame(InGame::new(
            state.players.clone(),
            state.maze.clone(),
        )))
    }
}
