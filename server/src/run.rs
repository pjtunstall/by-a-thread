use std::net::SocketAddr;
use std::thread;
use std::time::{Duration, Instant};

use bincode::{config::standard, serde::encode_to_vec};
use renet::RenetServer;
use renet_netcode::NetcodeServerTransport;

use crate::{
    net::{self, RenetServerNetworkHandle, ServerNetworkEvent, ServerNetworkHandle},
    state::{Lobby, ServerState},
    state_handlers,
};
use shared::{self, auth::Passcode, net::AppChannel, protocol::ServerMessage};

pub fn run_server() {
    let private_key = shared::auth::private_key();
    let server_addr = net::server_address();
    let socket = net::bind_socket(server_addr);

    let current_time = shared::time::now();
    let protocol_id = shared::protocol::version();

    let server_config =
        net::build_server_config(current_time, protocol_id, server_addr, private_key);
    let mut transport =
        NetcodeServerTransport::new(server_config, socket).expect("failed to create transport");
    let connection_config = shared::net::connection_config();
    let mut server = RenetServer::new(connection_config);
    let passcode = Passcode::generate(6);
    let mut state = ServerState::Lobby(Lobby::new());

    print_server_banner(protocol_id, server_addr, &passcode);
    server_loop(&mut server, &mut transport, &mut state, &passcode);
    println!("Server shutting down.");
}

fn print_server_banner(protocol_id: u64, server_addr: SocketAddr, passcode: &Passcode) {
    println!("  Game version:   {}", protocol_id);
    println!("  Server address: {}", server_addr);
    println!("  Passcode:       {}", passcode.string);
}

fn server_loop(
    server: &mut RenetServer,
    transport: &mut NetcodeServerTransport,
    state: &mut ServerState,
    passcode: &Passcode,
) {
    let mut last_updated = Instant::now();
    let mut last_sync_time = Instant::now();
    let sync_interval = Duration::from_millis(50);

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        transport
            .update(duration, server)
            .expect("failed to update transport");
        server.update(duration);

        let mut network_handle = RenetServerNetworkHandle { server };

        process_events(&mut network_handle, state);

        if now.duration_since(last_sync_time) > sync_interval {
            sync_clocks(&mut network_handle);
            last_sync_time = now;
        }

        let next_state = state_handlers::handle_messages(&mut network_handle, state, passcode);
        if let Some(new_state) = next_state {
            *state = new_state;
        }

        transport.send_packets(server);
        thread::sleep(Duration::from_millis(16));
    }
}

pub fn process_events(network: &mut dyn ServerNetworkHandle, state: &mut ServerState) {
    while let Some(event) = network.get_event() {
        match event {
            ServerNetworkEvent::ClientConnected { client_id } => {
                println!("Client {} connected.", client_id);
                state.register_connection(client_id);
            }
            ServerNetworkEvent::ClientDisconnected { client_id, reason } => {
                println!("Client {} disconnected: {}.", client_id, reason);
                state.remove_client(client_id, network);
            }
        }
    }
}

fn sync_clocks(network: &mut dyn ServerNetworkHandle) {
    let server_time_f64 = shared::time::now().as_secs_f64();
    let message = ServerMessage::ServerTime(server_time_f64);
    let payload = encode_to_vec(&message, standard()).expect("failed to serialize ServerTime");
    network.broadcast_message(AppChannel::ServerTime, payload);
}

#[cfg(test)]
mod tests {
    use bincode::config::standard;
    use bincode::serde::decode_from_slice;

    use super::*;
    use crate::state::{Lobby, ServerState};
    use crate::test_helpers::MockServerNetwork;
    use shared::protocol::ServerMessage;

    #[test]
    fn test_process_events_client_connect() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::Lobby(Lobby::new());

        network.queue_event(ServerNetworkEvent::ClientConnected { client_id: 1 });

        process_events(&mut network, &mut state);

        if let ServerState::Lobby(lobby) = state {
            assert!(lobby.is_authenticating(1));
        } else {
            panic!("state is not Lobby");
        }
    }

    #[test]
    fn test_process_events_client_disconnect_with_username() {
        let mut network = MockServerNetwork::new();
        let mut state = ServerState::Lobby(Lobby::new());

        if let ServerState::Lobby(lobby) = &mut state {
            lobby.register_connection(1);
            lobby.mark_authenticated(1);
            lobby.register_username(1, "Alice");
        }

        network.queue_event(ServerNetworkEvent::ClientDisconnected {
            client_id: 1,
            reason: "timeout".to_string(),
        });

        process_events(&mut network, &mut state);

        if let ServerState::Lobby(lobby) = state {
            assert_eq!(lobby.username(1), None);
        } else {
            panic!("state is not Lobby");
        }

        let broadcasts = network.get_broadcast_messages_data();
        assert_eq!(broadcasts.len(), 1);
        let msg = decode_from_slice::<ServerMessage, _>(&broadcasts[0], standard())
            .unwrap()
            .0;
        if let ServerMessage::UserLeft { username } = msg {
            assert_eq!(username, "Alice");
        } else {
            panic!("expected UserLeft message, got {:?}", msg);
        }
    }
}
