use std::time::Duration;

use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::state::{
    ClientSession, ClientState, MAX_ATTEMPTS, username_prompt, validate_username_input,
};
use crate::ui::{ClientUi, UiInputError};
pub use shared::net::AppChannel;
use shared::{
    auth::Passcode,
    chat::MAX_CHAT_MESSAGE_BYTES,
    input::UiKey,
    protocol::{ClientMessage, ServerMessage},
};

pub trait NetworkHandle {
    fn is_connected(&self) -> bool;
    fn is_disconnected(&self) -> bool;
    fn get_disconnect_reason(&self) -> String;
    fn send_message(&mut self, channel: AppChannel, message: Vec<u8>);
    fn receive_message(&mut self, channel: AppChannel) -> Option<Vec<u8>>;
    fn rtt(&self) -> f64;
}

pub fn startup(session: &mut ClientSession, ui: &mut dyn ClientUi) -> Option<ClientState> {
    if let ClientState::Startup { prompt_printed } = session.state_mut() {
        if !*prompt_printed {
            ui.show_prompt(&passcode_prompt(MAX_ATTEMPTS));
            *prompt_printed = true;
        }

        match ui.poll_input(MAX_CHAT_MESSAGE_BYTES) {
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
                message: "input thread disconnected.".to_string(),
            }),
        }
    } else {
        panic!(
            "called startup() when state was not Startup; current state: {:?}",
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
            "called connecting() when state was not Connecting; current state: {:?}",
            session.state()
        );
    }

    if network.is_connected() {
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
            "called authenticating() when state was not Authenticating; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::GameStarted { maze }, _)) => {
                session.maze = Some(maze);
            }
            Ok((ServerMessage::CountdownStarted { end_time }, _)) => {
                session.countdown_end_time = Some(end_time);
                return Some(ClientState::Countdown);
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_message(&format!("Server: {}", message));

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
                        ui.show_prompt(&passcode_prompt(*guesses_left));
                        *waiting_for_input = true;
                    }
                } else if message.starts_with("Incorrect passcode. Disconnecting.") {
                    return Some(ClientState::Disconnected {
                        message: "Authentication failed.".to_string(),
                    });
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_message(&format!("[Deserialization error: {}]", e)),
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
                        ui.show_message("Sending new guess...");

                        let message = ClientMessage::SendPasscode(passcode.bytes);
                        let payload = encode_to_vec(&message, standard())
                            .expect("failed to serialize SendPasscode");
                        network.send_message(AppChannel::ReliableOrdered, payload);

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

pub fn choosing_username(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::ChoosingUsername { .. }) {
        panic!(
            "called choosing_username() when state was not ChoosingUsername; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::GameStarted { maze }, _)) => {
                session.maze = Some(maze);
            }
            Ok((ServerMessage::CountdownStarted { end_time }, _)) => {
                session.countdown_end_time = Some(end_time);
                return Some(ClientState::Countdown);
            }
            Ok((ServerMessage::Welcome { username }, _)) => {
                ui.show_message(&format!("Welcome, {}!", username));
                session.expect_initial_roster();
                return Some(ClientState::InChat);
            }
            Ok((ServerMessage::UsernameError { message }, _)) => {
                ui.show_message(&format!("Username error: {}", message));
                ui.show_message("Please try a different username.");
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
            Err(e) => ui.show_message(&format!("[Deserialization error: {}]", e)),
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

            match ui.poll_input(MAX_CHAT_MESSAGE_BYTES) {
                Ok(Some(input)) => {
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
                            ui.show_error(&message);
                            *prompt_printed = false;
                        }
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
            "called in_chat() when state was not InChat; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::GameStarted { maze }, _)) => {
                session.maze = Some(maze);
            }
            Ok((ServerMessage::CountdownStarted { end_time }, _)) => {
                session.countdown_end_time = Some(end_time);
                return Some(ClientState::Countdown);
            }
            Ok((ServerMessage::RequestDifficultyChoice, _)) => {
                return Some(ClientState::ChoosingDifficulty {
                    prompt_printed: false,
                });
            }
            Ok((ServerMessage::ChatMessage { username, content }, _)) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_message(&format!("{}: {}", username, content));
            }
            Ok((ServerMessage::UserJoined { username }, _)) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_message(&format!("{} joined the chat.", username));
            }
            Ok((ServerMessage::UserLeft { username }, _)) => {
                if session.awaiting_initial_roster() {
                    continue;
                }
                ui.show_message(&format!("{} left the chat.", username));
            }
            Ok((ServerMessage::Roster { online }, _)) => {
                let msg = if online.is_empty() {
                    "You are the only player online.".to_string()
                } else {
                    format!("Players online: {}", online.join(", "))
                };
                ui.show_message(&msg);
                session.mark_initial_roster_received();
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_message(&format!("Server: {}", message));
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_message(&format!("[Deserialization error: {}]", e)),
        }
    }

    loop {
        match ui.poll_input(MAX_CHAT_MESSAGE_BYTES) {
            Ok(Some(input)) => {
                if !input.trim().is_empty() {
                    let message = if input == shared::auth::START_COUNTDOWN {
                        ClientMessage::RequestStartGame
                    } else {
                        ClientMessage::SendChat(input)
                    };

                    let payload =
                        encode_to_vec(&message, standard()).expect("failed to serialize chat");
                    network.send_message(AppChannel::ReliableOrdered, payload);
                }
            }
            Ok(None) => break,
            Err(UiInputError::Disconnected) => {
                return Some(ClientState::Disconnected {
                    message: "input thread disconnected.".to_string(),
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

pub fn countdown(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Countdown) {
        panic!(
            "called countdown() when state was not Countdown; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::GameStarted { maze }, _)) => {
                session.maze = Some(maze);
            }
            Ok((ServerMessage::ChatMessage { username, content }, _)) => {
                ui.show_message(&format!("{}: {}", username, content));
            }
            Ok((ServerMessage::UserJoined { username }, _)) => {
                ui.show_message(&format!("{} joined the chat.", username));
            }
            Ok((ServerMessage::UserLeft { username }, _)) => {
                ui.show_message(&format!("{} left the chat.", username));
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_message(&format!("[Deserialization error: {}]", e)),
        }
    }

    if let Some(end_time) = session.countdown_end_time {
        let time_remaining_secs = end_time - session.estimated_server_time - 1.0;

        if time_remaining_secs > 0.0 {
            let status_message = format!("Time Remaining: {:.0}s", time_remaining_secs.ceil());
            ui.show_status_line(&status_message);
        } else {
            ui.show_status_line("Time Remaining: 0s");

            if let Some(maze) = session.maze.take() {
                std::thread::sleep(Duration::from_millis(100));
                println!("\r");
                ui.show_message("Game started! Maze received:");

                let maze_layout = maze.log();
                for line in maze_layout.lines() {
                    ui.show_message(line);
                }

                ui.show_message("Exiting for now.");
                return Some(ClientState::InGame);
            } else {
                ui.show_status_line("Waiting for maze data...");
            }
        }
    } else {
        ui.show_status_line("Waiting for countdown info...");
    }

    if let Err(UiInputError::Disconnected) = ui.poll_input(0) {
        return Some(ClientState::Disconnected {
            message: "input thread disconnected.".to_string(),
        });
    }

    None
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

pub fn choosing_difficulty(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let is_correct_state = matches!(session.state(), ClientState::ChoosingDifficulty { .. });
    if !is_correct_state {
        panic!(
            "called choosing_difficulty() when state was not ChoosingDifficulty; current state: {:?}",
            session.state()
        );
    };

    if let ClientState::ChoosingDifficulty { prompt_printed } = session.state_mut() {
        if !*prompt_printed {
            ui.show_message("Server: Choose a difficulty level:");
            ui.show_message("  1. Easy");
            ui.show_message("  2. So-so");
            ui.show_message("  3. Next level");
            ui.show_prompt("Enter 1, 2, or 3: ");
            *prompt_printed = true;
        }
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((ServerMessage::GameStarted { maze }, _)) => {
                session.maze = Some(maze);
            }
            Ok((ServerMessage::CountdownStarted { end_time }, _)) => {
                session.countdown_end_time = Some(end_time);
                return Some(ClientState::Countdown);
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_message(&format!("Server: {}", message));
                if let ClientState::ChoosingDifficulty { prompt_printed } = session.state_mut() {
                    *prompt_printed = false;
                }
            }
            Ok((ServerMessage::ChatMessage { username, content }, _)) => {
                ui.show_message(&format!("{}: {}", username, content));
            }
            Ok((ServerMessage::UserJoined { username }, _)) => {
                ui.show_message(&format!("{} joined the chat.", username));
            }
            Ok((ServerMessage::UserLeft { username }, _)) => {
                ui.show_message(&format!("{} left the chat.", username));
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_message(&format!("[Deserialization error: {}]", e)),
        }
    }

    match ui.poll_single_key() {
        Ok(Some(key)) => {
            let level = match key {
                UiKey::Char('1') => Some(1),
                UiKey::Char('2') => Some(2),
                UiKey::Char('3') => Some(3),
                _ => None,
            };

            if let Some(level) = level {
                let msg = ClientMessage::SetDifficulty(level);
                let payload =
                    encode_to_vec(&msg, standard()).expect("failed to serialize SetDifficulty");
                network.send_message(AppChannel::ReliableOrdered, payload);
            }
        }
        Ok(None) => {}
        Err(UiInputError::Disconnected) => {
            return Some(ClientState::Disconnected {
                message: "input disconnected.".to_string(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::net::SocketAddr;

    use super::*;
    use crate::state::ClientSession;
    use crate::ui::{ClientUi, UiInputError};
    use bincode::{config::standard, serde::encode_to_vec};

    #[derive(Default)]
    pub struct MockUi {
        pub messages: Vec<String>,
        pub errors: Vec<String>,
        pub prompts: Vec<String>,
        pub status_lines: Vec<String>,
        pub inputs: VecDeque<Result<Option<String>, UiInputError>>,
        pub keys: VecDeque<Result<Option<UiKey>, UiInputError>>,
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

    impl MockUi {
        pub fn new() -> Self {
            Self {
                messages: Vec::new(),
                errors: Vec::new(),
                prompts: Vec::new(),
                status_lines: Vec::new(),
                inputs: VecDeque::new(),
                keys: VecDeque::new(),
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

        fn show_status_line(&mut self, message: &str) {
            self.status_lines.push(message.to_string());
        }

        fn poll_input(&mut self, _limit: usize) -> Result<Option<String>, UiInputError> {
            self.inputs
                .pop_front()
                .unwrap_or(Ok(None))
                .map(|opt| opt.map(|s| s.to_string()))
        }

        fn poll_single_key(&mut self) -> Result<Option<UiKey>, UiInputError> {
            self.keys.pop_front().unwrap_or(Ok(None))
        }

        fn print_client_banner(
            &mut self,
            protocol_id: u64,
            server_addr: SocketAddr,
            client_id: u64,
        ) {
            self.messages.push(format!(
                "Client Banner: Protocol={}, Server={}, ClientID={}",
                protocol_id, server_addr, client_id
            ));
        }
    }

    #[derive(Default)]
    struct MockNetwork {
        is_connected_val: bool,
        is_disconnected_val: bool,
        disconnect_reason_val: String,
        messages_to_receive: VecDeque<Vec<u8>>,
        sent_messages: VecDeque<(AppChannel, Vec<u8>)>,
        rtt_val: f64,
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

        fn queue_server_message(&mut self, message: ServerMessage) {
            let data =
                encode_to_vec(&message, standard()).expect("failed to serialize test message");
            self.messages_to_receive.push_back(data);
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
            self.sent_messages.push_back((channel, message));
        }

        fn receive_message(&mut self, _channel: AppChannel) -> Option<Vec<u8>> {
            self.messages_to_receive.pop_front()
        }

        fn rtt(&self) -> f64 {
            self.rtt_val
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
                assert_eq!(message, "input thread disconnected.");
            }
            _ => panic!("unexpected transition: expected disconnection"),
        }
    }

    #[test]
    fn authenticating_requests_new_guess_after_incorrect_passcode_message() {
        let mut session = ClientSession::new();
        session.transition(ClientState::Authenticating {
            waiting_for_input: false,
            guesses_left: MAX_ATTEMPTS,
        });

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::ServerInfo {
            message: "Incorrect passcode. Try again.".to_string(),
        });

        let next_state = authenticating(&mut session, &mut ui, &mut network);

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
            expected = "called startup() when state was not Startup; current state: Connecting"
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
                "should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "called connecting() when state was not Connecting; current state: Startup"
        )]
        fn connecting_panics_if_not_in_connecting_state() {
            let mut session = ClientSession::new();
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
                "should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "called authenticating() when state was not Authenticating; current state: Startup"
        )]
        fn authenticating_panics_if_not_in_authenticating_state() {
            let mut session = ClientSession::new();
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
                "should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "called choosing_username() when state was not ChoosingUsername; current state: Startup"
        )]
        fn choosing_username_panics_if_not_in_choosing_username_state() {
            let mut session = ClientSession::new();
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
                "should not panic and should return None"
            );
        }

        #[test]
        #[should_panic(
            expected = "called in_chat() when state was not InChat; current state: Startup"
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

    fn setup_choosing_difficulty_tests() -> (ClientSession, MockUi, MockNetwork) {
        let session = ClientSession {
            state: ClientState::ChoosingDifficulty {
                prompt_printed: false,
            },
            ..ClientSession::new()
        };
        let ui = MockUi::new();
        let network = MockNetwork::new();
        (session, ui, network)
    }

    #[test]
    fn test_choosing_difficulty_prints_prompt() {
        let (mut session, mut ui, mut network) = setup_choosing_difficulty_tests();

        let next_state = choosing_difficulty(&mut session, &mut ui, &mut network);

        assert_eq!(ui.messages.len(), 4);
        assert_eq!(ui.messages[0], "Server: Choose a difficulty level:");
        assert_eq!(ui.messages[1], "  1. Easy");
        assert_eq!(ui.prompts[0], "Enter 1, 2, or 3: ");
        assert!(matches!(
            session.state(),
            ClientState::ChoosingDifficulty {
                prompt_printed: true
            }
        ));
        assert!(next_state.is_none());
    }

    #[test]
    fn test_choosing_difficulty_selects_level_2() {
        let (mut session, mut ui, mut network) = setup_choosing_difficulty_tests();

        choosing_difficulty(&mut session, &mut ui, &mut network);

        ui.keys.push_back(Ok(Some(UiKey::Char('2'))));

        let next_state = choosing_difficulty(&mut session, &mut ui, &mut network);

        assert!(next_state.is_none());
        assert_eq!(network.sent_messages.len(), 1);

        let (channel, payload) = network.sent_messages.pop_front().unwrap();
        assert_eq!(channel, AppChannel::ReliableOrdered);

        let (msg, _) = decode_from_slice::<ClientMessage, _>(&payload, standard()).unwrap();
        assert_eq!(msg, ClientMessage::SetDifficulty(2));
    }

    #[test]
    fn test_choosing_difficulty_ignores_invalid_key() {
        let (mut session, mut ui, mut network) = setup_choosing_difficulty_tests();

        choosing_difficulty(&mut session, &mut ui, &mut network);

        ui.keys.push_back(Ok(Some(UiKey::Char('a'))));
        ui.keys.push_back(Ok(Some(UiKey::Enter)));
        ui.keys.push_back(Ok(Some(UiKey::Char('5'))));

        let next_state_1 = choosing_difficulty(&mut session, &mut ui, &mut network);
        let next_state_2 = choosing_difficulty(&mut session, &mut ui, &mut network);
        let next_state_3 = choosing_difficulty(&mut session, &mut ui, &mut network);

        assert!(next_state_1.is_none());
        assert!(next_state_2.is_none());
        assert!(next_state_3.is_none());
        assert!(network.sent_messages.is_empty());
        assert!(ui.errors.is_empty());
    }

    #[test]
    fn test_choosing_difficulty_handles_disconnect() {
        let (mut session, mut ui, mut network) = setup_choosing_difficulty_tests();

        choosing_difficulty(&mut session, &mut ui, &mut network);

        ui.keys.push_back(Err(UiInputError::Disconnected));

        let next_state = choosing_difficulty(&mut session, &mut ui, &mut network);

        assert!(network.sent_messages.is_empty());
        assert!(matches!(next_state, Some(ClientState::Disconnected { .. })));
    }
}
