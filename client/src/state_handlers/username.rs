use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    net::NetworkHandle,
    session::{ClientSession, username_prompt, validate_username_input},
    state::ClientState,
    ui::{ClientUi, UiErrorKind},
};
use shared::{
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::ChoosingUsername { .. }) {
        panic!(
            "called username::handle() when state was not ChoosingUsername; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        if let Ok((msg, _)) = decode_from_slice::<ServerMessage, _>(&data, standard()) {
            if let Some(next) = handle_server_message(session, ui, &msg) {
                return Some(next);
            }
        }
    }

    if let Some(input) = session.take_input() {
        if let ClientState::ChoosingUsername { prompt_printed } = session.state_mut() {
            let trimmed_input = input.trim();

            if trimmed_input.is_empty() {
                ui.show_typed_error(
                    UiErrorKind::UsernameValidation(shared::player::UsernameError::Empty),
                    "Username must not be empty.",
                );
                *prompt_printed = false;
                return None;
            }

            let validation = validate_username_input(&input);
            match validation {
                Ok(username) => {
                    let message = ClientMessage::SetUsername(username);
                    let payload = encode_to_vec(&message, standard())
                        .expect("failed to serialize SetUsername");

                    network.send_message(AppChannel::ReliableOrdered, payload);

                    return Some(ClientState::AwaitingUsernameConfirmation);
                }
                Err(err) => {
                    let message = err.to_string();
                    ui.show_typed_error(UiErrorKind::UsernameValidation(err), &message);
                    *prompt_printed = false;
                }
            }
        }
    }

    if let ClientState::ChoosingUsername { prompt_printed } = session.state_mut() {
        if !*prompt_printed {
            ui.show_sanitized_prompt(&username_prompt());
            *prompt_printed = true;
        }
    }

    if network.is_disconnected() {
        return Some(ClientState::TransitioningToDisconnected {
            message: format!(
                "disconnected while choosing username: {}",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}

pub fn handle_server_message(
    _session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    message: &ServerMessage,
) -> Option<ClientState> {
    if let ServerMessage::UsernameError { message } = message {
        let sanitized: String = message.chars().filter(|c| !c.is_control()).collect();
        ui.show_typed_error(
            UiErrorKind::UsernameServerError,
            &format!("Username error: {}", sanitized),
        );
        return Some(ClientState::ChoosingUsername {
            prompt_printed: false,
        });
    }
    if let ServerMessage::ServerInfo { message } = message {
        ui.show_sanitized_message(&format!("Server: {}", message));
        ui.show_message("Server: Disconnecting.");
        return Some(ClientState::TransitioningToDisconnected {
            message: message.clone(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{MockNetwork, MockUi};
    use shared::player::{MAX_USERNAME_LENGTH, UsernameError};

    fn set_choosing_username(session: &mut ClientSession) {
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: false,
        });
    }

    mod guards {
        use super::*;

        #[test]
        #[should_panic(
            expected = "called username::handle() when state was not ChoosingUsername; current state: Startup"
        )]
        fn choosing_username_panics_if_not_in_choosing_username_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn choosing_username_does_not_panic_in_choosing_username_state() {
            let mut session = ClientSession::new(0);
            set_choosing_username(&mut session);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                handle(&mut session, &mut ui, &mut network).is_none(),
                "should return None when successfully handling state and no input is provided"
            );
        }
    }

    #[test]
    fn enforces_max_username_length() {
        let mut session = ClientSession::new(0);
        set_choosing_username(&mut session);

        let long_name = "A".repeat(MAX_USERNAME_LENGTH as usize + 1);
        session.add_input(long_name.clone());

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();

        handle(&mut session, &mut ui, &mut network);

        assert_eq!(
            network.sent_messages.len(),
            0,
            "no message should be sent to the network for invalid input"
        );
        assert_eq!(
            ui.errors.len(),
            1,
            "exactly one error message should be displayed for invalid input"
        );
        assert_eq!(
            ui.error_kinds,
            vec![UiErrorKind::UsernameValidation(UsernameError::TooLong)]
        );
    }

    #[test]
    fn handles_local_validation_error() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: true,
        });

        session.add_input("   ".to_string());

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();

        handle(&mut session, &mut ui, &mut network);

        assert_eq!(
            network.sent_messages.len(),
            0,
            "no message should be sent to the network for empty input"
        );
        assert_eq!(
            ui.errors.len(),
            1,
            "exactly one error message should be displayed for empty input"
        );
        assert_eq!(
            ui.error_kinds,
            vec![UiErrorKind::UsernameValidation(UsernameError::Empty)]
        );

        if let ClientState::ChoosingUsername { prompt_printed } = session.state() {
            assert_eq!(
                *prompt_printed, false,
                "prompt_printed must be reset to false after an error"
            );
        } else {
            panic!("state unexpectedly changed from ChoosingUsername");
        }
    }

    #[test]
    fn sanitizes_server_username_error() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::AwaitingUsernameConfirmation);
        let mut ui = MockUi::new();
        let bell = '\x07';

        let malicious_error = ServerMessage::UsernameError {
            message: format!("Name{}Taken", bell),
        };

        let next_state = handle_server_message(&mut session, &mut ui, &malicious_error);

        assert_eq!(
            ui.errors.len(),
            1,
            "expected exactly one sanitized error from server message"
        );
        assert_eq!(ui.error_kinds, vec![UiErrorKind::UsernameServerError]);

        match next_state {
            Some(ClientState::ChoosingUsername { prompt_printed }) => {
                assert_eq!(
                    prompt_printed, false,
                    "state should transition to ChoosingUsername with prompt_printed false"
                );
            }
            _ => panic!("expected transition to ChoosingUsername state"),
        }
    }
}
