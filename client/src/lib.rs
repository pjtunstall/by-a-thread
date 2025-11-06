mod state;
mod ui;

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand;
use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};

use crate::state::{
    AuthMessageOutcome, ClientSession, ClientState, MAX_ATTEMPTS, interpret_auth_message,
    username_prompt, validate_username_input,
};
use crate::ui::{ClientUi, TerminalUi, UiInputError};
use shared::auth::Passcode;

pub fn run_client() {
    let mut ui = TerminalUi::new();

    let private_key = client_private_key();
    let client_id = rand::random::<u64>();
    let server_addr = default_server_addr();
    let protocol_id = protocol_version();
    let current_time = current_time();
    let connect_token = create_connect_token(
        current_time,
        protocol_id,
        client_id,
        server_addr,
        &private_key,
    );

    let socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind socket");
    let authentication = ClientAuthentication::Secure { connect_token };
    let mut transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .expect("Failed to create transport");
    let connection_config = ConnectionConfig::default();
    let mut client = RenetClient::new(connection_config);

    ui.show_message(&format!(
        "Connecting to {} with client ID: {}",
        server_addr, client_id
    ));

    let mut session = ClientSession::new();

    main_loop(&mut session, &mut ui, &mut client, &mut transport);

    ui.show_message("Client shutting down");
}

fn client_private_key() -> [u8; 32] {
    [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ]
}

fn default_server_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000)
}

fn protocol_version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("Failed to get major version")
        .parse()
        .expect("Failed to parse major version")
}

fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Your system clock appears to be incorrect--it's set to a date before 1970!")
}

fn create_connect_token(
    current_time: Duration,
    protocol_id: u64,
    client_id: u64,
    server_addr: SocketAddr,
    private_key: &[u8; 32],
) -> ConnectToken {
    ConnectToken::generate(
        current_time,
        protocol_id,
        3600,
        client_id,
        15,
        vec![server_addr],
        None,
        private_key,
    )
    .expect("Failed to generate token")
}

fn main_loop(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    client: &mut RenetClient,
    transport: &mut NetcodeClientTransport,
) {
    let mut last_updated = Instant::now();

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        if let Err(e) = transport.update(duration, client) {
            if apply_transition(
                session,
                ui,
                ClientState::Disconnected {
                    message: format!("Transport error: {}", e),
                },
            ) {
                break;
            }
            continue;
        }

        client.update(duration);

        let next_state = match session.state() {
            ClientState::Startup { .. } => process_startup(session, ui),
            ClientState::Connecting => process_connecting(session, ui, client, transport),
            ClientState::Authenticating { .. } => {
                process_authenticating(session, ui, client, transport)
            }
            ClientState::ChoosingUsername { .. } => {
                process_choosing_username(session, ui, client, transport)
            }
            ClientState::InChat => process_in_chat(session, ui, client, transport),
            ClientState::Disconnected { .. } => None,
        };

        if let Some(new_state) = next_state {
            if apply_transition(session, ui, new_state) {
                break;
            }
            continue;
        }

        if let Err(e) = transport.send_packets(client) {
            if apply_transition(
                session,
                ui,
                ClientState::Disconnected {
                    message: format!("Error sending packets: {}", e),
                },
            ) {
                break;
            }
        }

        thread::sleep(Duration::from_millis(16));
    }
}

fn process_startup(session: &mut ClientSession, ui: &mut dyn ClientUi) -> Option<ClientState> {
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
        None
    }
}

fn process_connecting(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    client: &mut RenetClient,
    transport: &NetcodeClientTransport,
) -> Option<ClientState> {
    if client.is_connected() {
        if let Some(passcode) = session.take_first_passcode() {
            ui.show_message(&format!(
                "Transport connected. Sending passcode: {}",
                passcode.string
            ));
            client.send_message(DefaultChannel::ReliableOrdered, passcode.bytes);
            Some(ClientState::Authenticating {
                waiting_for_input: false,
                guesses_left: MAX_ATTEMPTS,
            })
        } else {
            Some(ClientState::Disconnected {
                message: "Internal error: No passcode to send.".to_string(),
            })
        }
    } else if client.is_disconnected() {
        Some(ClientState::Disconnected {
            message: format!(
                "Connection failed: {}",
                get_disconnect_reason(client, transport)
            ),
        })
    } else {
        None
    }
}

fn process_authenticating(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    client: &mut RenetClient,
    transport: &NetcodeClientTransport,
) -> Option<ClientState> {
    if let ClientState::Authenticating {
        waiting_for_input,
        guesses_left,
    } = session.state_mut()
    {
        while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
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
                        client.send_message(DefaultChannel::ReliableOrdered, passcode.bytes);
                        *waiting_for_input = false;
                    } else {
                        ui.show_error(&format!(
                            "Invalid format: {}. Please enter a 6-digit number.",
                            input_string
                        ));
                        ui.show_message(&format!(
                            "Please type a new 6-digit passcode and press Enter. ({} guesses remaining)",
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

        if client.is_disconnected() {
            return Some(ClientState::Disconnected {
                message: format!(
                    "Disconnected while authenticating: {}",
                    get_disconnect_reason(client, transport)
                ),
            });
        }
    }

    None
}

fn process_choosing_username(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    client: &mut RenetClient,
    transport: &NetcodeClientTransport,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::ChoosingUsername { .. }) {
        return None;
    }

    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
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

    let (prompt_printed, awaiting_confirmation) = match session.state_mut() {
        ClientState::ChoosingUsername {
            prompt_printed,
            awaiting_confirmation,
        } => (prompt_printed, awaiting_confirmation),
        _ => return None,
    };

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
                        client.send_message(DefaultChannel::ReliableOrdered, username.into_bytes());
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

    if client.is_disconnected() {
        return Some(ClientState::Disconnected {
            message: format!(
                "Disconnected while choosing username: {}",
                get_disconnect_reason(client, transport)
            ),
        });
    }

    None
}

fn process_in_chat(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    client: &mut RenetClient,
    transport: &NetcodeClientTransport,
) -> Option<ClientState> {
    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let text = String::from_utf8_lossy(&message).to_string();
        handle_in_chat_server_message(session, ui, &text);
    }

    loop {
        match ui.poll_input() {
            Ok(Some(input)) => {
                if !input.trim().is_empty() {
                    client.send_message(DefaultChannel::ReliableOrdered, input.into_bytes());
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

    if client.is_disconnected() {
        Some(ClientState::Disconnected {
            message: format!(
                "Disconnected from chat: {}",
                get_disconnect_reason(client, transport)
            ),
        })
    } else {
        None
    }
}

fn apply_transition(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    new_state: ClientState,
) -> bool {
    session.transition(new_state);
    if let ClientState::Disconnected { message } = session.state() {
        ui.show_message(message);
        true
    } else {
        false
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

fn get_disconnect_reason(client: &RenetClient, transport: &NetcodeClientTransport) -> String {
    client
        .disconnect_reason()
        .map(|reason| format!("Renet - {:?}", reason))
        .or_else(|| {
            transport
                .disconnect_reason()
                .map(|reason| format!("Transport - {:?}", reason))
        })
        .unwrap_or_else(|| "No reason given".to_string())
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

    #[test]
    fn parses_valid_passcode_input() {
        let input = "123456\n";
        let passcode = parse_passcode_input(input).expect("Expected valid passcode");
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
        let passcode =
            parse_passcode_input(input).expect("Expected passcode with whitespace trimmed");
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

        assert!(process_startup(&mut session, &mut ui).is_none());
        assert_eq!(ui.prompts, vec![passcode_prompt(MAX_ATTEMPTS)]);

        ui.messages.clear();
        ui.errors.clear();

        assert!(process_startup(&mut session, &mut ui).is_none());
        assert_eq!(ui.prompts, vec![passcode_prompt(MAX_ATTEMPTS)]);
    }

    #[test]
    fn startup_transitions_when_valid_passcode_received() {
        let mut session = ClientSession::new();
        let mut ui = MockUi::with_inputs([Ok(Some("123456".into()))]);

        let next = process_startup(&mut session, &mut ui);
        assert!(matches!(next, Some(ClientState::Connecting)));
        assert_eq!(session.take_first_passcode().unwrap().string, "123456");
    }

    #[test]
    fn startup_reprompts_after_invalid_passcode() {
        let mut session = ClientSession::new();
        let mut ui = MockUi::with_inputs([Ok(Some("abc".into()))]);

        assert!(process_startup(&mut session, &mut ui).is_none());
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

        let next = process_startup(&mut session, &mut ui);
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
    fn choosing_username_transitions_after_welcome() {
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
}
