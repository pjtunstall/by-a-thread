pub use crate::{game::state::Game, lobby::state::Lobby, post_game_chat::PostGameChat};

#[derive(Debug)]
pub enum ClientState {
    Lobby(Lobby),
    Game(Game),
    PostGameChat(PostGameChat),
    Disconnected { message: String },
    EndAfterLeaderboard,
    Transitioning,
}

impl ClientState {
    pub fn not_already_disconnecting_or_disconnected(&self) -> bool {
        !matches!(
            self,
            ClientState::Disconnected { .. } | ClientState::EndAfterLeaderboard
        )
    }

    pub fn is_disconnected(&self) -> bool {
        matches!(
            self,
            ClientState::Disconnected { .. } | ClientState::EndAfterLeaderboard
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Hidden,
    SingleKey,
    Enabled,
    DisabledWaiting,
}
