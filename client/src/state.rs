use std::collections::HashMap;

use shared::{auth::Passcode, maze::Maze, player::Player};

#[derive(Debug)]
pub enum ClientState {
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
    Disconnected {
        message: String,
    },
    TransitioningToDisconnected {
        message: String,
    },
    ChoosingDifficulty {
        prompt_printed: bool,
        choice_sent: bool,
    },
    InGame {
        maze: Maze,
        players: HashMap<u64, Player>,
    },
}

impl ClientState {
    pub fn not_already_disconnecting_or_disconnected(&self) -> bool {
        !matches!(
            self,
            ClientState::Disconnected { .. } | ClientState::TransitioningToDisconnected { .. }
        )
    }

    pub fn is_disconnected(&self) -> bool {
        matches!(self, ClientState::Disconnected { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Hidden,
    SingleKey,
    Enabled,
    DisabledWaiting,
}
