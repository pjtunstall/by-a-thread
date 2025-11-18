pub mod auth;
pub mod chat;
pub mod connecting;
pub mod countdown;
pub mod difficulty;
pub mod game;
pub mod startup;
pub mod username;

use shared::auth::{MAX_ATTEMPTS, Passcode};

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
    use crate::{
        state::{ClientSession, ClientState},
        test_helpers::{MockNetwork, MockUi},
        ui::{ClientUi, UiInputError},
    };
    use shared::protocol::ServerMessage;

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

    #[test]
    fn startup_prompts_only_once_when_waiting_for_input() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::default();

        assert!(startup::handle(&mut session, &mut ui).is_none());
        assert!(ui.prompts.is_empty());

        ui.messages.clear();
        ui.errors.clear();

        assert!(startup::handle(&mut session, &mut ui).is_none());
        assert!(ui.prompts.is_empty());
    }

    #[test]
    fn startup_state_handlers_when_valid_passcode_received() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::with_inputs([Ok(Some("123456".into()))]);

        let next = startup::handle(&mut session, &mut ui);
        assert!(matches!(next, Some(ClientState::Connecting)));
        assert_eq!(session.take_first_passcode().unwrap().string, "123456");
    }

    #[test]
    fn startup_reprompts_after_invalid_passcode() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::with_inputs([Ok(Some("abc".into()))]);

        assert!(startup::handle(&mut session, &mut ui).is_none());
        assert_eq!(
            ui.errors,
            vec!["Invalid format. Please enter a 6-digit number.".to_string()]
        );
        assert_eq!(ui.prompts.len(), 1);
    }

    #[test]
    fn startup_returns_disconnected_when_input_thread_stops() {
        let mut session = ClientSession::new(0);
        let mut ui = MockUi::with_inputs([Err(UiInputError::Disconnected)]);

        let next = startup::handle(&mut session, &mut ui);
        match next {
            Some(ClientState::Disconnected { message }) => {
                assert_eq!(message, "input thread disconnected.");
            }
            _ => panic!("unexpected transition: expected disconnection"),
        }
    }

    #[test]
    fn authenticating_requests_new_guess_after_incorrect_passcode_message() {
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

        let next_state = auth::handle(&mut session, &mut ui, &mut network);

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

    mod panic_guards {
        use super::*;

        #[test]
        #[should_panic(
            expected = "called startup::handle() when state was not Startup; current state: Connecting"
        )]
        fn startup_panics_if_not_in_startup_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Connecting);
            let mut ui = MockUi::default();

            startup::handle(&mut session, &mut ui);
        }

        #[test]
        fn startup_does_not_panic_in_startup_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            assert!(
                startup::handle(&mut session, &mut ui).is_none(),
                "should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "called connecting::handle() when state was not Connecting; current state: Startup"
        )]
        fn connecting_panics_if_not_in_connecting_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            connecting::handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn connecting_does_not_panic_in_connecting_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::Connecting);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                connecting::handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "called auth::handle() when state was not Authenticating; current state: Startup"
        )]
        fn authenticating_panics_if_not_in_authenticating_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            auth::handle(&mut session, &mut ui, &mut network);
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
                auth::handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "called username::handle() when state was not ChoosingUsername; current state: Startup"
        )]
        fn choosing_username_panics_if_not_in_choosing_username_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            username::handle(&mut session, &mut ui, &mut network);
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
                username::handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "called chat::handle() when state was not InChat; current state: Startup"
        )]
        fn in_chat_panics_if_not_in_in_chat_state() {
            let mut session = ClientSession::new(0);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            chat::handle(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn in_chat_does_not_panic_in_in_chat_state() {
            let mut session = ClientSession::new(0);
            session.transition(ClientState::InChat);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                chat::handle(&mut session, &mut ui, &mut network).is_none(),
                "should not panic and should return None"
            );
        }
    }

    #[test]
    fn client_banner_is_printed_correctly() {
        let mut ui = MockUi::default();
        let protocol_id = 12345;
        let server_addr: std::net::SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let client_id = 99;

        let expected_banner =
            "Client Banner: Protocol=12345, Server=127.0.0.1:8080, ClientID=99".to_string();

        ui.print_client_banner(protocol_id, server_addr, client_id);

        assert_eq!(ui.messages, vec![expected_banner]);
        assert!(ui.errors.is_empty());
        assert!(ui.prompts.is_empty());
        assert!(ui.status_lines.is_empty());
    }

    #[test]
    fn test_incoming_server_data_is_sanitized_before_display() {
        let bell = '\x07';
        let esc = '\x1B';

        let mut session_chat = ClientSession::new(0);
        session_chat.transition(ClientState::InChat);
        session_chat.mark_initial_roster_received();
        let mut ui_chat = MockUi::new();
        let mut network_chat = MockNetwork::new();

        let malicious_chat = ServerMessage::ChatMessage {
            username: format!("User{}", bell),
            content: format!("Hello{}World", esc),
        };
        network_chat.queue_server_message(malicious_chat);

        chat::handle(&mut session_chat, &mut ui_chat, &mut network_chat);

        assert_eq!(ui_chat.messages.len(), 1);
        assert_eq!(ui_chat.messages[0], "User: HelloWorld");

        let mut session_user = ClientSession::new(0);
        session_user.transition(ClientState::ChoosingUsername {
            prompt_printed: true,
            awaiting_confirmation: true,
        });
        let mut ui_user = MockUi::new();
        let mut network_user = MockNetwork::new();

        let malicious_error = ServerMessage::UsernameError {
            message: format!("Name{}Taken", bell),
        };
        network_user.queue_server_message(malicious_error);

        username::handle(&mut session_user, &mut ui_user, &mut network_user);

        assert_eq!(ui_user.errors.len(), 1);
        assert_eq!(ui_user.errors[0], "Username error: NameTaken");
        assert_eq!(ui_user.messages.len(), 1);
        assert_eq!(ui_user.messages[0], "Please try a different username.");

        let mut session_auth = ClientSession::new(0);
        session_auth.transition(ClientState::Authenticating {
            waiting_for_input: false,
            guesses_left: MAX_ATTEMPTS,
        });
        let mut ui_auth = MockUi::new();
        let mut network_auth = MockNetwork::new();

        let malicious_info = ServerMessage::ServerInfo {
            message: format!("Incorrect passcode. Try again.{}", esc),
        };
        network_auth.queue_server_message(malicious_info);

        auth::handle(&mut session_auth, &mut ui_auth, &mut network_auth);

        assert_eq!(ui_auth.messages.len(), 1);
        assert_eq!(
            ui_auth.messages[0],
            "Server: Incorrect passcode. Try again."
        );
        assert_eq!(ui_auth.prompts.len(), 1);
        assert_eq!(ui_auth.prompts[0], passcode_prompt(MAX_ATTEMPTS - 1));
    }
}
