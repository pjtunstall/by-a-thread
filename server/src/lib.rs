pub mod state;

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

use crate::state::{AuthAttemptOutcome, MAX_AUTH_ATTEMPTS, ServerState, evaluate_passcode_attempt};

use renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use shared::{
    self,
    auth::Passcode,
    chat::{MAX_USERNAME_LENGTH, UsernameError, sanitize_username},
    net::AppChannel,
};

pub enum ServerNetworkEvent {
    ClientConnected { client_id: u64 },
    ClientDisconnected { client_id: u64, reason: String },
}

pub trait ServerNetworkHandle {
    fn get_event(&mut self) -> Option<ServerNetworkEvent>;
    fn clients_id(&self) -> Vec<u64>;
    fn receive_message(&mut self, client_id: u64, channel: AppChannel) -> Option<Vec<u8>>;
    fn send_message(&mut self, client_id: u64, channel: AppChannel, message: Vec<u8>);
    fn broadcast_message(&mut self, channel: AppChannel, message: Vec<u8>);
    fn disconnect(&mut self, client_id: u64);
}

pub fn run_server() {
    let private_key = shared::net::private_key();
    let server_addr = server_address();
    let socket = bind_socket(server_addr);

    let current_time = shared::current_time();
    let protocol_id = shared::protocol_version();

    let server_config = build_server_config(current_time, protocol_id, server_addr, private_key);
    let mut transport =
        NetcodeServerTransport::new(server_config, socket).expect("Failed to create transport");

    let connection_config = ConnectionConfig::default();
    let mut server = RenetServer::new(connection_config);

    let passcode = Passcode::generate(6);
    print_server_banner(protocol_id, server_addr, &passcode);

    let mut state = ServerState::new();

    server_loop(&mut server, &mut transport, &mut state, &passcode);
}

fn server_address() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000)
}

fn bind_socket(addr: SocketAddr) -> UdpSocket {
    UdpSocket::bind(addr).expect("Failed to bind socket")
}

fn build_server_config(
    current_time: Duration,
    protocol_id: u64,
    server_addr: SocketAddr,
    private_key: [u8; 32],
) -> ServerConfig {
    ServerConfig {
        current_time,
        max_clients: 10,
        protocol_id,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Secure { private_key },
    }
}

fn print_server_banner(protocol_id: u64, server_addr: SocketAddr, passcode: &Passcode) {
    println!("  Game version: {}", protocol_id);
    println!("  Server address: {}", server_addr);
    println!("  Passcode: {}", passcode.string);
}

// --- Step 4: Create the RenetServerNetworkHandle wrapper ---
pub struct RenetServerNetworkHandle<'a> {
    pub server: &'a mut RenetServer,
}

// --- Step 5: Implement the trait for the wrapper ---
impl ServerNetworkHandle for RenetServerNetworkHandle<'_> {
    fn get_event(&mut self) -> Option<ServerNetworkEvent> {
        self.server.get_event().map(|event| match event {
            ServerEvent::ClientConnected { client_id } => {
                ServerNetworkEvent::ClientConnected { client_id }
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                ServerNetworkEvent::ClientDisconnected {
                    client_id,
                    reason: reason.to_string(),
                }
            }
        })
    }

    fn clients_id(&self) -> Vec<u64> {
        self.server.clients_id()
    }

    fn receive_message(&mut self, client_id: u64, channel: AppChannel) -> Option<Vec<u8>> {
        self.server
            .receive_message(client_id, DefaultChannel::from(channel))
            .map(|bytes| bytes.to_vec())
    }

    fn send_message(&mut self, client_id: u64, channel: AppChannel, message: Vec<u8>) {
        self.server
            .send_message(client_id, DefaultChannel::from(channel), message);
    }

    fn broadcast_message(&mut self, channel: AppChannel, message: Vec<u8>) {
        self.server
            .broadcast_message(DefaultChannel::from(channel), message);
    }

    fn disconnect(&mut self, client_id: u64) {
        self.server.disconnect(client_id);
    }
}

fn server_loop(
    server: &mut RenetServer,
    transport: &mut NetcodeServerTransport,
    state: &mut ServerState,
    passcode: &Passcode,
) {
    let mut last_updated = Instant::now();

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        transport
            .update(duration, server)
            .expect("failed to update transport");
        server.update(duration);

        let mut network_handle = RenetServerNetworkHandle { server };

        process_events(&mut network_handle, state);
        handle_messages(&mut network_handle, state, passcode);

        transport.send_packets(server);
        thread::sleep(Duration::from_millis(16));
    }
}

pub fn process_events(network: &mut dyn ServerNetworkHandle, state: &mut ServerState) {
    while let Some(event) = network.get_event() {
        match event {
            ServerNetworkEvent::ClientConnected { client_id } => {
                println!("Client {} connected.", client_id);
                state.register_connection(client_id);
            }
            ServerNetworkEvent::ClientDisconnected { client_id, reason } => {
                println!("Client {} disconnected: {}.", client_id, reason);
                if let Some(username) = state.remove_client(client_id) {
                    let message = format!("{} left the chat.", username);
                    network.broadcast_message(AppChannel::ReliableOrdered, message.into_bytes());
                }
            }
        }
    }
}

pub fn handle_messages(
    network: &mut dyn ServerNetworkHandle,
    state: &mut ServerState,
    passcode: &Passcode,
) {
    for client_id in network.clients_id() {
        while let Some(message) = network.receive_message(client_id, AppChannel::ReliableOrdered) {
            if state.is_authenticating(client_id) {
                let (outcome, attempts_count) = {
                    let attempts_entry = state
                        .authentication_attempts(client_id)
                        .expect("expected authentication state for client");
                    let outcome = evaluate_passcode_attempt(
                        passcode.bytes.as_slice(),
                        attempts_entry,
                        message.as_ref(),
                        MAX_AUTH_ATTEMPTS,
                    );
                    let count = *attempts_entry;
                    (outcome, count)
                };

                match outcome {
                    AuthAttemptOutcome::Authenticated => {
                        println!("Client {} authenticated successfully.", client_id);
                        state.mark_authenticated(client_id);

                        let prompt = format!(
                            "Authentication successful! Please enter a username (1-{} characters).",
                            MAX_USERNAME_LENGTH
                        );
                        network.send_message(
                            client_id,
                            AppChannel::ReliableOrdered,
                            prompt.as_bytes().to_vec(),
                        );
                    }
                    AuthAttemptOutcome::TryAgain => {
                        println!(
                            "Client {} sent wrong passcode (Attempt {}).",
                            client_id, attempts_count
                        );

                        let try_again_msg = "Incorrect passcode. Try again.".as_bytes().to_vec();
                        network.send_message(client_id, AppChannel::ReliableOrdered, try_again_msg);
                    }
                    AuthAttemptOutcome::Disconnect => {
                        println!("Client {} failed authentication. Disconnecting.", client_id);
                        let error_msg = "Incorrect passcode. Disconnecting.".as_bytes().to_vec();
                        network.send_message(client_id, AppChannel::ReliableOrdered, error_msg);
                        network.disconnect(client_id);
                        state.remove_client(client_id);
                    }
                }
            } else if state.needs_username(client_id) {
                let text = String::from_utf8_lossy(&message).to_string();

                match sanitize_username(&text) {
                    Ok(username) => {
                        if state.is_username_taken(&username) {
                            send_username_error(network, client_id, "Username is already taken.");
                            continue;
                        }

                        state.register_username(client_id, &username);
                        println!("Client {} set username to '{}'.", client_id, username);

                        let welcome = format!("Welcome, {}!", username);
                        network.send_message(
                            client_id,
                            AppChannel::ReliableOrdered,
                            welcome.as_bytes().to_vec(),
                        );

                        let others = state.usernames_except(client_id);
                        if others.is_empty() {
                            network.send_message(
                                client_id,
                                AppChannel::ReliableOrdered,
                                "You are the only player online.".as_bytes().to_vec(),
                            );
                        } else {
                            let list = others.join(", ");
                            let message = format!("Players online: {}", list);
                            network.send_message(
                                client_id,
                                AppChannel::ReliableOrdered,
                                message.as_bytes().to_vec(),
                            );
                        }

                        let join_announcement = format!("{} joined the chat.", username);
                        network.broadcast_message(
                            AppChannel::ReliableOrdered,
                            join_announcement.into_bytes(),
                        );
                    }
                    Err(err) => {
                        let error_text = match err {
                            UsernameError::Empty => "Username must not be empty.",
                            UsernameError::TooLong => "Username is too long.",
                            UsernameError::InvalidCharacter(_) => {
                                "Username contains invalid characters."
                            }
                        };
                        send_username_error(network, client_id, error_text);
                    }
                }
            } else {
                let text = String::from_utf8_lossy(&message).trim().to_string();
                if text.is_empty() {
                    continue;
                }

                if let Some(username) = state.username(client_id) {
                    println!("{}: {}", username, text);
                    let chat_message = format!("{}: {}", username, text);
                    network
                        .broadcast_message(AppChannel::ReliableOrdered, chat_message.into_bytes());
                }
            }
        }
    }
}

fn send_username_error(network: &mut dyn ServerNetworkHandle, client_id: u64, message: &str) {
    let payload = format!("Username error: {}", message);
    network.send_message(
        client_id,
        AppChannel::ReliableOrdered,
        payload.as_bytes().to_vec(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ServerState;
    use shared::auth::Passcode;
    use std::collections::{HashMap, VecDeque};

    #[derive(Default)]
    struct MockServerNetwork {
        events_to_process: VecDeque<ServerNetworkEvent>,
        client_messages: HashMap<u64, VecDeque<Vec<u8>>>,
        sent_messages: HashMap<u64, Vec<Vec<u8>>>,
        broadcast_messages: Vec<Vec<u8>>,
        disconnected_clients: Vec<u64>,
        client_ids: Vec<u64>,
    }

    impl MockServerNetwork {
        fn new() -> Self {
            Self::default()
        }

        fn add_client(&mut self, client_id: u64) {
            self.client_ids.push(client_id);
            self.client_messages.entry(client_id).or_default();
            self.sent_messages.entry(client_id).or_default();
        }

        fn queue_event(&mut self, event: ServerNetworkEvent) {
            self.events_to_process.push_back(event);
        }

        fn queue_message(&mut self, client_id: u64, message: &str) {
            self.client_messages
                .entry(client_id)
                .or_default()
                .push_back(message.as_bytes().to_vec());
        }

        fn queue_raw_message(&mut self, client_id: u64, message: Vec<u8>) {
            self.client_messages
                .entry(client_id)
                .or_default()
                .push_back(message);
        }

        fn get_sent_messages(&mut self, client_id: u64) -> Vec<String> {
            self.sent_messages
                .entry(client_id)
                .or_default()
                .iter()
                .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                .collect()
        }

        fn get_broadcast_messages(&self) -> Vec<String> {
            self.broadcast_messages
                .iter()
                .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                .collect()
        }
    }

    impl ServerNetworkHandle for MockServerNetwork {
        fn get_event(&mut self) -> Option<ServerNetworkEvent> {
            self.events_to_process.pop_front()
        }

        fn clients_id(&self) -> Vec<u64> {
            self.client_ids.clone()
        }

        fn receive_message(&mut self, client_id: u64, _channel: AppChannel) -> Option<Vec<u8>> {
            self.client_messages
                .entry(client_id)
                .or_default()
                .pop_front()
        }

        fn send_message(&mut self, client_id: u64, _channel: AppChannel, message: Vec<u8>) {
            self.sent_messages
                .entry(client_id)
                .or_default()
                .push(message);
        }

        fn broadcast_message(&mut self, _channel: AppChannel, message: Vec<u8>) {
            self.broadcast_messages.push(message);
        }

        fn disconnect(&mut self, client_id: u64) {
            self.disconnected_clients.push(client_id);
            self.client_ids.retain(|&id| id != client_id);
        }
    }

    #[test]
    fn test_process_events_client_connect() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::new();

        network.queue_event(ServerNetworkEvent::ClientConnected { client_id: 1 });

        process_events(&mut network, &mut state);

        assert!(state.is_authenticating(1));
    }

    #[test]
    fn test_process_events_client_disconnect_with_username() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::new();

        state.register_connection(1);
        state.mark_authenticated(1);
        state.register_username(1, "Alice");

        network.queue_event(ServerNetworkEvent::ClientDisconnected {
            client_id: 1,
            reason: "timeout".to_string(),
        });

        process_events(&mut network, &mut state);

        assert_eq!(state.username(1), None);
        let broadcasts = network.get_broadcast_messages();
        assert_eq!(broadcasts, vec!["Alice left the chat."]);
    }

    #[test]
    fn test_handle_messages_auth_success() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::new();
        let passcode =
            Passcode::from_string("123456").expect("Failed to create passcode from string");

        network.add_client(1);
        state.register_connection(1);

        network.queue_raw_message(1, vec![1, 2, 3, 4, 5, 6]);

        handle_messages(&mut network, &mut state, &passcode);

        assert!(state.needs_username(1));
        let client_msgs = network.get_sent_messages(1);
        assert_eq!(client_msgs.len(), 1);
        assert!(client_msgs[0].starts_with("Authentication successful!"));
    }

    #[test]
    fn test_handle_messages_auth_fail_then_disconnect() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::new();
        let passcode =
            Passcode::from_string("123456").expect("Failed to create passcode from string");

        network.add_client(1);
        state.register_connection(1);

        for _ in 0..MAX_AUTH_ATTEMPTS {
            network.queue_message(1, "000000");
        }

        handle_messages(&mut network, &mut state, &passcode);

        assert_eq!(state.username(1), None);
        assert!(network.disconnected_clients.contains(&1));
        let client_msgs = network.get_sent_messages(1);
        assert!(
            client_msgs
                .last()
                .unwrap()
                .starts_with("Incorrect passcode. Disconnecting.")
        );
    }

    #[test]
    fn test_handle_messages_username_success_and_broadcast() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::new();
        let passcode =
            Passcode::from_string("123456").expect("Failed to create passcode from string");

        network.add_client(1);
        state.register_connection(1);
        state.mark_authenticated(1);
        state.register_username(1, "Alice");

        network.add_client(2);
        state.register_connection(2);
        state.mark_authenticated(2);
        network.queue_message(2, "Bob");

        handle_messages(&mut network, &mut state, &passcode);

        assert_eq!(state.username(2), Some("Bob"));

        let bob_msgs = network.get_sent_messages(2);
        assert!(bob_msgs.contains(&"Welcome, Bob!".to_string()));
        assert!(bob_msgs.contains(&"Players online: Alice".to_string()));

        let broadcasts = network.get_broadcast_messages();
        assert!(broadcasts.contains(&"Bob joined the chat.".to_string()));
    }

    #[test]
    fn test_handle_messages_chat_message() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::new();
        let passcode =
            Passcode::from_string("123456").expect("Failed to create passcode from string");

        network.add_client(1);
        state.register_connection(1);
        state.mark_authenticated(1);
        state.register_username(1, "Alice");

        network.add_client(2);
        state.register_connection(2);
        state.mark_authenticated(2);
        state.register_username(2, "Bob");

        network.queue_message(1, "Hello Bob!");

        handle_messages(&mut network, &mut state, &passcode);

        let broadcasts = network.get_broadcast_messages();
        assert_eq!(broadcasts, vec!["Alice: Hello Bob!"]);
    }
}
