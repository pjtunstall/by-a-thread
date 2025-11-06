pub mod state;

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::state::{AuthAttemptOutcome, MAX_AUTH_ATTEMPTS, ServerState, evaluate_passcode_attempt};

use renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use shared::auth::Passcode;
use shared::chat::{MAX_USERNAME_LENGTH, UsernameError, sanitize_username};

pub fn run_server() {
    let private_key = server_private_key();
    let server_addr = server_address();
    let socket = bind_socket(server_addr);

    let current_time = current_time();
    let protocol_id = protocol_version();

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

fn server_private_key() -> [u8; 32] {
    [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ]
}

fn server_address() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000)
}

fn bind_socket(addr: SocketAddr) -> UdpSocket {
    UdpSocket::bind(addr).expect("Failed to bind socket")
}

fn current_time() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect(
            "Your system clock appears to be incorrect--it's set to a date before 1970! Please open your system's date and time settings and enable automatic time synchronization (NTP). On most Linux systems, try `timedatectl set-ntp true`. On non-systemd distros (like Alpine or Gentoo), use `rc-service ntpd start` or `rc-service chronyd start` instead.",
        )
}

fn protocol_version() -> u64 {
    env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("Failed to get major version")
        .parse()
        .expect("Failed to parse major version")
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
            .expect("Failed to update transport");
        server.update(duration);

        process_events(server, state);
        handle_messages(server, state, passcode);

        transport.send_packets(server);
        thread::sleep(Duration::from_millis(16));
    }
}

pub fn process_events(server: &mut RenetServer, state: &mut ServerState) {
    while let Some(event) = server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                println!("Client {} connected", client_id);
                state.register_connection(client_id);
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!("Client {} disconnected: {}", client_id, reason);
                if let Some(username) = state.remove_client(client_id) {
                    let message = format!("{} left the chat.", username);
                    broadcast_message(server, &message);
                }
            }
        }
    }
}

pub fn handle_messages(server: &mut RenetServer, state: &mut ServerState, passcode: &Passcode) {
    for client_id in server.clients_id() {
        while let Some(message) = server.receive_message(client_id, DefaultChannel::ReliableOrdered)
        {
            if state.is_authenticating(client_id) {
                let (outcome, attempts_count) = {
                    let attempts_entry = state
                        .authentication_attempts(client_id)
                        .expect("Expected authentication state for client");
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
                        server.send_message(
                            client_id,
                            DefaultChannel::ReliableOrdered,
                            prompt.as_bytes().to_vec(),
                        );
                    }
                    AuthAttemptOutcome::TryAgain => {
                        println!(
                            "Client {} sent wrong passcode (Attempt {}).",
                            client_id, attempts_count
                        );

                        let try_again_msg = "Incorrect passcode. Try again.".as_bytes().to_vec();
                        server.send_message(
                            client_id,
                            DefaultChannel::ReliableOrdered,
                            try_again_msg,
                        );
                    }
                    AuthAttemptOutcome::Disconnect => {
                        println!("Client {} failed authentication. Disconnecting.", client_id);
                        let error_msg = "Incorrect passcode. Disconnecting.".as_bytes().to_vec();
                        server.send_message(client_id, DefaultChannel::ReliableOrdered, error_msg);
                        server.disconnect(client_id);
                        state.remove_client(client_id);
                    }
                }
            } else if state.needs_username(client_id) {
                let text = String::from_utf8_lossy(&message).to_string();

                match sanitize_username(&text) {
                    Ok(username) => {
                        if state.is_username_taken(&username) {
                            send_username_error(server, client_id, "Username is already taken.");
                            continue;
                        }

                        state.register_username(client_id, &username);

                        let welcome = format!("Welcome, {}!", username);
                        server.send_message(
                            client_id,
                            DefaultChannel::ReliableOrdered,
                            welcome.as_bytes().to_vec(),
                        );

                        let others = state.usernames_except(client_id);
                        if others.is_empty() {
                            server.send_message(
                                client_id,
                                DefaultChannel::ReliableOrdered,
                                "You are the first player online.".as_bytes().to_vec(),
                            );
                        } else {
                            let list = others.join(", ");
                            let message = format!("Players online: {}", list);
                            server.send_message(
                                client_id,
                                DefaultChannel::ReliableOrdered,
                                message.as_bytes().to_vec(),
                            );
                        }

                        let join_announcement = format!("{} joined the chat.", username);
                        broadcast_message(server, &join_announcement);
                    }
                    Err(err) => {
                        let error_text = match err {
                            UsernameError::Empty => "Username must not be empty.",
                            UsernameError::TooLong => "Username is too long.",
                            UsernameError::InvalidCharacter(_) => {
                                "Username contains invalid characters."
                            }
                        };
                        send_username_error(server, client_id, error_text);
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
                    broadcast_message(server, &chat_message);
                }
            }
        }
    }
}

fn send_username_error(server: &mut RenetServer, client_id: u64, message: &str) {
    let payload = format!("Username error: {}", message);
    server.send_message(
        client_id,
        DefaultChannel::ReliableOrdered,
        payload.as_bytes().to_vec(),
    );
}

fn broadcast_message(server: &mut RenetServer, message: &str) {
    let payload = message.as_bytes().to_vec();
    for recipient in server.clients_id() {
        server.send_message(recipient, DefaultChannel::ReliableOrdered, payload.clone());
    }
}
