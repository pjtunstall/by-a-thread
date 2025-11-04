use std::io::{self, Write};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};

enum ClientState {
    Connecting,
    Authenticating,
    InGame,
    Disconnected { message: String },
}

fn main() {
    let private_key: [u8; 32] = [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ];

    let client_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    fn prompt(msg: &str) -> String {
        print!("{}", msg);
        io::stdout().flush().unwrap();
        let mut s = String::new();
        io::stdin().read_line(&mut s).unwrap();
        s.trim().to_string()
    }

    fn parse_passcode(input: &str) -> Result<[u8; 6], String> {
        let s = input.trim();

        if s.len() != 6 {
            return Err(format!("Passcode must be 6 digits, got {}", s.len()));
        }

        let mut bytes = [0u8; 6];
        for (i, ch) in s.chars().enumerate() {
            if let Some(d) = ch.to_digit(10) {
                bytes[i] = d as u8;
            } else {
                return Err(format!("Invalid character in passcode: '{}'", ch));
            }
        }

        Ok(bytes)
    }

    let server_addr_input = prompt("Server address (e.g. 127.0.0.1:5000): ");
    let server_addr: SocketAddr = server_addr_input
        .parse()
        .expect("Failed to parse server address");

    let version = env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .unwrap()
        .parse()
        .unwrap();

    let passcode_as_string = prompt("Passcode: ");
    let passcode = match parse_passcode(&passcode_as_string) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let connect_token = ConnectToken::generate(
        current_time,
        version,
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
                    println!(
                        "Transport connected. Sending passcode: {}",
                        passcode_as_string
                    );
                    client.send_message(DefaultChannel::ReliableOrdered, passcode.to_vec());
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
        .unwrap_or_else(|| "No reason given.".to_string())
}
