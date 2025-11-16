use std::{
    net::{SocketAddr, UdpSocket},
    thread,
    time::{Duration, Instant},
};

use bincode::{config::standard, serde::encode_to_vec};
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    net::{self, RenetNetworkHandle},
    state::{self, ClientSession, ClientState, MAX_ATTEMPTS},
    state_handlers::{self, AppChannel, NetworkHandle},
    ui::ClientUi,
};
use shared::{
    self,
    protocol::{ClientMessage, ServerMessage},
};

pub fn run_client(
    socket: UdpSocket,
    server_addr: SocketAddr,
    private_key: [u8; 32],
    ui: &mut dyn ClientUi,
) {
    let client_id = rand::random::<u64>();
    let protocol_id = shared::protocol::version();
    let current_time = shared::time::now();
    let connect_token = net::create_connect_token(
        current_time,
        protocol_id,
        client_id,
        server_addr,
        &private_key,
    );

    let authentication = ClientAuthentication::Secure { connect_token };
    let mut transport = match NetcodeClientTransport::new(current_time, authentication, socket) {
        Ok(transport) => transport,
        Err(e) => {
            ui.show_message(&format!("Failed to create network transport: {}.", e));
            return;
        }
    };

    let connection_config = shared::net::connection_config();
    let mut client = RenetClient::new(connection_config);
    let mut session = ClientSession::new(client_id);

    ui.print_client_banner(protocol_id, server_addr, client_id);
    client_loop(&mut session, ui, &mut client, &mut transport);
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
                None,
                ClientState::Disconnected {
                    message: format!("Transport error: {}.", e),
                },
            ) {
                break;
            }
            continue;
        }

        client.update(duration);

        let should_break = {
            let mut network_handle = RenetNetworkHandle::new(client, transport);
            update_estimated_server_time(session, &mut network_handle);
            update_client_state(session, ui, &mut network_handle)
        };

        if should_break {
            break;
        }

        if let Err(e) = transport.send_packets(client) {
            if apply_transition(
                session,
                ui,
                None,
                ClientState::Disconnected {
                    message: format!("Error sending packets: {}.", e),
                },
            ) {
                break;
            }
        }

        thread::sleep(Duration::from_millis(16));
    }
}

fn update_client_state(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network_handle: &mut RenetNetworkHandle,
) -> bool {
    let next_state_from_logic = match session.state() {
        ClientState::Startup { .. } => state_handlers::startup(session, ui),
        ClientState::Connecting => state_handlers::connecting(session, ui, network_handle),
        ClientState::Authenticating { .. } => {
            state_handlers::authenticating(session, ui, network_handle)
        }
        ClientState::ChoosingUsername { .. } => {
            state_handlers::choosing_username(session, ui, network_handle)
        }
        ClientState::InChat => state_handlers::in_chat(session, ui, network_handle),
        ClientState::ChoosingDifficulty { .. } => {
            state_handlers::choosing_difficulty(session, ui, network_handle)
        }
        ClientState::Countdown => state_handlers::countdown(session, ui, network_handle),
        ClientState::Disconnected { .. } => None,
        ClientState::InGame { .. } => state_handlers::in_game(session, ui, network_handle),
    };

    if let Some(new_state) = next_state_from_logic {
        if apply_transition(session, ui, Some(network_handle), new_state) {
            return true;
        }
    }

    false
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
    network: Option<&mut dyn NetworkHandle>,
    new_state: ClientState,
) -> bool {
    session.transition(new_state);

    match session.state_mut() {
        ClientState::Startup { prompt_printed } => {
            if !*prompt_printed {
                ui.show_prompt(&state_handlers::passcode_prompt(MAX_ATTEMPTS));
                *prompt_printed = true;
            }
        }
        ClientState::Authenticating { .. } => {
            let network = network.expect("Network handle required for Authenticating transition");
            if let Some(passcode) = session.take_first_passcode() {
                ui.show_message(&format!(
                    "Transport connected. Sending passcode: {}.",
                    passcode.string
                ));

                let message = ClientMessage::SendPasscode(passcode.bytes);
                let payload =
                    encode_to_vec(&message, standard()).expect("failed to serialize SendPasscode");
                network.send_message(AppChannel::ReliableOrdered, payload);
            } else {
                ui.show_message("Internal error: No passcode to send.");
            }
        }
        ClientState::ChoosingUsername { prompt_printed, .. } => {
            if !*prompt_printed {
                ui.show_prompt(&state::username_prompt());
                *prompt_printed = true;
            }
        }
        ClientState::InChat => {
            session.expect_initial_roster();
        }
        ClientState::ChoosingDifficulty { prompt_printed, .. } => {
            if !*prompt_printed {
                ui.show_message("Server: Choose a difficulty level:");
                ui.show_message("  1. Easy");
                ui.show_message("  2. So-so");
                ui.show_message("  3. Next level");
                ui.show_prompt("Press 1, 2, or 3: ");
                *prompt_printed = true;
            }
        }
        ClientState::Countdown => {
            if let Some(players) = session.players.as_ref() {
                state_handlers::print_player_list(ui, session, players);
            }
        }
        _ => {}
    }

    if let ClientState::Disconnected { message } = session.state() {
        ui.show_message(message);
        true
    } else {
        false
    }
}
