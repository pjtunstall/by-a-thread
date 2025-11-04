use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand;
use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};

use shared::auth::Passcode;

enum ClientState {
    Connecting,
    Authenticating,
    InGame,
    Disconnected { message: String },
}

trait PasscodeExt {
    fn new() -> Option<Self>
    where
        Self: Sized;
}

impl PasscodeExt for Passcode {
    // Tries to get a valid 6-digit passcode from the user within 3 attempts.
    // Returns `Some(Passcode)` on success, or `None` if attempts are exhausted.
    fn new() -> Option<Self> {
        const MAX_ATTEMPTS: usize = 3;

        for attempt in 0..MAX_ATTEMPTS {
            let passcode_input = prompt("Passcode: ").expect("Failed to read passcode input");

            if passcode_input.len() == 6 && passcode_input.chars().all(|c| c.is_ascii_digit()) {
                let mut bytes = vec![0u8; 6];
                for (i, c) in passcode_input.chars().enumerate() {
                    bytes[i] = c
                        .to_digit(10)
                        .expect("Character could not be parsed as digit")
                        as u8;
                }

                return Some(Passcode {
                    bytes,
                    string: passcode_input,
                });
            } else {
                let attempts_left = (MAX_ATTEMPTS - 1) - attempt;
                if attempts_left > 0 {
                    println!(
                        "Invalid passcode. Please enter a 6-digit number. ({} attempts remaining)",
                        attempts_left
                    );
                } else {
                    println!("Invalid passcode. That was your last attempt.");
                }
            }
        }

        None
    }
}

fn main() {
    let private_key: [u8; 32] = [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ];

    let client_id = rand::random::<u64>();

    // let server_addr_input = prompt("Server address (e.g. 127.0.0.1:5000): ");
    // let server_addr: SocketAddr = server_addr_input
    //     .parse()
    //     .expect("Failed to parse server address");
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);

    let protocol_id = env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("Failed to get major version")
        .parse()
        .unwrap();

    let passcode = match Passcode::new() {
        Some(pc) => pc,
        None => {
            println!("Failed to provide a valid passcode. Exiting.");
            return;
        }
    };
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Your system clock appears to be incorrect--it's set to a date before 1970!");

    let connect_token = ConnectToken::generate(
        current_time,
        protocol_id,
        3600,
        client_id,
        15,
        vec![server_addr],
        None,
        &private_key,
    )
    .expect("Failed to generate token");

    let socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind socket");
    let authentication = ClientAuthentication::Secure { connect_token };
    let mut transport = NetcodeClientTransport::new(current_time, authentication, socket)
        .expect("Failed to create transport");
    let connection_config = ConnectionConfig::default();
    let mut client = RenetClient::new(connection_config);

    println!(
        "Connecting to {} with client ID: {}",
        server_addr, client_id
    );

    let mut state = ClientState::Connecting;
    let auth_timeout = Instant::now() + Duration::from_secs(10);
    let mut last_updated = Instant::now();
    let mut message_count = 0;

    'main_loop: loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        client.update(duration);
        if let Err(e) = transport.update(duration, &mut client) {
            state = ClientState::Disconnected {
                message: format!("Transport error: {}", e),
            };
        }

        match state {
            ClientState::Connecting => {
                if client.is_connected() {
                    println!("Transport connected. Sending passcode: {}", passcode.string);
                    client.send_message(DefaultChannel::ReliableOrdered, passcode.bytes.clone());
                    state = ClientState::Authenticating;
                } else if client.is_disconnected() {
                    state = ClientState::Disconnected {
                        message: format!(
                            "Connection failed: {}",
                            get_disconnect_reason(&client, &transport)
                        ),
                    };
                }
            }
            ClientState::Authenticating => {
                while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
                    let text = String::from_utf8_lossy(&message);
                    println!("Server: {}", text);

                    if text == "Welcome! You are connected." {
                        println!("Authenticated successfully!");
                        println!("Entering main game loop...");
                        state = ClientState::InGame;
                    } else if text == "Incorrect passcode. Disconnecting." {
                        state = ClientState::Disconnected {
                            message: "Incorrect passcode.".to_string(),
                        };
                    }
                }

                if client.is_disconnected() {
                    state = ClientState::Disconnected {
                        message: format!(
                            "Disconnected while authenticating: {}",
                            get_disconnect_reason(&client, &transport)
                        ),
                    };
                }
            }
            ClientState::InGame => {
                if message_count % 120 == 0 {
                    let message = format!(
                        "Hello from client {}! (message {})",
                        client_id,
                        message_count / 120
                    );
                    client
                        .send_message(DefaultChannel::ReliableOrdered, message.as_bytes().to_vec());
                    println!("Sent: {}", message);
                }
                message_count += 1;

                while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
                    let text = String::from_utf8_lossy(&message);
                    println!("Server: {}", text);
                }

                if client.is_disconnected() {
                    state = ClientState::Disconnected {
                        message: format!(
                            "Disconnected from game: {}",
                            get_disconnect_reason(&client, &transport)
                        ),
                    };
                }
            }
            ClientState::Disconnected { ref message } => {
                println!("{}", message);
                break 'main_loop;
            }
        }

        // Handle timeout for connecting/authenticating states.
        if matches!(state, ClientState::Connecting | ClientState::Authenticating) {
            if Instant::now() > auth_timeout {
                transport.disconnect();
                state = ClientState::Disconnected {
                    message: "Connection timed out.".to_string(),
                };
            }
        }

        // Send any queued packets.
        if let Err(e) = transport.send_packets(&mut client) {
            state = ClientState::Disconnected {
                message: format!("Error sending packets: {}", e),
            };
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    println!("Client shutting down");
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

fn prompt(msg: &str) -> Result<String, io::Error> {
    print!("{}", msg);
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
    Ok(s.trim().to_string())
}
