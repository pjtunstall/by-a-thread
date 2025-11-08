pub mod state;
#[cfg(test)]
pub mod test_helpers;

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

use renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};

use crate::state::{AuthAttemptOutcome, Lobby, MAX_AUTH_ATTEMPTS, evaluate_passcode_attempt};
use shared::{
    self,
    auth::Passcode,
    chat::{MAX_USERNAME_LENGTH, UsernameError, sanitize_username},
    net::AppChannel,
};

use shared::chat::MAX_CHAT_MESSAGE_BYTES;

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
    fn broadcast_message_except(&mut self, client_id: u64, channel: AppChannel, message: Vec<u8>);
}

pub fn run_server() {
    let private_key = shared::auth::private_key();
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

    let mut state = Lobby::new();

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

pub struct RenetServerNetworkHandle<'a> {
    pub server: &'a mut RenetServer,
}

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

    fn broadcast_message_except(&mut self, client_id: u64, channel: AppChannel, message: Vec<u8>) {
        self.server
            .broadcast_message_except(client_id, DefaultChannel::from(channel), message);
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
    state: &mut Lobby,
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

pub fn process_events(network: &mut dyn ServerNetworkHandle, state: &mut Lobby) {
    while let Some(event) = network.get_event() {
        match event {
            ServerNetworkEvent::ClientConnected { client_id } => {
                println!("Client {} connected.", client_id);
                state.register_connection(client_id);
            }
            ServerNetworkEvent::ClientDisconnected { client_id, reason } => {
                println!("Client {} disconnected: {}.", client_id, reason);
                state.remove_client(client_id, network);
            }
        }
    }
}

pub fn handle_messages(
    network: &mut dyn ServerNetworkHandle,
    state: &mut Lobby,
    passcode: &Passcode,
) {
    for client_id in network.clients_id() {
        while let Some(message) = network.receive_message(client_id, AppChannel::ReliableOrdered) {
            if message.len() > MAX_CHAT_MESSAGE_BYTES {
                println!(
                    "Client {} sent an overly long message; ignoring.",
                    client_id
                );
                continue;
            }
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
                        state.remove_client(client_id, network);
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
                            state.set_host(client_id, network);
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
                        network.broadcast_message_except(
                            client_id,
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
    use crate::state::Lobby;
    use crate::test_helpers::MockServerNetwork;
    use shared::auth::Passcode;

    #[test]
    fn test_process_events_client_connect() {
        let mut network = MockServerNetwork::new();
        let mut state = Lobby::new();

        network.queue_event(ServerNetworkEvent::ClientConnected { client_id: 1 });

        process_events(&mut network, &mut state);

        assert!(state.is_authenticating(1));
    }

    #[test]
    fn test_process_events_client_disconnect_with_username() {
        let mut network = MockServerNetwork::new();
        let mut state = Lobby::new();

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
        let mut state = Lobby::new();
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
        let mut state = Lobby::new();
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
        let mut state = Lobby::new();
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
        assert!(
            !bob_msgs.contains(&"Bob joined the chat.".to_string()),
            "Bob should not be told that he himself joined"
        );

        let alice_msgs = network.get_sent_messages(1);
        assert!(
            alice_msgs.contains(&"Bob joined the chat.".to_string()),
            "Alice should have been told that Bob joined"
        );
    }

    #[test]
    fn test_handle_messages_chat_message() {
        let mut network = MockServerNetwork::new();
        let mut state = Lobby::new();
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
