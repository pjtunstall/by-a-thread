pub use crate::{after_game_chat::AfterGameChat, game::state::Game, lobby::state::Lobby};

#[derive(Debug)]
pub enum ClientState {
    Lobby(Lobby),
    Game(Game),
    AfterGameChat(AfterGameChat),
    Disconnected {
        message: String,
        show_error: bool,
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
