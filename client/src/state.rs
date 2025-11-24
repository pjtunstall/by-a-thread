use std::collections::HashMap;

use shared::{maze::Maze, player::Player};

#[derive(Debug)]
pub enum ClientState {
    Startup {
        prompt_printed: bool,
    },
    Connecting,
    Authenticating {
        waiting_for_input: bool,
        guesses_left: u8,
    },
    ChoosingUsername {
        prompt_printed: bool,
    },
    AwaitingUsernameConfirmation,
    InChat,
    Countdown,
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
    pub fn already_disconnecting_or_disconnected(&self) -> bool {
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
    Enabled,
    DisabledWaiting,
}
