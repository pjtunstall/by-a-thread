pub use crate::{game::state::Game, lobby::state::Lobby};

#[derive(Debug)]
pub enum ClientState {
    Lobby(Lobby),
    Game(Game),
    AfterGameChat {
        awaiting_initial_roster: bool,
        waiting_for_server: bool,
    },
    Disconnected {
        message: String,
    },
}

impl ClientState {
    pub fn not_already_disconnecting_or_disconnected(&self) -> bool {
        !matches!(self, ClientState::Disconnected { .. })
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
