use std::{io::stdout, time::Instant};

use crossterm::{
    cursor::{MoveToColumn, Show},
    execute,
    style::Print,
    terminal::{Clear, ClearType},
};

use crate::{
    net::ServerNetworkHandle,
    state::{Countdown, InGame, ServerState},
};

pub fn handle(
    _network: &mut dyn ServerNetworkHandle,
    state: &mut Countdown,
) -> Option<ServerState> {
    let server_time = Instant::now();

    if server_time < state.end_time {
        let remaining_secs = (state.end_time - server_time).as_secs();
        let output = format!("Game starting in {:}s...", remaining_secs);

        execute!(
            stdout(),
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Print(output)
        )
        .expect("failed to print countdown line");

        None
    } else {
        execute!(
            stdout(),
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Show
        )
        .expect("failed to show cursor and clear line");

        println!();

        Some(ServerState::InGame(InGame::new(
            state.players.clone(),
            state.maze.clone(),
        )))
    }
}
