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
        awaiting_confirmation: bool,
    },
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
    pub fn allows_user_disconnect(&self) -> bool {
        !matches!(
            self,
            ClientState::Disconnected { .. } | ClientState::TransitioningToDisconnected { .. }
        )
    }

    pub fn is_disconnected(&self) -> bool {
        matches!(self, ClientState::Disconnected { .. })
    }

    pub fn should_show_input_box(&self) -> bool {
        !matches!(
            self,
            ClientState::ChoosingDifficulty {
                choice_sent: true,
                ..
            } | ClientState::Countdown
                | ClientState::Disconnected { .. }
                | ClientState::TransitioningToDisconnected { .. }
                | ClientState::InGame { .. }
        )
    }
}
