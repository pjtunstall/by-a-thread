use std::{
    collections::HashMap,
    io::stdout,
    net::{SocketAddr, UdpSocket},
    thread,
    time::{Duration, Instant},
};

use bincode::{config::standard, serde::encode_to_vec};
use crossterm::{
    cursor::{Hide, MoveToColumn, Show},
    execute,
    terminal::{Clear, ClearType},
};
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    net::{self, NetworkHandle, RenetNetworkHandle},
    state::{self, ClientSession, ClientState},
    state_handlers,
    ui::ClientUi,
};
use shared::{
    self,
    auth::MAX_ATTEMPTS,
    net::AppChannel,
    player::Player,
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
    let target_duration = Duration::from_micros(16667);

    loop {
        let now = Instant::now();
        let duration = now - last_updated;
        last_updated = now;

        if let Err(e) = transport.update(duration, client) {
            apply_client_transition(
                session,
                ui,
                None,
                ClientState::Disconnected {
                    message: format!("Transport error: {}.", e),
                },
            );
        }
        if matches!(session.state(), ClientState::Disconnected { .. }) {
            break;
        }

        client.update(duration);
        session.estimated_server_time += duration.as_secs_f64();

        {
            let mut network_handle = RenetNetworkHandle::new(client, transport);
            update_estimated_server_time(session, &mut network_handle);
            update_client_state(session, ui, &mut network_handle);
        }

        if matches!(session.state(), ClientState::Disconnected { .. }) {
            break;
        }

        if let Err(e) = transport.send_packets(client) {
            apply_client_transition(
                session,
                ui,
                None,
                ClientState::Disconnected {
                    message: format!("Error sending packets: {}.", e),
                },
            );
        }
        if matches!(session.state(), ClientState::Disconnected { .. }) {
            break;
        }

        let elapsed = Instant::now() - last_updated;
        if elapsed < target_duration {
            thread::sleep(target_duration - elapsed);
        }
    }
}

fn update_client_state(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network_handle: &mut RenetNetworkHandle,
) {
    let next_state_from_logic = match session.state() {
        ClientState::Startup { .. } => state_handlers::startup::handle(session, ui),
        ClientState::Connecting => state_handlers::connecting::handle(session, ui, network_handle),
        ClientState::Authenticating { .. } => {
            state_handlers::auth::handle(session, ui, network_handle)
        }
        ClientState::ChoosingUsername { .. } => {
            state_handlers::username::handle(session, ui, network_handle)
        }
        ClientState::InChat => state_handlers::chat::handle(session, ui, network_handle),
        ClientState::ChoosingDifficulty { .. } => {
            state_handlers::difficulty::handle(session, ui, network_handle)
        }
        ClientState::Countdown => state_handlers::countdown::handle(session, ui, network_handle),
        ClientState::Disconnected { .. } => None,
        ClientState::InGame { .. } => state_handlers::game::handle(session, ui, network_handle),
    };

    if let Some(new_state) = next_state_from_logic {
        apply_client_transition(session, ui, Some(network_handle), new_state);
    }
}

fn update_estimated_server_time(session: &mut ClientSession, network: &mut RenetNetworkHandle) {
    while let Some(message) = network.receive_message(AppChannel::ServerTime) {
        match bincode::serde::decode_from_slice(&message, bincode::config::standard()) {
            Ok((ServerMessage::ServerTime(server_sent_time), _)) => {
                let rtt = network.rtt();
                let one_way_latency = (rtt / 1000.0) / 2.0;
                let target_time = server_sent_time + one_way_latency;
                let delta = target_time - session.estimated_server_time;

                if delta.abs() > 1.0 {
                    // Snap instantly if we are way off, e.g. on startup.
                    session.estimated_server_time = target_time;
                } else {
                    // Otherwise move 10% of the way toward the target per update.
                    let alpha = 0.1;
                    session.estimated_server_time += delta * alpha;
                }
            }
            _ => {}
        }
    }
}

fn apply_client_transition(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: Option<&mut dyn NetworkHandle>,
    new_state: ClientState,
) {
    session.transition(new_state);

    match session.state_mut() {
        ClientState::Startup { prompt_printed } => {
            if !*prompt_printed {
                ui.show_prompt(&state_handlers::passcode_prompt(MAX_ATTEMPTS));
                *prompt_printed = true;
            }
        }
        ClientState::Authenticating { .. } => {
            let network = network.expect("network handle required for Authenticating transition");
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
            execute!(stdout(), MoveToColumn(0), Clear(ClearType::CurrentLine))
                .expect("failed to clear line");

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
                print_player_list(ui, session, players);
            }
            execute!(stdout(), Hide).expect("failed to hide cursor");
        }
        ClientState::Disconnected { message } => {
            execute!(
                stdout(),
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Show
            )
            .expect("failed to show cursor and clear line");
            ui.show_message(message);
        }
        _ => {}
    }
}

pub fn print_player_list(
    ui: &mut dyn ClientUi,
    session: &ClientSession,
    players: &HashMap<u64, Player>,
) {
    ui.show_message("\nPlayers:");
    for player in players.values() {
        let is_self = if player.id == session.client_id {
            "<--you"
        } else {
            ""
        };
        ui.show_sanitized_message(&format!(
            " - {} ({}) {}",
            player.name,
            player.color.as_str(),
            is_self
        ));
    }
    ui.show_sanitized_message("");
}
