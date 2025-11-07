use crate::state::{
    AuthMessageOutcome, ClientSession, ClientState, MAX_ATTEMPTS, interpret_auth_message,
    username_prompt, validate_username_input,
};
use crate::ui::{ClientUi, UiInputError};
use shared::auth::Passcode;

#[allow(dead_code)]
pub enum AppChannel {
    ReliableOrdered,
    Unreliable,
}

pub trait NetworkHandle {
    fn is_connected(&self) -> bool;
    fn is_disconnected(&self) -> bool;
    fn get_disconnect_reason(&self) -> String;
    fn send_message(&mut self, channel: AppChannel, message: Vec<u8>);
    fn receive_message(&mut self, channel: AppChannel) -> Option<Vec<u8>>;
}

pub fn startup(session: &mut ClientSession, ui: &mut dyn ClientUi) -> Option<ClientState> {
    if let ClientState::Startup { prompt_printed } = session.state_mut() {
        if !*prompt_printed {
            ui.show_prompt(&passcode_prompt(MAX_ATTEMPTS));
            *prompt_printed = true;
        }

        match ui.poll_input() {
            Ok(Some(input_string)) => {
                if let Some(passcode) = parse_passcode_input(&input_string) {
                    session.store_first_passcode(passcode);
                    Some(ClientState::Connecting)
                } else {
                    ui.show_error("Invalid format. Please enter a 6-digit number.");
                    ui.show_prompt(&passcode_prompt(MAX_ATTEMPTS));
                    None
                }
            }
            Ok(None) => None,
            Err(UiInputError::Disconnected) => Some(ClientState::Disconnected {
                message: "Input thread disconnected.".to_string(),
            }),
        }
    } else {
        panic!(
            "BUG: Called startup() when state was not Startup. Current state: {:?}",
            session.state()
        );
    }
}

pub fn connecting(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Connecting) {
        panic!(
            "BUG: Called connecting() when state was not Connecting. Current state: {:?}",
            session.state()
        );
    }

    if network.is_connected() {
        if let Some(passcode) = session.take_first_passcode() {
            ui.show_message(&format!(
                "Transport connected. Sending passcode: {}.",
                passcode.string
            ));
            network.send_message(AppChannel::ReliableOrdered, passcode.bytes);
            Some(ClientState::Authenticating {
                waiting_for_input: false,
                guesses_left: MAX_ATTEMPTS,
            })
        } else {
            Some(ClientState::Disconnected {
                message: "Internal error: No passcode to send.".to_string(),
            })
        }
    } else if network.is_disconnected() {
        Some(ClientState::Disconnected {
            message: format!("Connection failed: {}.", network.get_disconnect_reason()),
        })
    } else {
        None
    }
}

pub fn authenticating(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Authenticating { .. }) {
        panic!(
            "BUG: Called authenticating() when state was not Authenticating. Current state: {:?}",
            session.state()
        );
    }

    if let ClientState::Authenticating {
        waiting_for_input,
        guesses_left,
    } = session.state_mut()
    {
        while let Some(message) = network.receive_message(AppChannel::ReliableOrdered) {
            let text = String::from_utf8_lossy(&message);
            ui.show_message(&format!("Server: {}", text));

            match interpret_auth_message(&text, guesses_left) {
                AuthMessageOutcome::Authenticated => {
                    ui.show_message("Authenticated successfully!");
                    return Some(ClientState::ChoosingUsername {
                        prompt_printed: false,
                        awaiting_confirmation: false,
                    });
                }
                AuthMessageOutcome::RequestNewGuess(remaining) => {
                    ui.show_prompt(&passcode_prompt(remaining));
                    *waiting_for_input = true;
                }
                AuthMessageOutcome::Disconnect(message) => {
                    return Some(ClientState::Disconnected { message });
                }
                AuthMessageOutcome::None => {}
            }
        }

        if *waiting_for_input {
            match ui.poll_input() {
                Ok(Some(input_string)) => {
                    if let Some(passcode) = parse_passcode_input(&input_string) {
                        ui.show_message("Sending new guess...");
                        network.send_message(AppChannel::ReliableOrdered, passcode.bytes);
                        *waiting_for_input = false;
                    } else {
                        ui.show_error(&format!(
                            "Invalid format: {}. Please enter a 6-digit number.",
                            input_string
                        ));
                        ui.show_message(&format!(
                            "Please type a new 6-digit passcode and press Enter. ({} guesses remaining.)",
                            *guesses_left
                        ));
                    }
                }
                Ok(None) => {}
                Err(UiInputError::Disconnected) => {
                    return Some(ClientState::Disconnected {
                        message: "Input thread disconnected.".to_string(),
                    });
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
    }

    None
}

pub fn choosing_username(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::ChoosingUsername { .. }) {
        panic!(
            "BUG: Called choosing_username() when state was not ChoosingUsername. Current state: {:?}",
            session.state()
        );
    }

    while let Some(message) = network.receive_message(AppChannel::ReliableOrdered) {
        let text = String::from_utf8_lossy(&message).to_string();

        if is_participant_announcement(&text) {
            continue;
        }

        if matches!(
            handle_username_server_message(session, ui, &text),
            UsernameMessageResult::TransitionToChat
        ) {
            session.expect_initial_roster();
            return Some(ClientState::InChat);
        }
    }

    if let ClientState::ChoosingUsername {
        prompt_printed,
        awaiting_confirmation,
    } = session.state_mut()
    {
        if !*awaiting_confirmation {
            if !*prompt_printed {
                ui.show_prompt(&username_prompt());
                *prompt_printed = true;
            }

            match ui.poll_input() {
                Ok(Some(input)) => {
                    let validation = validate_username_input(&input);
                    match validation {
                        Ok(username) => {
                            network
                                .send_message(AppChannel::ReliableOrdered, username.into_bytes());
                            *awaiting_confirmation = true;
                        }
                        Err(err) => {
                            let message = err.to_string();
                            ui.show_error(&message);
                            *prompt_printed = false;
                        }
                    }
                }
                Ok(None) => {}
                Err(UiInputError::Disconnected) => {
                    return Some(ClientState::Disconnected {
                        message: "Input thread disconnected.".to_string(),
                    });
                }
            }
        }
    } else {
        unreachable!(
            "BUG: Guard at top of choosing_username failed to panic on mismatched state: {:?}",
            session.state()
        );
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

pub fn in_chat(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::InChat) {
        panic!(
            "BUG: Called in_chat() when state was not InChat. Current state: {:?}",
            session.state()
        );
    }

    while let Some(message) = network.receive_message(AppChannel::ReliableOrdered) {
        let text = String::from_utf8_lossy(&message).to_string();
        handle_in_chat_server_message(session, ui, &text);
    }

    loop {
        match ui.poll_input() {
            Ok(Some(input)) => {
                if !input.trim().is_empty() {
                    network.send_message(AppChannel::ReliableOrdered, input.into_bytes());
                }
            }
            Ok(None) => break,
            Err(UiInputError::Disconnected) => {
                return Some(ClientState::Disconnected {
                    message: "Input thread disconnected.".to_string(),
                });
            }
        }
    }

    if network.is_disconnected() {
        Some(ClientState::Disconnected {
            message: format!(
                "Disconnected from chat: {}.",
                network.get_disconnect_reason()
            ),
        })
    } else {
        None
    }
}

fn passcode_prompt(remaining: u8) -> String {
    if remaining == MAX_ATTEMPTS {
        format!("Passcode ({} guesses): ", remaining)
    } else {
        format!(
            "Please enter new 6-digit passcode. ({} guesses remaining): ",
            remaining
        )
    }
}

fn parse_passcode_input(input: &str) -> Option<Passcode> {
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

#[derive(Debug, PartialEq, Eq)]
enum UsernameMessageResult {
    None,
    TransitionToChat,
}

fn handle_username_server_message(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    text: &str,
) -> UsernameMessageResult {
    if is_participant_announcement(text) {
        return UsernameMessageResult::None;
    }

    let mut result = UsernameMessageResult::None;

    let handled = session.with_choosing_username(|prompt_printed, awaiting_confirmation| {
        ui.show_message(&format!("Server: {}", text));

        if text.starts_with("Username error:") {
            ui.show_message("Please try a different username.");
            *awaiting_confirmation = false;
            *prompt_printed = false;
        } else if text.starts_with("Welcome, ") {
            result = UsernameMessageResult::TransitionToChat;
        }
    });

    if handled.is_none() {
        ui.show_message(&format!("Server: {}", text));
    }

    result
}

fn handle_in_chat_server_message(session: &mut ClientSession, ui: &mut dyn ClientUi, text: &str) {
    if session.awaiting_initial_roster() {
        if is_roster_message(text) {
            ui.show_message(text);
            session.mark_initial_roster_received();
        } else if !is_participant_announcement(text) {
            ui.show_message(text);
        }
        return;
    }

    ui.show_message(text);
}

fn is_participant_announcement(text: &str) -> bool {
    text.ends_with(" joined the chat.") || text.ends_with(" left the chat.")
}

fn is_roster_message(text: &str) -> bool {
    text.starts_with("Players online: ") || text == "You are the first player online."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ClientSession;
    use crate::ui::{ClientUi, UiInputError};
    use std::collections::VecDeque;

    #[derive(Default)]
    struct MockUi {
        inputs: VecDeque<Result<Option<String>, UiInputError>>,
        messages: Vec<String>,
        errors: Vec<String>,
        prompts: Vec<String>,
    }

    impl MockUi {
        fn with_inputs<I>(inputs: I) -> Self
        where
            I: IntoIterator<Item = Result<Option<String>, UiInputError>>,
        {
            Self {
                inputs: inputs.into_iter().collect(),
                ..Default::default()
            }
        }
    }

    impl ClientUi for MockUi {
        fn show_message(&mut self, message: &str) {
            self.messages.push(message.to_string());
        }

        fn show_error(&mut self, message: &str) {
            self.errors.push(message.to_string());
        }

        fn show_prompt(&mut self, prompt: &str) {
            self.prompts.push(prompt.to_string());
        }

        fn poll_input(&mut self) -> Result<Option<String>, UiInputError> {
            self.inputs
                .pop_front()
                .unwrap_or(Ok(None))
                .map(|opt| opt.map(|s| s.to_string()))
        }
    }

    #[derive(Default)]
    struct MockNetwork {
        is_connected_val: bool,
        is_disconnected_val: bool,
        disconnect_reason_val: String,
        messages_to_receive: VecDeque<Vec<u8>>,
        sent_messages: Vec<(AppChannel, Vec<u8>)>,
    }

    impl MockNetwork {
        fn new() -> Self {
            Self::default()
        }

        #[allow(dead_code)]
        fn set_connected(&mut self, connected: bool) {
            self.is_connected_val = connected;
        }

        #[allow(dead_code)]
        fn set_disconnected(&mut self, disconnected: bool, reason: &str) {
            self.is_disconnected_val = disconnected;
            self.disconnect_reason_val = reason.to_string();
        }

        #[allow(dead_code)]
        fn queue_message(&mut self, message: &str) {
            self.messages_to_receive
                .push_back(message.as_bytes().to_vec());
        }
    }

    impl NetworkHandle for MockNetwork {
        fn is_connected(&self) -> bool {
            self.is_connected_val
        }
        fn is_disconnected(&self) -> bool {
            self.is_disconnected_val
        }
        fn get_disconnect_reason(&self) -> String {
            self.disconnect_reason_val.clone()
        }
        fn send_message(&mut self, channel: AppChannel, message: Vec<u8>) {
            self.sent_messages.push((channel, message));
        }
        fn receive_message(&mut self, _channel: AppChannel) -> Option<Vec<u8>> {
            self.messages_to_receive.pop_front()
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

    #[test]
    fn startup_prompts_only_once_when_waiting_for_input() {
        let mut session = ClientSession::new();
        let mut ui = MockUi::default();

        assert!(startup(&mut session, &mut ui).is_none());
        assert_eq!(ui.prompts, vec![passcode_prompt(MAX_ATTEMPTS)]);

        ui.messages.clear();
        ui.errors.clear();

        assert!(startup(&mut session, &mut ui).is_none());
        assert_eq!(ui.prompts, vec![passcode_prompt(MAX_ATTEMPTS)]);
    }

    #[test]
    fn startup_state_handlers_when_valid_passcode_received() {
        let mut session = ClientSession::new();
        let mut ui = MockUi::with_inputs([Ok(Some("123456".into()))]);

        let next = startup(&mut session, &mut ui);
        assert!(matches!(next, Some(ClientState::Connecting)));
        assert_eq!(session.take_first_passcode().unwrap().string, "123456");
    }

    #[test]
    fn startup_reprompts_after_invalid_passcode() {
        let mut session = ClientSession::new();
        let mut ui = MockUi::with_inputs([Ok(Some("abc".into()))]);

        assert!(startup(&mut session, &mut ui).is_none());
        assert_eq!(
            ui.errors,
            vec!["Invalid format. Please enter a 6-digit number.".to_string()]
        );
        assert_eq!(ui.prompts.len(), 2);
    }

    #[test]
    fn startup_returns_disconnected_when_input_thread_stops() {
        let mut session = ClientSession::new();
        let mut ui = MockUi::with_inputs([Err(UiInputError::Disconnected)]);

        let next = startup(&mut session, &mut ui);
        match next {
            Some(ClientState::Disconnected { message }) => {
                assert_eq!(message, "Input thread disconnected.");
            }
            _ => panic!("Unexpected transition: expected disconnection"),
        }
    }

    #[test]
    fn choosing_username_discards_announcements() {
        let mut session = ClientSession::new();
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: false,
            awaiting_confirmation: false,
        });
        let mut ui = MockUi::default();

        handle_username_server_message(&mut session, &mut ui, "Riley joined the chat.");

        assert!(ui.messages.is_empty());
        assert!(ui.errors.is_empty());
        assert!(ui.prompts.is_empty());
    }

    #[test]
    fn choosing_username_retries_after_error_message() {
        let mut session = ClientSession::new();
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: true,
            awaiting_confirmation: true,
        });
        let mut ui = MockUi::default();

        let result = handle_username_server_message(
            &mut session,
            &mut ui,
            "Username error: Username is already taken.",
        );

        assert_eq!(result, UsernameMessageResult::None);
        assert_eq!(
            ui.messages,
            vec![
                "Server: Username error: Username is already taken.".to_string(),
                "Please try a different username.".to_string(),
            ]
        );

        let (prompt_printed, awaiting_confirmation) = session
            .with_choosing_username(|prompt_printed, awaiting_confirmation| {
                (*prompt_printed, *awaiting_confirmation)
            })
            .expect("session should still be choosing a username");

        assert!(!prompt_printed);
        assert!(!awaiting_confirmation);
    }

    #[test]
    fn choosing_username_state_handlers_after_welcome() {
        let mut session = ClientSession::new();
        session.transition(ClientState::ChoosingUsername {
            prompt_printed: false,
            awaiting_confirmation: true,
        });
        let mut ui = MockUi::default();

        let result = handle_username_server_message(&mut session, &mut ui, "Welcome, Pat!");

        assert_eq!(result, UsernameMessageResult::TransitionToChat);
        assert_eq!(ui.messages, vec!["Server: Welcome, Pat!".to_string()]);
    }

    #[test]
    fn chat_discards_announcements_received_before_roster() {
        let mut session = ClientSession::new();
        session.transition(ClientState::InChat);
        session.expect_initial_roster();
        let mut ui = MockUi::default();

        handle_in_chat_server_message(&mut session, &mut ui, "Casey left the chat.");
        assert!(ui.messages.is_empty());

        handle_in_chat_server_message(&mut session, &mut ui, "You are the first player online.");

        assert_eq!(
            ui.messages,
            vec!["You are the first player online.".to_string()]
        );
        assert!(!session.awaiting_initial_roster());

        handle_in_chat_server_message(&mut session, &mut ui, "Morgan joined the chat.");

        assert_eq!(
            ui.messages,
            vec![
                "You are the first player online.".to_string(),
                "Morgan joined the chat.".to_string(),
            ]
        );
    }

    #[test]
    fn chat_surfaces_regular_messages_before_roster_arrives() {
        let mut session = ClientSession::new();
        session.transition(ClientState::InChat);
        session.expect_initial_roster();
        let mut ui = MockUi::default();

        handle_in_chat_server_message(
            &mut session,
            &mut ui,
            "Server: Maintenance starts in 5 minutes.",
        );

        assert_eq!(
            ui.messages,
            vec!["Server: Maintenance starts in 5 minutes.".to_string()]
        );
        assert!(session.awaiting_initial_roster());
    }

    mod panic_guards {
        use super::*;

        #[test]
        #[should_panic(
            expected = "BUG: Called startup() when state was not Startup. Current state: Connecting"
        )]
        fn startup_panics_if_not_in_startup_state() {
            let mut session = ClientSession::new();
            session.transition(ClientState::Connecting);
            let mut ui = MockUi::default();

            startup(&mut session, &mut ui);
        }

        #[test]
        fn startup_does_not_panic_in_startup_state() {
            let mut session = ClientSession::new();
            let mut ui = MockUi::default();
            assert!(
                startup(&mut session, &mut ui).is_none(),
                "Should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "BUG: Called connecting() when state was not Connecting. Current state: Startup"
        )]
        fn connecting_panics_if_not_in_connecting_state() {
            let mut session = ClientSession::new(); // Starts in Startup
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            connecting(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn connecting_does_not_panic_in_connecting_state() {
            let mut session = ClientSession::new();
            session.transition(ClientState::Connecting);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                connecting(&mut session, &mut ui, &mut network).is_none(),
                "Should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "BUG: Called authenticating() when state was not Authenticating. Current state: Startup"
        )]
        fn authenticating_panics_if_not_in_authenticating_state() {
            let mut session = ClientSession::new(); // Starts in Startup
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            authenticating(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn authenticating_does_not_panic_in_authenticating_state() {
            let mut session = ClientSession::new();
            session.transition(ClientState::Authenticating {
                waiting_for_input: false,
                guesses_left: MAX_ATTEMPTS,
            });
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                authenticating(&mut session, &mut ui, &mut network).is_none(),
                "Should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "BUG: Called choosing_username() when state was not ChoosingUsername. Current state: Startup"
        )]
        fn choosing_username_panics_if_not_in_choosing_username_state() {
            let mut session = ClientSession::new(); // Starts in Startup
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            choosing_username(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn choosing_username_does_not_panic_in_choosing_username_state() {
            let mut session = ClientSession::new();
            session.transition(ClientState::ChoosingUsername {
                prompt_printed: false,
                awaiting_confirmation: false,
            });
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                choosing_username(&mut session, &mut ui, &mut network).is_none(),
                "Should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "BUG: Called in_chat() when state was not InChat. Current state: Startup"
        )]
        fn in_chat_panics_if_not_in_in_chat_state() {
            let mut session = ClientSession::new();
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();

            in_chat(&mut session, &mut ui, &mut network);
        }

        #[test]
        fn in_chat_does_not_panic_in_in_chat_state() {
            let mut session = ClientSession::new();
            session.transition(ClientState::InChat);
            let mut ui = MockUi::default();
            let mut network = MockNetwork::new();
            assert!(
                in_chat(&mut session, &mut ui, &mut network).is_none(),
                "Should not panic and should return None"
            );
        }
    }
}
