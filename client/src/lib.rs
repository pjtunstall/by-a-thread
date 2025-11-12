mod net;
mod state;
mod state_handlers;
mod ui;

use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::net::RenetNetworkHandle;
use crate::state::{ClientSession, ClientState};
use crate::state_handlers::{AppChannel, NetworkHandle};
use crate::ui::{ClientUi, TerminalUi};
use shared::{self, ServerMessage};

pub fn run_client() {
    let private_key = shared::auth::private_key();
    let client_id = rand::random::<u64>();
    let server_addr = net::default_server_addr();
    let protocol_id = shared::protocol_version();
    let current_time = shared::current_time();
    let connect_token = net::create_connect_token(
        current_time,
        protocol_id,
        client_id,
        server_addr,
        &private_key,
    );
    let mut ui = TerminalUi::new().expect("failed to initialize terminal UI");
    let socket = match UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => socket,
        Err(e) => {
            ui.show_message(&format!("Failed to bind client socket: {}.", e));
            return;
        }
    };
    let authentication = ClientAuthentication::Secure { connect_token };
    let mut transport = match NetcodeClientTransport::new(current_time, authentication, socket) {
        Ok(transport) => transport,
        Err(e) => {
            ui.show_message(&format!("Failed to create network transport: {}", e));
            return;
        }
    };

    let connection_config = shared::connection_config();
    let mut client = RenetClient::new(connection_config);

    ui.print_client_banner(protocol_id, server_addr, client_id);

    let mut session = ClientSession::new();

    client_loop(&mut session, &mut ui, &mut client, &mut transport);

    ui.show_message("Client shutting down.");
}

fn client_loop(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    client: &mut RenetClient,
    transport: &mut NetcodeClientTransport,
) {
    let mut last_updated = Instant::now();

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        if let Err(e) = transport.update(duration, client) {
            if apply_transition(
                session,
                ui,
                ClientState::Disconnected {
                    message: format!("Transport error: {}", e),
                },
            ) {
                break;
            }
            continue;
        }

        client.update(duration);

        let mut network_handle = RenetNetworkHandle::new(client, transport);

        update_estimated_server_time(session, &mut network_handle);

        let next_state_from_logic = match session.state() {
            ClientState::Startup { .. } => state_handlers::startup(session, ui),
            ClientState::Connecting => state_handlers::connecting(session, ui, &mut network_handle),
            ClientState::Authenticating { .. } => {
                state_handlers::authenticating(session, ui, &mut network_handle)
            }
            ClientState::ChoosingUsername { .. } => {
                state_handlers::choosing_username(session, ui, &mut network_handle)
            }
            ClientState::InChat => state_handlers::in_chat(session, ui, &mut network_handle),
            ClientState::Countdown => state_handlers::countdown(session, ui),
            ClientState::Disconnected { .. } => None,
            ClientState::InGame => break,
        };

        if let Some(new_state) = next_state_from_logic {
            if apply_transition(session, ui, new_state) {
                break;
            }
            continue;
        }

        if let Err(e) = transport.send_packets(client) {
            if apply_transition(
                session,
                ui,
                ClientState::Disconnected {
                    message: format!("Error sending packets: {}", e),
                },
            ) {
                break;
            }
        }

        thread::sleep(Duration::from_millis(16));
    }
}

fn update_estimated_server_time(session: &mut ClientSession, network: &mut RenetNetworkHandle) {
    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        match bincode::serde::decode_from_slice(&message, bincode::config::standard()) {
            Ok((ServerMessage::ServerTime(server_sent_time), _)) => {
                let rtt = network.rtt();
                let one_way_latency = (rtt / 1000.0) / 2.0;
                session.estimated_server_time = server_sent_time + one_way_latency;
            }
            _ => {}
        }
    }
}

fn apply_transition(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    new_state: ClientState,
) -> bool {
    session.transition(new_state);
    if let ClientState::Disconnected { message } = session.state() {
        ui.show_message(message);
        true
    } else {
        false
    }
}
