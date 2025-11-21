use std::{
    collections::HashMap,
    io::Write,
    net::{SocketAddr, UdpSocket},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::prelude::{KeyCode, is_key_pressed, next_frame};
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    net::{self, NetworkHandle, RenetNetworkHandle},
    state::{self, ClientSession, ClientState},
    state_handlers,
    ui::{ClientUi, MacroquadUi, UiInputError},
};
use shared::{self, auth::MAX_ATTEMPTS, net::AppChannel, player::Player, protocol::ServerMessage};

pub struct ClientRunner {
    session: ClientSession,
    client: RenetClient,
    transport: NetcodeClientTransport,
    ui: MacroquadUi,
    last_updated: Instant,
}

impl ClientRunner {
    pub fn new(
        socket: UdpSocket,
        server_addr: SocketAddr,
        private_key: [u8; 32],
        ui: MacroquadUi,
    ) -> Result<Self, String> {
        let client_id = rand::random::<u64>();
        let protocol_id = shared::protocol::version();
        let current_time_duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time is before Unix epoch");
        socket
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set socket as non-blocking: {}", e))?;
        let connect_token = net::create_connect_token(
            current_time_duration,
            protocol_id,
            client_id,
            server_addr,
            &private_key,
        );
        let authentication = ClientAuthentication::Secure { connect_token };
        let transport = NetcodeClientTransport::new(current_time_duration, authentication, socket)
            .map_err(|e| format!("Failed to create network transport: {}", e))?;
        let connection_config = shared::net::connection_config();
        let client = RenetClient::new(connection_config);
        let session = ClientSession::new(client_id);
        Ok(Self {
            session,
            client,
            transport,
            ui,
            last_updated: Instant::now(),
        })
    }
}

pub async fn run_client_loop(
    socket: UdpSocket,
    server_addr: SocketAddr,
    private_key: [u8; 32],
    ui: MacroquadUi,
) {
    let mut runner = match ClientRunner::new(socket, server_addr, private_key, ui) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    let ui_ref: &mut dyn ClientUi = &mut runner.ui;
    ui_ref.print_client_banner(
        shared::protocol::version(),
        server_addr,
        runner.session.client_id,
    );

    loop {
        if is_key_pressed(KeyCode::Escape)
            && !matches!(runner.session.state(), ClientState::Disconnected { .. })
        {
            apply_client_transition(
                &mut runner.session,
                &mut runner.ui,
                None,
                ClientState::TransitioningToDisconnected {
                    message: "Client closed by user.".to_string(),
                },
            );
        }

        client_frame_update(&mut runner);

        let is_countdown_active = matches!(runner.session.state(), ClientState::Countdown)
            && runner.session.countdown_end_time.is_some();
        if !is_countdown_active {
            let show_input = !matches!(
                runner.session.state(),
                ClientState::ChoosingDifficulty {
                    choice_sent: true,
                    ..
                } | ClientState::Countdown
                    | ClientState::Disconnected { .. }
                    | ClientState::TransitioningToDisconnected { .. }
                    | ClientState::InGame { .. }
            );
            runner.ui.draw(show_input);
        }

        let is_disconnected = matches!(runner.session.state(), ClientState::Disconnected { .. });
        if is_disconnected {
            loop {
                runner.ui.draw(false);
                if is_key_pressed(KeyCode::Escape) {
                    break;
                }
                next_frame().await;
            }
            break;
        }

        next_frame().await;
    }
}

fn client_frame_update(runner: &mut ClientRunner) {
    let now = Instant::now();
    let duration = now - runner.last_updated;
    runner.last_updated = now;
    if let Err(e) = runner.transport.update(duration, &mut runner.client) {
        eprintln!("NETWORK ERROR: Transport Update Failed: {}.", e);
        std::io::stderr().flush().ok();
        apply_client_transition(
            &mut runner.session,
            &mut runner.ui,
            None,
            ClientState::TransitioningToDisconnected {
                message: format!("Transport error: {}.", e),
            },
        );
        return;
    }
    if matches!(runner.session.state(), ClientState::Disconnected { .. }) {
        return;
    }
    let ui_ref: &mut dyn ClientUi = &mut runner.ui;
    match ui_ref.poll_input(shared::chat::MAX_CHAT_MESSAGE_BYTES) {
        Ok(Some(input)) => {
            runner.session.add_input(input);
        }
        Err(UiInputError::Disconnected) => {
            apply_client_transition(
                &mut runner.session,
                &mut runner.ui,
                None,
                ClientState::TransitioningToDisconnected {
                    message: "Input source disconnected (Ctrl+C or window closed).".to_string(),
                },
            );
            return;
        }
        Ok(None) => {}
    }
    runner.client.update(duration);
    runner.session.estimated_server_time += duration.as_secs_f64();
    {
        let mut network_handle = RenetNetworkHandle::new(&mut runner.client, &mut runner.transport);
        update_estimated_server_time(&mut runner.session, &mut network_handle);
        update_client_state(&mut runner.session, &mut runner.ui, &mut network_handle);
    }
    if matches!(runner.session.state(), ClientState::Disconnected { .. }) {
        return;
    }
    if let Err(e) = runner.transport.send_packets(&mut runner.client) {
        apply_client_transition(
            &mut runner.session,
            &mut runner.ui,
            None,
            ClientState::TransitioningToDisconnected {
                message: format!("Error sending packets: {}.", e),
            },
        );
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
        ClientState::TransitioningToDisconnected { .. } => None,
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
                    session.estimated_server_time = target_time;
                } else {
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
    _network: Option<&mut dyn NetworkHandle>,
    new_state: ClientState,
) {
    if let ClientState::TransitioningToDisconnected { message } = new_state {
        ui.show_status_line(&format!("Disconnected: {}", message));
        session.transition(ClientState::Disconnected { message });
        return;
    }
    session.transition(new_state);
    match session.state_mut() {
        ClientState::Startup { prompt_printed } => {
            if !*prompt_printed {
                ui.show_prompt(&state_handlers::auth::passcode_prompt(MAX_ATTEMPTS));
                *prompt_printed = true;
            }
        }
        ClientState::Authenticating {
            waiting_for_input, ..
        } => {
            *waiting_for_input = true;
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
        ClientState::ChoosingDifficulty {
            prompt_printed,
            choice_sent,
        } => {
            if !*prompt_printed && !*choice_sent {
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
        }
        ClientState::Disconnected { message } => {
            ui.show_status_line(message);
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
