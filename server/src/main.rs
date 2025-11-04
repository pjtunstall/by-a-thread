use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use renet::{Bytes, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use shared::auth::Passcode;

fn main() {
    let private_key: [u8; 32] = [
        211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123,
        88, 129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
    ];

    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);
    let socket = UdpSocket::bind(server_addr).expect("Failed to bind socket");

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Your system clock appears to be incorrect--it's set to a date before 1970! Please open your system's date and time settings and enable automatic time synchronization (NTP). On most Linux systems, try `timedatectl set-ntp true`. On non-systemd distros (like Alpine or Gentoo), use `rc-service ntpd start` or `rc-service chronyd start` instead.");

    let protocol_id = env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .expect("Failed to get major version")
        .parse()
        .expect("Failed to parse major version");

    let server_config = ServerConfig {
        current_time,
        max_clients: 10,
        protocol_id,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Secure { private_key },
    };

    let mut transport =
        NetcodeServerTransport::new(server_config, socket).expect("Failed to create transport");

    let connection_config = ConnectionConfig::default();
    let mut server = RenetServer::new(connection_config);

    let Passcode { bytes, string } = Passcode::generate(6);
    let passcode = Bytes::copy_from_slice(&bytes);

    println!("  Game version: {}", protocol_id);
    println!("  Server address: {}", server_addr);
    println!("  Passcode: {}", string);

    let mut auth_attempts: HashMap<u64, u8> = HashMap::new();
    let mut last_updated = Instant::now();
    const MAX_AUTH_ATTEMPTS: u8 = 3;

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        transport
            .update(duration, &mut server)
            .expect("Failed to update transport");
        server.update(duration);

        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    println!("Client {} connected", client_id);
                    auth_attempts.insert(client_id, 0);
                }
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    println!("Client {} disconnected: {}", client_id, reason);
                    auth_attempts.remove(&client_id);
                }
            }
        }

        for client_id in server.clients_id() {
            while let Some(message) =
                server.receive_message(client_id, DefaultChannel::ReliableOrdered)
            {
                if let Some(attempts) = auth_attempts.get_mut(&client_id) {
                    if message == passcode {
                        println!("Client {} authenticated successfully.", client_id);
                        auth_attempts.remove(&client_id);

                        let welcome_msg = "Welcome! You are connected.".as_bytes().to_vec();
                        server.send_message(
                            client_id,
                            DefaultChannel::ReliableOrdered,
                            welcome_msg,
                        );
                    } else {
                        *attempts += 1;
                        println!(
                            "Client {} sent wrong passcode (Attempt {}).",
                            client_id, *attempts
                        );

                        if *attempts >= MAX_AUTH_ATTEMPTS {
                            println!("Client {} failed authentication. Disconnecting.", client_id);
                            let error_msg =
                                "Incorrect passcode. Disconnecting.".as_bytes().to_vec();
                            server.send_message(
                                client_id,
                                DefaultChannel::ReliableOrdered,
                                error_msg,
                            );
                            server.disconnect(client_id);
                        } else {
                            let try_again_msg =
                                "Incorrect passcode. Try again.".as_bytes().to_vec();
                            server.send_message(
                                client_id,
                                DefaultChannel::ReliableOrdered,
                                try_again_msg,
                            );
                        }
                    }
                } else {
                    let text = String::from_utf8_lossy(&message);
                    println!("Client {}: {}", client_id, text);

                    let response = format!("Server received: {}", text);
                    server.send_message(
                        client_id,
                        DefaultChannel::ReliableOrdered,
                        response.as_bytes().to_vec(),
                    );
                }
            }
        }

        transport.send_packets(&mut server);
        std::thread::sleep(Duration::from_millis(16));
    }
}
