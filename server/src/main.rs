use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use shared::auth::Passcode;

#[derive(Debug, PartialEq)]
enum AuthAttemptOutcome {
    Authenticated,
    TryAgain,
    Disconnect,
}

fn evaluate_passcode_attempt(
    passcode: &[u8],
    attempts: &mut u8,
    guess: &[u8],
    max_attempts: u8,
) -> AuthAttemptOutcome {
    if guess == passcode {
        AuthAttemptOutcome::Authenticated
    } else {
        *attempts = attempts.saturating_add(1);
        if *attempts >= max_attempts {
            AuthAttemptOutcome::Disconnect
        } else {
            AuthAttemptOutcome::TryAgain
        }
    }
}

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

    let version = env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .unwrap()
        .parse()
        .unwrap();

    let server_config = ServerConfig {
        current_time,
        max_clients: 10,
        protocol_id: version,
        public_addresses: vec![server_addr],
        authentication: ServerAuthentication::Secure { private_key },
    };

    let mut transport =
        NetcodeServerTransport::new(server_config, socket).expect("Failed to create transport");

    let connection_config = ConnectionConfig::default();
    let mut server = RenetServer::new(connection_config);

    let Passcode {
        bytes: passcode_bytes,
        string: passcode_string,
    } = Passcode::generate(6);

    println!("  Game version: {}", version);
    println!("  Server address: {}", server_addr);
    println!("  Passcode: {}", passcode_string);

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
                    match evaluate_passcode_attempt(
                        passcode_bytes.as_slice(),
                        attempts,
                        message.as_ref(),
                        MAX_AUTH_ATTEMPTS,
                    ) {
                        AuthAttemptOutcome::Authenticated => {
                            println!("Client {} authenticated successfully.", client_id);
                            auth_attempts.remove(&client_id);

                            let welcome_msg = "Welcome! You are connected.".as_bytes().to_vec();
                            server.send_message(
                                client_id,
                                DefaultChannel::ReliableOrdered,
                                welcome_msg,
                            );
                        }
                        AuthAttemptOutcome::TryAgain => {
                            println!(
                                "Client {} sent wrong passcode (Attempt {}).",
                                client_id, *attempts
                            );

                            let try_again_msg =
                                "Incorrect passcode. Try again.".as_bytes().to_vec();
                            server.send_message(
                                client_id,
                                DefaultChannel::ReliableOrdered,
                                try_again_msg,
                            );
                        }
                        AuthAttemptOutcome::Disconnect => {
                            println!("Client {} failed authentication. Disconnecting.", client_id);
                            let error_msg =
                                "Incorrect passcode. Disconnecting.".as_bytes().to_vec();
                            server.send_message(
                                client_id,
                                DefaultChannel::ReliableOrdered,
                                error_msg,
                            );
                            server.disconnect(client_id);
                            auth_attempts.remove(&client_id);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_authentication_does_not_increment_attempts() {
        let passcode = [1, 2, 3, 4, 5, 6];
        let mut attempts = 0;
        let outcome = evaluate_passcode_attempt(&passcode, &mut attempts, &passcode, 3);
        assert_eq!(outcome, AuthAttemptOutcome::Authenticated);
        assert_eq!(attempts, 0);
    }

    #[test]
    fn incorrect_attempt_requests_retry() {
        let passcode = [1, 2, 3, 4, 5, 6];
        let mut attempts = 0;
        let outcome = evaluate_passcode_attempt(&passcode, &mut attempts, &[0, 0, 0, 0, 0, 0], 3);
        assert_eq!(outcome, AuthAttemptOutcome::TryAgain);
        assert_eq!(attempts, 1);
    }

    #[test]
    fn max_attempts_triggers_disconnect() {
        let passcode = [1, 2, 3, 4, 5, 6];
        let mut attempts = 2;
        let outcome = evaluate_passcode_attempt(&passcode, &mut attempts, &[0, 0, 0, 0, 0, 0], 3);
        assert_eq!(outcome, AuthAttemptOutcome::Disconnect);
        assert_eq!(attempts, 3);
    }
}
