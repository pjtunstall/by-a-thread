use bincode::{config::standard, serde::decode_from_slice, serde::encode_to_vec};

use crate::{
    lobby::ui::LobbyUi,
    net::{DisconnectKind, NetworkHandle},
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{
    auth::MAX_ATTEMPTS,
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(
        session.state(),
        ClientState::Lobby(Lobby::Connecting { .. })
    ) {
        panic!(
            "called connecting::handle() when state was not Connecting; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                return Some(ClientState::Disconnected { message });
            }
            Ok((_, _)) => {}
            Err(_) => {}
        }
    }

    if network.is_connected() {
        let passcode = match session.state_mut() {
            ClientState::Lobby(Lobby::Connecting { pending_passcode }) => pending_passcode.take(),
            _ => None,
        };

        if let Some(passcode) = passcode {
            ui.show_message(&format!(
                "Transport connected. Sending passcode: {}.",
                passcode.string
            ));

            let message = ClientMessage::SendPasscode(passcode.bytes);
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize SendPasscode");
            network.send_message(AppChannel::ReliableOrdered, payload);

            Some(ClientState::Lobby(Lobby::Authenticating {
                waiting_for_input: false,
                waiting_for_server: true,
                guesses_left: MAX_ATTEMPTS,
            }))
        } else {
            Some(ClientState::Lobby(Lobby::Authenticating {
                waiting_for_input: true,
                waiting_for_server: false,
                guesses_left: MAX_ATTEMPTS,
            }))
        }
    } else if network.is_disconnected() {
        let reason = network.get_disconnect_reason();
        let message = match network.disconnect_kind() {
            DisconnectKind::DisconnectedByServer | DisconnectKind::ConnectionDenied => {
                common::protocol::GAME_ALREADY_STARTED_MESSAGE.to_string()
            }
            _ => format!("Connection failed: {}.", reason),
        };

        Some(ClientState::Disconnected { message })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        net::DisconnectKind,
        test_helpers::{MockNetwork, MockUi},
    };
    use common::protocol::{ServerMessage, GAME_ALREADY_STARTED_MESSAGE};

    mod guards {
        use super::*;
        use crate::{test_helpers::MockNetwork, test_helpers::MockUi};

        #[test]
        #[should_panic(
            expected = "called connecting::handle() when state was not Connecting; current state: Lobby(Startup { prompt_printed: false })"
        )]
        fn connecting_panics_if_not_in_connecting_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn connecting_does_not_panic_in_connecting_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Lobby(Lobby::Connecting {
                pending_passcode: None,
            }));
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }
    }

    #[test]
    fn server_info_disconnects_during_connecting() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::Connecting {
            pending_passcode: None,
        }));
        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::ServerInfo {
            message: GAME_ALREADY_STARTED_MESSAGE.to_string(),
        });

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(matches!(
            next_state,
            Some(ClientState::Disconnected { message }) if message == GAME_ALREADY_STARTED_MESSAGE
        ));
        assert!(
            ui.errors.is_empty(),
            "disconnecting info should defer messaging to global handler"
        );
    }

    #[test]
    fn disconnect_reason_mapping_game_already_started_on_disconnect() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::Connecting {
            pending_passcode: None,
        }));
        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.set_disconnected(true, "connection terminated by server");
        network.set_disconnect_kind(DisconnectKind::DisconnectedByServer);

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(matches!(
            next_state,
            Some(ClientState::Disconnected { ref message })
                if message == GAME_ALREADY_STARTED_MESSAGE
        ));
    }

    #[test]
    fn disconnect_reason_mapping_other_reasons_remain_generic() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Lobby(Lobby::Connecting {
            pending_passcode: None,
        }));
        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.set_disconnected(true, "dns failure");
        network.set_disconnect_kind(DisconnectKind::Other(
            "dns failure".to_string(),
        ));

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(matches!(
            next_state,
            Some(ClientState::Disconnected { ref message })
                if message == "Connection failed: dns failure."
        ));
    }
}
