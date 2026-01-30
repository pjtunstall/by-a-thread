use crate::{
    assets::Assets,
    lobby::state_handlers,
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
    let state = std::mem::take(&mut session.state);

    let result = match state {
        ClientState::Lobby(mut lobby_state) => {
            let result = match lobby_state {
                Lobby::ServerAddress { .. } => {
                    state_handlers::server_address::handle(&mut lobby_state, session, ui)
                }
                Lobby::Passcode { .. } => {
                    state_handlers::passcode::handle(&mut lobby_state, session, ui)
                }
                Lobby::Connecting { .. } => state_handlers::connecting::handle(
                    &mut lobby_state,
                    session,
                    ui,
                    network_handle,
                ),
                Lobby::Authenticating { .. } => {
                    state_handlers::auth::handle(&mut lobby_state, session, ui, network_handle)
                }
                Lobby::ChoosingUsername { .. } => {
                    state_handlers::username::handle(&mut lobby_state, session, ui, network_handle)
                }
                Lobby::AwaitingUsernameConfirmation => {
                    state_handlers::waiting::handle(&mut lobby_state, session, ui, network_handle)
                }
                Lobby::Chat { .. } => state_handlers::chat::handle(
                    &mut lobby_state,
                    session,
                    ui,
                    network_handle,
                    assets,
                ),
                Lobby::ChoosingDifficulty { .. } => state_handlers::difficulty::handle(
                    &mut lobby_state,
                    session,
                    ui,
                    network_handle,
                    assets,
                ),
                Lobby::Countdown { .. } => state_handlers::countdown::handle(
                    &mut lobby_state,
                    session,
                    ui,
                    network_handle,
                    assets,
                ),
            };
            session.state = ClientState::Lobby(lobby_state);
            result
        }
        other_state => {
            session.state = other_state;
            None
        }
    };

    result
}
