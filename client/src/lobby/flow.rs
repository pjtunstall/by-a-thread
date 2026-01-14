use crate::{
    assets::Assets,
    lobby::handlers,
    net::RenetNetworkHandle,
    session::ClientSession,
    state::{ClientState, Lobby},
};

pub enum LobbyStep {
    Continue,
    StartGame,
    Transition(ClientState),
}

pub fn update(
    session: &mut ClientSession,
    ui: &mut dyn crate::lobby::ui::LobbyUi,
    network_handle: &mut RenetNetworkHandle<'_>,
    assets: Option<&Assets>,
    is_host: bool,
) -> LobbyStep {
    if session.state.is_disconnected() {
        return LobbyStep::Continue;
    }

    if matches!(session.input_mode(), crate::state::InputMode::Enabled) {
        let ui_ref: &mut dyn crate::lobby::ui::LobbyUi = ui;
        match ui_ref.poll_input(common::chat::MAX_CHAT_MESSAGE_BYTES, is_host) {
            Ok(Some(input)) => session.add_input(input),
            Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                ui.show_sanitized_error(&format!("No connection: {}.", e));
                return LobbyStep::Transition(ClientState::Disconnected {
                    message: e.to_string(),
                });
            }
            Ok(None) => {}
        }
    }

    if let Some(next_state) = transition(session, ui, network_handle, assets) {
        return LobbyStep::Transition(next_state);
    }

    if session.is_countdown_finished() {
        return LobbyStep::StartGame;
    }

    let ui_state = session.prepare_ui_state();
    if ui_state.show_waiting_message {
        ui.show_warning("Waiting for server...");
    }

    if !session.is_countdown_active() {
        let should_show_input = matches!(ui_state.mode, crate::state::InputMode::Enabled);
        let show_cursor = should_show_input;
        let font = assets.map(|assets| &assets.font);
        ui.draw(should_show_input, show_cursor, font);
    }

    LobbyStep::Continue
}

fn transition(
    session: &mut ClientSession,
    ui: &mut dyn crate::lobby::ui::LobbyUi,
    network_handle: &mut RenetNetworkHandle<'_>,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    match session.state {
        ClientState::Lobby(Lobby::ServerAddress { .. }) => {
            handlers::server_address::handle(session, ui)
        }
        ClientState::Lobby(Lobby::Passcode { .. }) => handlers::passcode::handle(session, ui),
        ClientState::Lobby(Lobby::Connecting { .. }) => {
            handlers::connecting::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::Authenticating { .. }) => {
            handlers::auth::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::ChoosingUsername { .. }) => {
            handlers::username::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::AwaitingUsernameConfirmation) => {
            handlers::waiting::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::Chat { .. }) => {
            handlers::chat::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::ChoosingDifficulty { .. }) => {
            handlers::difficulty::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::Countdown { .. }) => {
            handlers::countdown::handle(session, ui, network_handle, assets)
        }
        ClientState::Disconnected { .. } => None,
        ClientState::Game(_) => None,
        ClientState::AfterGameChat { .. } => None,
    }
}
