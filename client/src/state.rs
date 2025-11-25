pub use crate::in_game::Game;
pub use crate::lobby::LobbyState;

#[derive(Debug)]
pub enum ClientState {
    Lobby(LobbyState),
    InGame(Game),
    Debrief,
    Disconnected { message: String },
    TransitioningToDisconnected { message: String },
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
