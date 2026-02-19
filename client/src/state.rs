pub use crate::{
    game::state::Game,
    lobby::state::Lobby,
    post_game_chat::PostGameChat,
    pre_lobby::state::PreLobby,
};

#[derive(Debug)]
pub enum ClientState {
    PreLobby(PreLobby),
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

impl Default for ClientState {
    fn default() -> Self {
        Self::Transitioning
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Hidden,
    SingleKey,
    Enabled,
    DisabledWaiting,
}
