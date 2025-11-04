mod state;

use crate::state::{interpret_auth_message, AuthMessageOutcome, ClientSession, ClientState, MAX_ATTEMPTS};
use std::io::{stdin, stdout, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand;
use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};

use shared::auth::Passcode;

fn main() {
    run_client();
}

fn run_client() {
    let rx = spawn_input_thread();

    let private_key = client_private_key();
    let client_id = rand::random::<u64>();
    let server_addr = default_server_addr();
    let protocol_id = protocol_version();
    let current_time = current_time();
    let connect_token = create_connect_token(current_time, protocol_id, client_id, server_addr, &private_key);

    let socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind socket");
    let authentication = ClientAuthentication::Secure { connect_token };
    let mut transport =
        NetcodeClientTransport::new(current_time, authentication, socket).expect("Failed to create transport");
    let connection_config = ConnectionConfig::default();
    let mut client = RenetClient::new(connection_config);

    println!(
        "Connecting to {} with client ID: {}",
        server_addr, client_id
    );

    let mut session = ClientSession::new();

    main_loop(
        &mut session,
        &rx,
        &mut client,
        &mut transport,
        client_id,
    );

    println!("Client shutting down");
}

fn spawn_input_thread() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();

    thread::spawn(move || loop {
        let mut buffer = String::new();
        match stdin().read_line(&mut buffer) {
            Ok(_) => {
                if tx.send(buffer.trim().to_string()).is_err() {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Input thread error: {}", e);
                break;
            }
        }
    });

    rx
}

fn client_private_key() -> [u8; 32] {
    [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123, 88, 129, 160,
        222, 33, 251, 99, 37, 145, 18, 199, 199,
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
        .expect(
            "Your system clock appears to be incorrect--it's set to a date before 1970!",
        )
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
    rx: &mpsc::Receiver<String>,
    client: &mut RenetClient,
    transport: &mut NetcodeClientTransport,
    client_id: u64,
) {
    let mut last_updated = Instant::now();

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        if let Err(e) = transport.update(duration, client) {
            if apply_transition(
                session,
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
            ClientState::Startup { .. } => process_startup(session, rx),
            ClientState::Connecting => process_connecting(session, client, transport),
            ClientState::Authenticating { .. } => process_authenticating(session, rx, client, transport),
            ClientState::InGame => process_in_game(session, client, transport, client_id),
            ClientState::Disconnected { .. } => None,
        };

        if let Some(new_state) = next_state {
            if apply_transition(session, new_state) {
                break;
            }
            continue;
        }

        if let Err(e) = transport.send_packets(client) {
            if apply_transition(
                session,
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

fn process_startup(session: &mut ClientSession, rx: &mpsc::Receiver<String>) -> Option<ClientState> {
    if let ClientState::Startup { prompt_printed } = session.state_mut() {
        if !*prompt_printed {
            print!("Passcode ({} guesses): ", MAX_ATTEMPTS);
            stdout().flush().unwrap();
            *prompt_printed = true;
        }

        match rx.try_recv() {
            Ok(input_string) => {
                if let Some(passcode) = parse_passcode_input(&input_string) {
                    session.store_first_passcode(passcode);
                    Some(ClientState::Connecting)
                } else {
                    eprintln!("Invalid format. Please enter a 6-digit number.");
                    print!("Passcode ({} guesses): ", MAX_ATTEMPTS);
                    stdout().flush().unwrap();
                    None
                }
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(ClientState::Disconnected {
                message: "Input thread disconnected.".to_string(),
            }),
        }
    } else {
        None
    }
}

fn process_connecting(
    session: &mut ClientSession,
    client: &mut RenetClient,
    transport: &NetcodeClientTransport,
) -> Option<ClientState> {
    if client.is_connected() {
        if let Some(passcode) = session.take_first_passcode() {
            println!("Transport connected. Sending passcode: {}", passcode.string);
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
    rx: &mpsc::Receiver<String>,
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
            println!("Server: {}", text);

            match interpret_auth_message(&text, guesses_left) {
                AuthMessageOutcome::Authenticated => {
                    println!("Authenticated successfully!");
                    println!("Entering main game loop...");
                    return Some(ClientState::InGame);
                }
                AuthMessageOutcome::RequestNewGuess(remaining) => {
                    print!(
                        "Please enter new 6-digit passcode. ({} guesses remaining): ",
                        remaining
                    );
                    stdout().flush().expect("Failed to flush stdout");
                    *waiting_for_input = true;
                }
                AuthMessageOutcome::Disconnect(message) => {
                    return Some(ClientState::Disconnected { message });
                }
                AuthMessageOutcome::None => {}
            }
        }

        if *waiting_for_input {
            match rx.try_recv() {
                Ok(input_string) => {
                    if let Some(passcode) = parse_passcode_input(&input_string) {
                        println!("Sending new guess...");
                        client.send_message(DefaultChannel::ReliableOrdered, passcode.bytes);
                        *waiting_for_input = false;
                    } else {
                        eprintln!(
                            "Invalid format: {}. Please enter a 6-digit number.",
                            input_string
                        );
                        println!(
                            "Please type a new 6-digit passcode and press Enter. ({} guesses remaining)",
                            *guesses_left
                        );
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
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

fn process_in_game(
    session: &mut ClientSession,
    client: &mut RenetClient,
    transport: &NetcodeClientTransport,
    client_id: u64,
) -> Option<ClientState> {
    let message_count = session.tick_message_counter();

    if message_count % 120 == 0 {
        let message = format!(
            "Hello from client {}! (message {})",
            client_id,
            message_count / 120
        );
        client.send_message(DefaultChannel::ReliableOrdered, message.as_bytes().to_vec());
        println!("Sent: {}", message);
    }

    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let text = String::from_utf8_lossy(&message);
        println!("Server: {}", text);
    }

    if client.is_disconnected() {
        Some(ClientState::Disconnected {
            message: format!(
                "Disconnected from game: {}",
                get_disconnect_reason(client, transport)
            ),
        })
    } else {
        None
    }
}

fn apply_transition(session: &mut ClientSession, new_state: ClientState) -> bool {
    session.transition(new_state);
    if let ClientState::Disconnected { message } = session.state() {
        println!("{}", message);
        true
    } else {
        false
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let passcode = parse_passcode_input(input).expect("Expected passcode with whitespace trimmed");
        assert_eq!(passcode.string, "098765");
        assert_eq!(passcode.bytes, vec![0, 9, 8, 7, 6, 5]);
    }

    #[test]
    fn rejects_passcode_with_internal_whitespace() {
        assert!(parse_passcode_input("12 3456").is_none());
        assert!(parse_passcode_input("1 234 56").is_none());
    }
}
