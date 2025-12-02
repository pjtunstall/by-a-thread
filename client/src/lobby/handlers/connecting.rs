use bincode::{config::standard, serde::decode_from_slice, serde::encode_to_vec};

use crate::{
    lobby::ui::{LobbyUi, UiErrorKind},
    net::NetworkHandle,
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
                if message.starts_with("The game has already started.") {
                    ui.show_message(&message);
                    return Some(ClientState::Disconnected { message });
                }
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
        ui.show_typed_error(
            UiErrorKind::NetworkDisconnect,
            &format!("Connection failed: {}.", network.get_disconnect_reason()),
        );
        Some(ClientState::Disconnected {
            message: format!("Connection failed: {}.", network.get_disconnect_reason()),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
