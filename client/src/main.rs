use std::io::{self, Write};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport};

// TODO: User-friendly error handling.
fn main() {
    let private_key: [u8; 32] = [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ];

    let client_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    // Prompt user for server details.
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

    // Generate connect token on the client side
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let connect_token = ConnectToken::generate(
        current_time,
        version,
        3600, // Valid for 1 hour.
        client_id,
        15, // 15 second connection timeout.
        vec![server_addr],
        None, // No user data.
        &private_key,
    )
    .expect("Failed to generate token");

    // Bind to any available port.
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

    client.send_message(DefaultChannel::ReliableOrdered, passcode.to_vec());

    println!("Sent passcode: {}", passcode_as_string);
    println!("Waiting for server response...");

    let mut authenticated = false;
    let auth_timeout = Instant::now() + Duration::from_secs(10);

    // Authentication loop.
    loop {
        client.update(Duration::from_millis(16));
        transport
            .update(Duration::from_millis(16), &mut client)
            .unwrap();

        // Check for messages from the server.
        while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
            let text = String::from_utf8_lossy(&message);
            println!("Server: {}", text); // Always print server messages.

            if text == "Welcome! You are connected." {
                authenticated = true;
                println!("Authenticated successfully!");
                break;
            }

            if text == "Incorrect passcode. Disconnecting." {
                // Server sent the error message. We can exit now.
                println!("Passcode was wrong. Exiting.");
                return; // Exit main.
            }
        }

        if authenticated {
            // CRITICAL: Send any pending packets/acknowledgments before exiting auth loop!
            transport.send_packets(&mut client).unwrap();

            if client.is_connected() {
                break; // Break from auth loop. After that the main game loop will begin.
            }
            // Otherwise continue looping until authenticated or timeout.
        }

        // Check for disconnect state
        if client.is_disconnected() {
            println!("Failed to authenticate.");
            match client.disconnect_reason() {
                Some(reason) => {
                    println!("Disconnected: {:?}", reason);
                }
                None => {
                    println!("Disconnected (no reason given)");
                }
            }
            return; // Exit main.
        }

        // If there is no response from the server within the timeout, exit.
        if Instant::now() > auth_timeout {
            println!("Authentication timed out.");
            transport.disconnect(); // Tell the transport to disconnect.
            return;
        }

        // Send queued packets.
        if client.is_connected() {
            transport.send_packets(&mut client).unwrap();
        }

        // Sleep to avoid busy-waiting.
        std::thread::sleep(Duration::from_millis(16));
    }

    println!("Entering main game loop...");

    // Main game loop.
    let mut last_updated = Instant::now();
    let mut message_count = 0;

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        client.update(duration);

        if let Err(e) = transport.update(duration, &mut client) {
            eprintln!("Transport error: {}", e);
            break;
        }

        if client.is_connected() {
            // Send a test message every 2 seconds (120 frames at 60fps).
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

            // Receive messages from server.
            while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
                let text = String::from_utf8_lossy(&message);
                println!("Server: {}", text);
            }
        } else if client.is_disconnected() {
            match client.disconnect_reason() {
                Some(reason) => {
                    println!("Disconnected: {:?}", reason);
                }
                None => {
                    println!("Disconnected (no reason given)");
                }
            }
            break;
        }

        // Send packets to server using the transport layer.
        transport.send_packets(&mut client).unwrap();

        std::thread::sleep(Duration::from_millis(16));
    }

    println!("Client shutting down");
}
