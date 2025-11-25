pub use crate::in_game::Game;
pub use crate::lobby::LobbyState;
pub use crate::lobby::LobbyState as Lobby;

#[derive(Debug)]
pub enum ClientState {
    Lobby(LobbyState),
    InGame(Game),
    Debrief,
    Disconnected { message: String },
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
