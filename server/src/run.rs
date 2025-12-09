use std::{
    collections::HashSet,
    io::stdout,
    net::{SocketAddr, UdpSocket},
    thread,
    time::{Duration, Instant},
};

use bincode::{config::standard, serde::encode_to_vec};
use crossterm::{
    cursor::{Hide, MoveToColumn},
    execute,
    terminal::{Clear, ClearType},
};
use renet::RenetServer;
use renet_netcode::NetcodeServerTransport;

use crate::{
    net::{self, RenetServerNetworkHandle, ServerNetworkEvent, ServerNetworkHandle},
    state::{Lobby, ServerState},
    state_handlers,
};
use common::{
    self,
    auth::Passcode,
    net::AppChannel,
    protocol::ServerMessage,
    time::{self, TICK_MICROS},
};

pub fn run_server(socket: UdpSocket, server_addr: SocketAddr, private_key: [u8; 32]) {
    let current_time = common::time::now();
    let protocol_id = common::protocol::version();

    let server_config =
        net::build_server_config(current_time, protocol_id, server_addr, private_key);
    let mut transport =
        NetcodeServerTransport::new(server_config, socket).expect("failed to create transport");
    let connection_config = common::net::connection_config();
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

    let target_tick_duration = Duration::from_micros(TICK_MICROS);

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        transport
            .update(duration, server)
            .expect("failed to update transport");
        server.update(duration);

        let mut network_handle = RenetServerNetworkHandle { server };

        if now.duration_since(last_sync_time) > sync_interval {
            sync_clocks(&mut network_handle);
            last_sync_time = now;
        }

        update_server_state(&mut network_handle, state, passcode);

        transport.send_packets(server);

        let elapsed = Instant::now() - last_updated;
        if elapsed < target_tick_duration {
            thread::sleep(target_tick_duration - elapsed);
        }
    }
}

pub fn update_server_state(
    network: &mut dyn ServerNetworkHandle,
    state: &mut ServerState,
    passcode: &Passcode,
) {
    process_events(network, state);

    let next_state = match state {
        ServerState::Lobby(lobby_state) => {
            state_handlers::lobby::handle(network, lobby_state, passcode)
        }
        ServerState::ChoosingDifficulty(difficulty_state) => {
            state_handlers::difficulty::handle(network, difficulty_state)
        }
        ServerState::Countdown(countdown_state) => {
            state_handlers::countdown::handle(network, countdown_state)
        }
        ServerState::Game(game_state) => state_handlers::game::handle(network, game_state),
    };

    if let Some(new_state) = next_state {
        apply_server_transition(state, new_state, network);
    }
}

fn apply_server_transition(
    old_state: &mut ServerState,
    new_state: ServerState,
    network: &mut dyn ServerNetworkHandle,
) {
    println!(
        "Server state changing from {} to {}.",
        old_state.name(),
        new_state.name()
    );

    *old_state = new_state;

    match old_state {
        ServerState::Lobby(_) => {}

        ServerState::ChoosingDifficulty(difficulty_state) => {
            let host_id = difficulty_state
                .host_id()
                .expect("host should be set when entering difficulty selection");
            let host_name = difficulty_state
                .username(host_id)
                .expect("host should have a username");
            println!("Host ({}) is choosing a difficulty.", host_name);
            let message = ServerMessage::BeginDifficultySelection;
            let payload = encode_to_vec(&message, standard())
                .expect("failed to serialize BeginDifficultySelection");
            network.send_message(host_id, AppChannel::ReliableOrdered, payload);

            let pending: HashSet<u64> = difficulty_state
                .lobby
                .pending_clients()
                .into_iter()
                .collect();

            for client_id in pending {
                let message = ServerMessage::ServerInfo {
                    message: "Game already started: disconnecting.".to_string(),
                };
                let payload =
                    encode_to_vec(&message, standard()).expect("failed to serialize ServerInfo");
                network.send_message(client_id, AppChannel::ReliableOrdered, payload);
            }

            let info = ServerMessage::ServerInfo {
                message: format!(
                    "Waiting for host {} to choose a difficulty level.",
                    host_name
                ),
            };
            let info_payload =
                encode_to_vec(&info, standard()).expect("failed to serialize ServerInfo");

            for client_id in network.clients_id() {
                let has_username = difficulty_state.lobby.username(client_id).is_some();
                if client_id == host_id || !has_username {
                    continue;
                }
                network.send_message(client_id, AppChannel::ReliableOrdered, info_payload.clone());
            }
        }

        ServerState::Countdown(countdown_state) => {
            println!("Server entering Countdown state.");

            let end_time = countdown_state
                .end_time
                .duration_since(Instant::now())
                .as_secs_f64()
                + time::now().as_secs_f64();

            let message = ServerMessage::CountdownStarted {
                end_time,
                snapshot: std::mem::take(&mut countdown_state.snapshot),
            };
            let payload = encode_to_vec(&message, standard())
                .expect("failed to serialize CountDownStarted mesage");
            network.broadcast_message(AppChannel::ReliableOrdered, payload);

            execute!(stdout(), Hide).expect("failed to hide cursor");
        }

        ServerState::Game(_) => {}
    }
}

pub fn process_events(network: &mut dyn ServerNetworkHandle, state: &mut ServerState) {
    while let Some(event) = network.get_event() {
        match event {
            ServerNetworkEvent::ClientConnected { client_id } => {
                println!("Client {} connected.", client_id);
                state.register_connection(client_id, network);
            }
            ServerNetworkEvent::ClientDisconnected { client_id, reason } => {
                if matches!(state, ServerState::Countdown(_)) {
                    // Clear the "Game starting in..." line.
                    execute!(stdout(), MoveToColumn(0), Clear(ClearType::CurrentLine)).ok();
                }

                println!("Client {} disconnected: {}.", client_id, reason);
                state.remove_client(client_id, network);
            }
        }
    }
}

fn sync_clocks(network: &mut dyn ServerNetworkHandle) {
    let server_time_f64 = common::time::now().as_secs_f64();
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
    use common::protocol::ServerMessage;

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
