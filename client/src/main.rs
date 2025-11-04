use std::io::{Write, stdin, stdout};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand;
use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};

use shared::auth::Passcode;

enum ClientState {
    Startup {
        prompt_printed: bool,
    },
    Connecting,
    Authenticating {
        waiting_for_input: bool,
        guesses_left: u8,
    },
    InGame,
    Disconnected {
        message: String,
    },
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

fn main() {
    const MAX_ATTEMPTS: u8 = 3;

    let (tx, rx) = mpsc::channel::<String>();

    thread::spawn(move || {
        loop {
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
        }
    });

    let private_key: [u8; 32] = [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ];

    let client_id = rand::random::<u64>();

    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);

    let protocol_id = env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("Failed to get major version")
        .parse()
        .expect("Failed to parse major version");

    let mut first_passcode: Option<Passcode> = None;

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

    let mut state = ClientState::Startup {
        prompt_printed: false,
    };
    let mut last_updated = Instant::now();
    let mut message_count = 0;

    'main_loop: loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        if let Err(e) = transport.update(duration, &mut client) {
            state = ClientState::Disconnected {
                message: format!("Transport error: {}", e),
            };
        }
        client.update(duration);

        let mut next_state = None;

        match state {
            ClientState::Startup {
                ref mut prompt_printed,
            } => {
                if !*prompt_printed {
                    print!("Passcode ({} guesses): ", MAX_ATTEMPTS);
                    stdout().flush().unwrap();
                    *prompt_printed = true;
                }

                match rx.try_recv() {
                    Ok(input_string) => {
                        if let Some(passcode) = parse_passcode_input(&input_string) {
                            first_passcode = Some(passcode);
                            next_state = Some(ClientState::Connecting);
                        } else {
                            eprintln!("Invalid format. Please enter a 6-digit number.");
                            print!("Passcode ({} guesses): ", MAX_ATTEMPTS);
                            stdout().flush().unwrap();
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        next_state = Some(ClientState::Disconnected {
                            message: "Input thread disconnected.".to_string(),
                        });
                    }
                }
            }
            ClientState::Connecting => {
                if client.is_connected() {
                    if let Some(passcode) = &first_passcode {
                        println!("Transport connected. Sending passcode: {}", passcode.string);
                        client
                            .send_message(DefaultChannel::ReliableOrdered, passcode.bytes.clone());
                        next_state = Some(ClientState::Authenticating {
                            waiting_for_input: false,
                            guesses_left: 3,
                        });
                    } else {
                        next_state = Some(ClientState::Disconnected {
                            message: "Internal error: No passcode to send.".to_string(),
                        });
                    }
                } else if client.is_disconnected() {
                    next_state = Some(ClientState::Disconnected {
                        message: format!(
                            "Connection failed: {}",
                            get_disconnect_reason(&client, &transport)
                        ),
                    });
                }
            }
            ClientState::Authenticating {
                ref mut waiting_for_input,
                ref mut guesses_left,
            } => {
                while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
                    let text = String::from_utf8_lossy(&message);
                    println!("Server: {}", text);

                    if text == "Welcome! You are connected." {
                        println!("Authenticated successfully!");
                        println!("Entering main game loop...");
                        next_state = Some(ClientState::InGame);
                        break;
                    } else if text == "Incorrect passcode. Try again." {
                        *guesses_left = guesses_left.saturating_sub(1);
                        print!(
                            "Please enter new 6-digit passcode. ({} guesses remaining): ",
                            *guesses_left
                        );
                        stdout().flush().expect("Failed to flush stdout");
                        *waiting_for_input = true;
                    } else if text == "Incorrect passcode. Disconnecting." {
                        next_state = Some(ClientState::Disconnected {
                            message: "Incorrect passcode (final attempt failed).".to_string(),
                        });
                        break;
                    }
                }

                if *waiting_for_input {
                    match rx.try_recv() {
                        Ok(input_string) => {
                            if let Some(passcode) = parse_passcode_input(&input_string) {
                                println!("Sending new guess...");
                                client
                                    .send_message(DefaultChannel::ReliableOrdered, passcode.bytes);
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
                            next_state = Some(ClientState::Disconnected {
                                message: "Input thread disconnected.".to_string(),
                            });
                        }
                    }
                }

                if client.is_disconnected() {
                    next_state = Some(ClientState::Disconnected {
                        message: format!(
                            "Disconnected while authenticating: {}",
                            get_disconnect_reason(&client, &transport)
                        ),
                    });
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
                    next_state = Some(ClientState::Disconnected {
                        message: format!(
                            "Disconnected from game: {}",
                            get_disconnect_reason(&client, &transport)
                        ),
                    });
                }
            }
            ClientState::Disconnected { ref message } => {
                println!("{}", message);
                break 'main_loop;
            }
        }

        if let Some(new_state) = next_state {
            state = new_state;
            if matches!(state, ClientState::Disconnected { .. }) {
                continue 'main_loop;
            }
        }

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
