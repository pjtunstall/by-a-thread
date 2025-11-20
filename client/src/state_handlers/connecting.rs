use crate::{
    net::NetworkHandle,
    state::{ClientSession, ClientState},
    ui::ClientUi,
};
use bincode::serde::decode_from_slice;
use bincode::{config::standard, serde::encode_to_vec};
use shared::auth::MAX_ATTEMPTS;
use shared::net::AppChannel;
use shared::protocol::ClientMessage;
use shared::protocol::ServerMessage;

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Connecting) {
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
                    return Some(ClientState::TransitioningToDisconnected { message });
                }
            }
            Ok((_, _)) => {}
            Err(_) => {}
        }
    }

    if network.is_connected() {
        ui.show_status_line("");

        if let Some(passcode) = session.take_first_passcode() {
            ui.show_message(&format!(
                "Transport connected. Sending passcode: {}.",
                passcode.string
            ));

            let message = ClientMessage::SendPasscode(passcode.bytes);
            let payload =
                encode_to_vec(&message, standard()).expect("failed to serialize SendPasscode");
            network.send_message(AppChannel::ReliableOrdered, payload);

            Some(ClientState::Authenticating {
                waiting_for_input: false,
                guesses_left: MAX_ATTEMPTS,
            })
        } else {
            Some(ClientState::Authenticating {
                waiting_for_input: true,
                guesses_left: MAX_ATTEMPTS,
            })
        }
    } else if network.is_disconnected() {
        Some(ClientState::TransitioningToDisconnected {
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
        use crate::{
            state::{ClientSession, ClientState},
            test_helpers::MockNetwork,
            test_helpers::MockUi,
        };

        #[test]
        #[should_panic(
            expected = "called connecting::handle() when state was not Connecting; current state: Startup"
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
            session.transition(ClientState::Connecting);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }
    }
}
