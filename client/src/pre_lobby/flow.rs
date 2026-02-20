use crate::{
    assets::Assets,
    lobby::ui::LobbyUi,
    pre_lobby::state::{ApiRequestPhase, PreLobby},
    pre_lobby::state_handlers::{self, api_request_menu::CompleteInfo},
    session::ClientSession,
    state::{ClientState, InputMode},
};

pub enum PreLobbyStep {
    Continue,
    Complete(CompleteInfo),
    Exit,
    ExitPendingUserAck,
}

pub fn update(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    assets: Option<&Assets>,
) -> PreLobbyStep {
    let in_awaiting_matchmaker = matches!(
        &session.state,
        ClientState::PreLobby(PreLobby::ApiRequestMenu {
            phase: ApiRequestPhase::AwaitingCreate { .. } | ApiRequestPhase::AwaitingJoin { .. },
            ..
        })
    );
    if !in_awaiting_matchmaker && crate::exit::should_quit() {
        return PreLobbyStep::Exit;
    }

    if matches!(session.input_mode(), InputMode::Enabled) {
        match ui.poll_input(common::chat::MAX_CHAT_MESSAGE_BYTES, false) {
            Ok(Some(input)) => session.add_input(input),
            Err(e) => {
                ui.show_sanitized_error(&format!("No connection: {}.", e));
                return PreLobbyStep::Exit;
            }
            Ok(None) => {}
        }
    }

    if let Some(step) = transition(session, ui, assets) {
        return step;
    }

    let ui_state = session.prepare_ui_state();
    if ui_state.show_waiting_message {
        ui.show_warning("Waiting for server... (Press ESCAPE to cancel.)");
    }

    let should_show_input = matches!(ui_state.mode, InputMode::Enabled);
    let show_cursor = should_show_input;
    let font = assets.map(|a| &a.font);
    ui.draw(should_show_input, show_cursor, font, None);

    PreLobbyStep::Continue
}

fn transition(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    _assets: Option<&Assets>,
) -> Option<PreLobbyStep> {
    let state = std::mem::take(&mut session.state);

    match state {
        ClientState::PreLobby(mut pre_lobby_state) => {
            let transition = match &mut pre_lobby_state {
                PreLobby::ServerAddress { .. } => {
                    state_handlers::server_address::handle(&mut pre_lobby_state, session, ui)
                }
                PreLobby::ApiRequestMenu { .. } => {
                    state_handlers::api_request_menu::handle(&mut pre_lobby_state, session, ui)
                }
            };

            use state_handlers::api_request_menu::PreLobbyTransition;
            match transition {
                PreLobbyTransition::Stay => {
                    session.state = ClientState::PreLobby(pre_lobby_state);
                    return None;
                }
                PreLobbyTransition::NextState(new_state) => {
                    ui.flush_input();
                    session.transition(new_state);
                    return None;
                }
                PreLobbyTransition::Complete(info) => {
                    return Some(PreLobbyStep::Complete(info));
                }
                PreLobbyTransition::Exit => {
                    session.state = ClientState::PreLobby(pre_lobby_state);
                    return Some(PreLobbyStep::Exit);
                }
                PreLobbyTransition::ExitPendingUserAck => {
                    session.state = ClientState::PreLobby(pre_lobby_state);
                    return Some(PreLobbyStep::ExitPendingUserAck);
                }
            }
        }
        other_state => {
            session.state = other_state;
            return None;
        }
    }
}
