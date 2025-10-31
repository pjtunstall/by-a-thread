use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::{self, Engine};
use rand::Rng;
use renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};

fn main() {
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);
    let socket = UdpSocket::bind(server_addr).expect("Failed to bind socket");

    let passcode = fill_32_random_bytes();

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Your system clock appears to be incorrect--it's set to a date before 1970! Please open your system's date and time settings and enable automatic time synchronization (NTP). On most Linux systems, try `timedatectl set-ntp true`. On non-systemd distros (like Alpine or Gentoo), use `rc-service ntpd start` or `rc-service chronyd start` instead.");

    // Derive protocol ID from game version. Update this when making breaking changes.
    // Since Renet doesn't provide a DisconnectReason to let the client know that they got the version wrong,
    // keep the protocol ID at 0 until I've impremented a suitable error message.
    let version = env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .unwrap()
        .parse()
        .unwrap();

    // Configure the server transport.
    let server_config = ServerConfig {
        current_time,
        max_clients: 10,
        protocol_id: version,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Secure {
            private_key: passcode,
        },
    };

    let mut transport =
        NetcodeServerTransport::new(server_config, socket).expect("Failed to create transport");

    // Configure the renet server with channels, etc.
    let connection_config = ConnectionConfig::default();
    let mut server = RenetServer::new(connection_config);

    println!("  Game version: {}", version);
    println!("  Server address: {}", server_addr);
    println!(
        "  Passcode: {}",
        base64::engine::general_purpose::STANDARD.encode(passcode)
    );

    // Main game loop.
    let mut last_updated = Instant::now();

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        // Receive messages from clients, i.e. process raw packets from the transport layer (socket).
        // This handles decryption/authentication and feeds valid data into the server's queues.
        server.update(duration); // Updates internal renet state, handles message queues, timeouts, etc.
        transport.update(duration, &mut server).unwrap(); // Reads incoming UDP packets, decrypts them, validates connections, and passes messages to the renet server.

        // Handle server events: connections and disconnections.
        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    println!("Client {} connected", client_id);
                }
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    println!("Client {} disconnected: {}", client_id, reason);
                }
            }
        }

        // Consume application-level messages from the server's internal queues.
        // This dequeues reassembled, ordered messages from a specific channel.
        for client_id in server.clients_id() {
            while let Some(message) =
                server.receive_message(client_id, DefaultChannel::ReliableOrdered)
            {
                let text = String::from_utf8_lossy(&message);
                println!("Client {}: {}", client_id, text);

                // Echo back
                let response = format!("Server received: {}", text);
                server.send_message(
                    client_id,
                    DefaultChannel::ReliableOrdered,
                    response.as_bytes().to_vec(),
                );

                // Other options:
                // server.broadcast_message(channel, bytes): Send to all clients
                // server.broadcast_message_except(client_id, channel, bytes): Send to all except one
            }
        }

        // Send packets to clients using the transport layer.
        transport.send_packets(&mut server);

        // Sleep to avoid busy-waiting (~60 FPS).
        std::thread::sleep(Duration::from_millis(16));
    }
}

fn fill_32_random_bytes() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes[..]);
    bytes
}
