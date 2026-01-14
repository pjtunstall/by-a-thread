use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    lobby::ui::{LobbyUi, UiErrorKind},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{
    net::AppChannel,
    protocol::{ServerMessage, GAME_ALREADY_STARTED_MESSAGE},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(
        &session.state,
        ClientState::Lobby(Lobby::AwaitingUsernameConfirmation)
    ) {
        panic!(
            "called awaiting_confirmation::handle() when state was not AwaitingUsernameConfirmation; current state: {:?}",
            &session.state
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::Welcome { username }, _)) => {
                ui.show_sanitized_message(&format!("Server: Welcome, {}!", username));
                return Some(ClientState::Lobby(Lobby::Chat {
                    awaiting_initial_roster: true,
                    waiting_for_server: false,
                }));
            }
            Ok((ServerMessage::UsernameError { message }, _)) => {
                ui.show_typed_error(
                    UiErrorKind::UsernameServerError,
                    &format!("Username error: {}", message),
                );
                ui.show_sanitized_message("Please try a different username.");

                // Server rejected the username, transition back to ChoosingUsername.
                return Some(ClientState::Lobby(Lobby::ChoosingUsername {
                    prompt_printed: false,
                }));
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                if message != GAME_ALREADY_STARTED_MESSAGE {
                    ui.show_sanitized_message(&format!("Server: {}", message));
                }
                return Some(ClientState::Disconnected { message });
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[Deserialization error: {}]", e),
            ),
        }
    }

    if network.is_disconnected() {
        return Some(ClientState::Disconnected {
            message: format!(
                "Disconnected while awaiting username confirmation: {}.",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{MockNetwork, MockUi};
    use common::protocol::{ServerMessage, GAME_ALREADY_STARTED_MESSAGE};

    fn set_awaiting_state(session: &mut ClientSession) {
        session.transition(ClientState::Lobby(Lobby::AwaitingUsernameConfirmation));
    }

    #[test]
    #[should_panic(
        expected = "called awaiting_confirmation::handle() when state was not AwaitingUsernameConfirmation; current state: Lobby(Startup { prompt_printed: false })"
    )]
    fn guards_panics_if_not_in_awaiting_confirmation_state() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        handle(&mut session, &mut ui, &mut network);
    }

    #[test]
    fn handles_server_welcome() {
        let mut session = ClientSession::new(0);
        set_awaiting_state(&mut session);

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::Welcome {
            username: "TestUser".to_string(),
        });

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(matches!(
            next_state,
            Some(ClientState::Lobby(Lobby::Chat {
                awaiting_initial_roster: true,
                waiting_for_server: false
            }))
        ));
        assert_eq!(ui.messages.len(), 1);
        assert_eq!(ui.messages[0], "Server: Welcome, TestUser!");
    }

    #[test]
    fn handles_username_error() {
        let mut session = ClientSession::new(0);
        set_awaiting_state(&mut session);

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::UsernameError {
            message: "Name Taken".to_string(),
        });

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(matches!(
            next_state,
            Some(ClientState::Lobby(Lobby::ChoosingUsername {
                prompt_printed: false
            }))
        ));
        assert_eq!(ui.errors.len(), 1);
        assert_eq!(ui.error_kinds, vec![UiErrorKind::UsernameServerError]);
        assert_eq!(ui.messages.len(), 1);
        assert_eq!(ui.messages[0], "Please try a different username.");
    }

    #[test]
    fn handles_server_info_disconnecting() {
        let mut session = ClientSession::new(0);
        set_awaiting_state(&mut session);

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::ServerInfo {
            message: GAME_ALREADY_STARTED_MESSAGE.to_string(),
        });

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(matches!(next_state, Some(ClientState::Disconnected { .. })));
        assert!(
            ui.messages.is_empty(),
            "disconnecting info should defer messaging to global handler"
        );
    }
}
