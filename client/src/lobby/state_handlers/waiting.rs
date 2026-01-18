use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    lobby::ui::{LobbyUi, UiErrorKind},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{
    net::AppChannel,
    protocol::{GAME_ALREADY_STARTED_MESSAGE, ServerMessage},
};

pub fn handle(
    _lobby_state: &mut Lobby,
    _session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::Welcome { username, color }, _)) => {
                ui.set_local_player_color(color);
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
            Err(e) => {
                ui.show_typed_error(
                    UiErrorKind::Deserialization,
                    &format!("[DESERIALIZATION ERROR: {}]", e),
                );
            }
        }
    }

    if network.is_disconnected() {
        return Some(ClientState::Disconnected {
            message: format!(
                "disconnected while awaiting username confirmation: {}",
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
    use common::player::Color;
    use common::protocol::{GAME_ALREADY_STARTED_MESSAGE, ServerMessage};

    fn set_awaiting_state(session: &mut ClientSession) {
        session.transition(ClientState::Lobby(Lobby::AwaitingUsernameConfirmation));
    }

    #[test]
    fn handles_server_welcome() {
        let mut session = ClientSession::new(0);
        set_awaiting_state(&mut session);

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::Welcome {
            username: "TestUser".to_string(),
            color: Color::RED,
        });

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert!(matches!(
            _next_state,
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

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert!(matches!(
            _next_state,
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

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert!(matches!(
            _next_state,
            Some(ClientState::Disconnected { .. })
        ));
        assert!(
            ui.messages.is_empty(),
            "disconnecting info should defer messaging to global handler"
        );
    }
}
