use bincode::{config::standard, serde::decode_from_slice};

use crate::{net::NetworkHandle, session::ClientSession, state::ClientState, ui::ClientUi};
use shared::{net::AppChannel, protocol::ServerMessage};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::AwaitingUsernameConfirmation) {
        panic!(
            "called awaiting_confirmation::handle() when state was not AwaitingUsernameConfirmation; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::Welcome { username }, _)) => {
                ui.show_sanitized_message(&format!("Welcome, {}!", username));
                return Some(ClientState::InChat);
            }
            Ok((ServerMessage::UsernameError { message }, _)) => {
                ui.show_sanitized_error(&format!("Username error: {}", message));
                ui.show_sanitized_message("Please try a different username.");

                // Server rejected the username, transition back to ChoosingUsername
                return Some(ClientState::ChoosingUsername {
                    prompt_printed: false,
                });
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    if network.is_disconnected() {
        return Some(ClientState::TransitioningToDisconnected {
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
    use shared::protocol::ServerMessage;

    fn set_awaiting_state(session: &mut ClientSession) {
        session.transition(ClientState::AwaitingUsernameConfirmation);
    }

    #[test]
    #[should_panic(
        expected = "called awaiting_confirmation::handle() when state was not AwaitingUsernameConfirmation; current state: Startup"
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

        assert!(matches!(next_state, Some(ClientState::InChat)));
        assert_eq!(ui.messages.len(), 1);
        assert_eq!(ui.messages[0], "Welcome, TestUser!");
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
            Some(ClientState::ChoosingUsername {
                prompt_printed: false
            })
        ));
        assert_eq!(ui.errors.len(), 1);
        assert_eq!(ui.errors[0], "Username error: Name Taken");
        assert_eq!(ui.messages.len(), 1);
        assert_eq!(ui.messages[0], "Please try a different username.");
    }
}
