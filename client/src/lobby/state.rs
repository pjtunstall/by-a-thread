use std::collections::HashMap;

use shared::{auth::Passcode, maze::Maze, player::Player};

#[derive(Debug)]
pub enum LobbyState {
    Startup {
        prompt_printed: bool,
    },
    Connecting {
        pending_passcode: Option<Passcode>,
    },
    Authenticating {
        waiting_for_input: bool,
        guesses_left: u8,
        waiting_for_server: bool,
    },
    ChoosingUsername {
        prompt_printed: bool,
    },
    AwaitingUsernameConfirmation,
    InChat {
        awaiting_initial_roster: bool,
        waiting_for_server: bool,
    },
    Countdown {
        end_time: f64,
        maze: Maze,
        players: HashMap<u64, Player>,
    },
    ChoosingDifficulty {
        prompt_printed: bool,
        choice_sent: bool,
    },
}
