use std::io::{self, Write};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64;
use base64::Engine;
use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};

// TODO: User-friendly error handling.
fn main() {
    // Get client ID from command line or generate one
    let args: Vec<String> = std::env::args().collect();
    let client_id: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        println!("No client ID provided, using generated ID: {}", id);
        id
    });

    // Prompt user for server details
    fn prompt(msg: &str) -> String {
        print!("{}", msg);
        io::stdout().flush().unwrap();
        let mut s = String::new();
        io::stdin().read_line(&mut s).unwrap();
        s.trim().to_string()
    }

    fn parse_private_key(input: &str) -> Result<[u8; 32], String> {
        let s = input.trim();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(|e| format!("Base64 decode error: {}", e))?;
        if bytes.len() != 32 {
            return Err(format!(
                "Decoded passcode must be 32 bytes, got {}",
                bytes.len()
            ));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(key)
    }

    // Server address
    let server_addr_input = prompt("Server address (e.g. 127.0.0.1:5000): ");
    let server_addr: SocketAddr = server_addr_input
        .parse()
        .expect("Failed to parse server address");

    // Game version.
    let protocol_id: u64 = 0;

    // Private key
    let private_key_input = prompt("Passcode: ");
    let private_key = parse_private_key(&private_key_input).expect("Invalid private key");

    // Generate connect token on the client side
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let connect_token = ConnectToken::generate(
        current_time,
        protocol_id,
        3600, // Valid for 1 hour
        client_id,
        15, // 15 second connection timeout
        vec![server_addr],
        None, // No user data
        &private_key,
    )
    .expect("Failed to generate token");

    // Client setup - bind to any available port
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

    // Main game loop
    let mut last_updated = Instant::now();
    let mut message_count = 0;

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        // Receive new messages and update client
        client.update(duration);

        if let Err(e) = transport.update(duration, &mut client) {
            eprintln!("Transport error: {}", e);
            break;
        }

        if client.is_connected() {
            // Send a test message every 2 seconds (120 frames at 60fps)
            if message_count % 120 == 0 {
                let message = format!(
                    "Hello from client {}! (message {})",
                    client_id,
                    message_count / 120
                );
                client.send_message(DefaultChannel::ReliableOrdered, message.as_bytes().to_vec());
                println!("Sent: {}", message);
            }
            message_count += 1;

            // Receive messages from server
            while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
                let text = String::from_utf8_lossy(&message);
                println!("Server: {}", text);
            }
        } else if client.is_connecting() {
            if message_count % 60 == 0 {
                println!("Still connecting...");
            }
            message_count += 1;
        } else if client.is_disconnected() {
            match client.disconnect_reason() {
                Some(reason) => {
                    println!("Disconnected: {:?}", reason);
                }
                None => {
                    println!("Disconnected (no reason given).");
                }
            }
            break;
        } else {
            println!("Client in unknown state, neither connected nor disconnected.");
            break;
        }

        // Send packets to server using the transport layer
        transport.send_packets(&mut client).unwrap();

        // Sleep to avoid busy-waiting (~60 FPS)
        std::thread::sleep(Duration::from_millis(16));
    }

    println!("Client shutting down");
}
