use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::state::{ClientSession, ClientState};
use crate::{
    net::NetworkHandle,
    ui::{ClientUi, UiInputError},
};
use shared::auth::{MAX_ATTEMPTS, Passcode};
use shared::net::AppChannel;
use shared::{
    chat::MAX_CHAT_MESSAGE_BYTES,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Authenticating { .. }) {
        panic!(
            "called auth::handle() when state was not Authenticating; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));

                if message.starts_with("Authentication successful!") {
                    return Some(ClientState::ChoosingUsername {
                        prompt_printed: false,
                        awaiting_confirmation: false,
                    });
                } else if message.starts_with("Incorrect passcode. Try again.") {
                    if let ClientState::Authenticating {
                        waiting_for_input,
                        guesses_left,
                    } = session.state_mut()
                    {
                        *guesses_left = guesses_left.saturating_sub(1);
                        ui.show_sanitized_prompt(&passcode_prompt(*guesses_left));
                        *waiting_for_input = true;
                    }
                } else if message.starts_with("Incorrect passcode. Disconnecting.") {
                    return Some(ClientState::Disconnected {
                        message: "Authentication failed.".to_string(),
                    });
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    if let ClientState::Authenticating {
        waiting_for_input,
        guesses_left,
    } = session.state_mut()
    {
        if *waiting_for_input {
            match ui.poll_input(MAX_CHAT_MESSAGE_BYTES) {
                Ok(Some(input_string)) => {
                    if let Some(passcode) = parse_passcode_input(&input_string) {
                        ui.show_sanitized_message("Sending new guess...");

                        let message = ClientMessage::SendPasscode(passcode.bytes);
                        let payload = encode_to_vec(&message, standard())
                            .expect("failed to serialize SendPasscode");
                        network.send_message(AppChannel::ReliableOrdered, payload);

                        *waiting_for_input = false;
                    } else {
                        ui.show_sanitized_error(&format!(
                            "Invalid format: {}. Please enter a 6-digit number.",
                            input_string
                        ));
                        ui.show_sanitized_message(
                            &format!(
                                "Please type a new 6-digit passcode and press Enter. ({} guesses remaining.)",
                                *guesses_left
                            ),
                        );
                    }
                }
                Ok(None) => {}
                Err(UiInputError::Disconnected) => {
                    return Some(ClientState::Disconnected {
                        message: "input thread disconnected.".to_string(),
                    });
                }
            }
        }
    }

    if network.is_disconnected() {
        return Some(ClientState::Disconnected {
            message: format!(
                "Disconnected while authenticating: {}.",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}

pub fn passcode_prompt(remaining: u8) -> String {
    if remaining == MAX_ATTEMPTS {
        format!("Passcode ({} guesses): ", remaining)
    } else {
        format!(
            "Please enter new 6-digit passcode. ({} guesses remaining): ",
            remaining
        )
    }
}

pub fn parse_passcode_input(input: &str) -> Option<Passcode> {
    let s = input.trim();
    if s.len() == 6 && s.chars().all(|c| c.is_ascii_digit()) {
        let mut bytes = vec![0u8; 6];
        for (i, c) in s.chars().enumerate() {
            bytes[i] = c.to_digit(10).unwrap() as u8;
        }
        return Some(Passcode {
            bytes,
            string: s.to_string(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{MockNetwork, MockUi};
    use shared::auth::MAX_ATTEMPTS;

    mod guards {
        use super::*;
        use crate::{
            state::{ClientSession, ClientState},
            test_helpers::{MockNetwork, MockUi},
        };

        #[test]
        #[should_panic(
            expected = "called auth::handle() when state was not Authenticating; current state: Startup"
        )]
        fn authenticating_panics_if_not_in_authenticating_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn authenticating_does_not_panic_in_authenticating_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Authenticating {
                waiting_for_input: false,
                guesses_left: MAX_ATTEMPTS,
            });
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }
    }

    #[test]
    fn requests_new_guess_after_incorrect_passcode_message() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::Authenticating {
            waiting_for_input: false,
            guesses_left: MAX_ATTEMPTS,
        });

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::ServerInfo {
            message: "Incorrect passcode. Try again.".to_string(),
        });

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_none());
        assert_eq!(
            ui.messages,
            vec!["Server: Incorrect passcode. Try again.".to_string()]
        );
        assert_eq!(ui.prompts, vec![passcode_prompt(MAX_ATTEMPTS - 1)]);

        match session.state() {
            ClientState::Authenticating {
                waiting_for_input,
                guesses_left,
            } => {
                assert!(*waiting_for_input);
                assert_eq!(*guesses_left, MAX_ATTEMPTS - 1);
            }
            other => panic!("expected Authenticating state, found {:?}", other),
        }
    }

    #[test]
    fn parses_valid_passcode_input() {
        let input = "123456\n";
        let passcode = parse_passcode_input(input).expect("valid passcode expected");
        assert_eq!(passcode.string, "123456");
        assert_eq!(passcode.bytes, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn rejects_invalid_passcode_input() {
        assert!(parse_passcode_input("abc123").is_none());
        assert!(parse_passcode_input("12345").is_none());
    }

    #[test]
    fn trims_whitespace_around_passcode_input() {
        let input = "  098765  \n";
        let passcode = parse_passcode_input(input).expect("valid passcode expected after trimming");
        assert_eq!(passcode.string, "098765");
        assert_eq!(passcode.bytes, vec![0, 9, 8, 7, 6, 5]);
    }

    #[test]
    fn rejects_passcode_with_internal_whitespace() {
        assert!(parse_passcode_input("12 3456").is_none());
        assert!(parse_passcode_input("1 234 56").is_none());
    }
}
