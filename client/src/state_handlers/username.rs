use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::{
    net::NetworkHandle,
    session::{ClientSession, username_prompt, validate_username_input},
    state::ClientState,
    ui::ClientUi,
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
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::Welcome { username }, _)) => {
                ui.show_sanitized_message(&format!("Welcome, {}!", username));
                return Some(ClientState::InChat);
            }
            Ok((ServerMessage::UsernameError { message }, _)) => {
                ui.show_sanitized_error(&format!("Username error: {}", message));
                ui.show_sanitized_message("Please try a different username.");
                if let ClientState::ChoosingUsername {
                    prompt_printed,
                    awaiting_confirmation,
                } = session.state_mut()
                {
                    *awaiting_confirmation = false;
                    *prompt_printed = false;
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    if let Some(input) = session.take_input() {
        if let ClientState::ChoosingUsername {
            prompt_printed,
            awaiting_confirmation,
        } = session.state_mut()
        {
            if !*awaiting_confirmation {
                let trimmed_input = input.trim();

                if trimmed_input.is_empty() {
                    ui.show_sanitized_error("Username must not be empty.");
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

                        *awaiting_confirmation = true;
                    }
                    Err(err) => {
                        let message = err.to_string();
                        ui.show_sanitized_error(&message);
                        *prompt_printed = false;
                    }
                }
            }
        }
    }

    if let ClientState::ChoosingUsername {
        prompt_printed,
        awaiting_confirmation,
    } = session.state_mut()
    {
        if !*awaiting_confirmation && !*prompt_printed {
            ui.show_sanitized_prompt(&username_prompt());
            *prompt_printed = true;
        }
    }

    if network.is_disconnected() {
        return Some(ClientState::Disconnected {
            message: format!(
                "Disconnected while choosing username: {}.",
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
    use shared::player::MAX_USERNAME_LENGTH;
    use shared::protocol::ServerMessage;

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
            session.transition(ClientState::ChoosingUsername {
                prompt_printed: false,
                awaiting_confirmation: false,
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
    fn enforces_max_username_length() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: false,
            awaiting_confirmation: false,
        });

        let long_name = "A".repeat(MAX_USERNAME_LENGTH as usize + 1);
        session.add_input(long_name.clone());

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();

        handle(&mut session, &mut ui, &mut network);

        assert_eq!(
            network.sent_messages.len(),
            0,
            "packet was sent despite invalid username length."
        );
        assert_eq!(ui.errors.len(), 1, "expected exactly one error message.");
        assert!(
            ui.errors[0].contains("too long"),
            "UI error message did not contain 'too long'."
        );

        if let ClientState::ChoosingUsername {
            prompt_printed,
            awaiting_confirmation,
        } = session.state()
        {
            assert_eq!(
                *prompt_printed, true,
                "'prompt_printed' flag was not reset to true after validation failure"
            );

            assert_eq!(
                *awaiting_confirmation, false,
                "'awaiting_confirmation' flag was not reset to false"
            );
        } else {
            panic!("state unexpectedly changed");
        }
    }

    #[test]
    fn sanitizes_control_characters() {
        let bad_char = "\x07";

        let mut session_user = ClientSession::new(0);
        session_user.transition(ClientState::ChoosingUsername {
            prompt_printed: false,
            awaiting_confirmation: false,
        });
        let mut ui_user = MockUi::new();
        let mut network_user = MockNetwork::new();

        let malicious_error = ServerMessage::UsernameError {
            message: format!("NameTaken{}", bad_char),
        };
        network_user.queue_server_message(malicious_error);

        handle(&mut session_user, &mut ui_user, &mut network_user);

        assert_eq!(ui_user.errors.len(), 1);
        assert_eq!(ui_user.errors[0], "Username error: NameTaken");

        assert_eq!(ui_user.messages.len(), 1);
        assert_eq!(ui_user.messages[0], "Please try a different username.");
    }

    #[test]
    fn sanitizes_ansi() {
        let esc = "\x1B[31mMalicious Text\x1B[0m";

        let mut session_user = ClientSession::new(0);
        session_user.transition(ClientState::ChoosingUsername {
            prompt_printed: false,
            awaiting_confirmation: false,
        });

        let mut ui_user = MockUi::new();
        let mut network_user = MockNetwork::new();

        let malicious_error = ServerMessage::UsernameError {
            message: format!("NameTaken{}", esc),
        };
        network_user.queue_server_message(malicious_error);

        handle(&mut session_user, &mut ui_user, &mut network_user);

        assert_eq!(ui_user.errors.len(), 1);
        assert_eq!(ui_user.errors[0], "Username error: NameTakenMalicious Text");
        assert_eq!(ui_user.messages.len(), 1);
        assert_eq!(ui_user.messages[0], "Please try a different username.");
    }

    #[test]
    fn handles_local_validation_error() {
        let mut session = ClientSession::new(0);
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: true,
            awaiting_confirmation: false,
        });

        session.add_input("   ".to_string());

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();

        handle(&mut session, &mut ui, &mut network);

        assert_eq!(network.sent_messages.len(), 0);
        assert_eq!(ui.errors.len(), 1);
        assert!(ui.errors[0].contains("Username must not be empty"));

        if let ClientState::ChoosingUsername {
            prompt_printed,
            awaiting_confirmation,
        } = session.state()
        {
            assert_eq!(*prompt_printed, false);
            assert_eq!(*awaiting_confirmation, false);
        } else {
            panic!("State unexpectedly changed");
        }
    }
}
