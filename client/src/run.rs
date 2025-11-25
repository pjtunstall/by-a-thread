use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::{color, prelude::*, window::clear_background};
use renet::RenetClient;
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};

use crate::{
    lobby::{
        handlers,
        ui::{LobbyUi, MacroquadLobbyUi, UiInputError},
    },
    net::{self, RenetNetworkHandle},
    resources::Resources,
    session::ClientSession,
    state::{ClientState, InputMode, Lobby},
};
use shared::{self, player::Player};

// This enum is used to control how to transiton between states.
// For most transitions, the plain ChangeTo is sufficient.
// StartGame is a special transition with logic to move the
// maze and player data rather than cloning it.
pub enum TransitionAction {
    // Change to a simple state (Disconnected, Lobby, etc).
    ChangeTo(ClientState),
    // Signal to perform the zero-copy swap from Countdown to InGame.
    StartGame,
}

pub struct ClientRunner {
    session: ClientSession,
    client: RenetClient,
    transport: NetcodeClientTransport,
    ui: MacroquadLobbyUi,
    last_updated: Instant,
    resources: Resources,
}

impl ClientRunner {
    pub async fn new(
        socket: UdpSocket,
        server_addr: SocketAddr,
        private_key: [u8; 32],
        ui: MacroquadLobbyUi,
    ) -> Result<Self, String> {
        let resources = Resources::load().await;
        let client_id = ::rand::random::<u64>();
        let protocol_id = shared::protocol::version();
        let current_time_duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before unix epoch");
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
            resources,
        })
    }

    pub fn pump_network(&mut self) -> Result<(), String> {
        let now = Instant::now();
        let duration = now - self.last_updated;
        self.last_updated = now;

        if let Err(e) = self.transport.update(duration, &mut self.client) {
            return Err(format!("Transport Update Failed: {}", e));
        }

        self.client.update(duration);

        {
            let mut network_handle = RenetNetworkHandle::new(&mut self.client, &mut self.transport);
            crate::time::update_clock(&mut self.session, &mut network_handle, duration);
        }

        if let Err(e) = self.transport.send_packets(&mut self.client) {
            return Err(format!("Packet Send Failed: {}", e));
        }

        Ok(())
    }
}

pub async fn run_client_loop(
    socket: UdpSocket,
    server_addr: SocketAddr,
    private_key: [u8; 32],
    ui: MacroquadLobbyUi,
) {
    let mut runner = match ClientRunner::new(socket, server_addr, private_key, ui).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    let ui_ref: &mut dyn LobbyUi = &mut runner.ui;
    ui_ref.print_client_banner(
        shared::protocol::version(),
        server_addr,
        runner.session.client_id,
    );

    loop {
        handle_user_escape(&mut runner);

        if let Err(e) = runner.pump_network() {
            runner
                .ui
                .show_sanitized_error(&format!("No connection: {}.", e));
            apply_client_transition(
                &mut runner.session,
                &mut runner.ui,
                TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
                    message: format!("transport error: {}", e),
                }),
            );
            return;
        }

        if let ClientState::InGame(game_state) = runner.session.state() {
            let yaw: f32 = 0.0;
            let pitch: f32 = 0.1;

            let mut position = Default::default();
            for (id, player) in &game_state.players {
                if *id == runner.session.client_id {
                    position = vec3(player.position.x, 24.0, player.position.z)
                }
            }

            set_camera(&Camera3D {
                position,
                target: position
                    + vec3(
                        yaw.sin() * pitch.cos(),
                        pitch.sin(),
                        yaw.cos() * pitch.cos(),
                    ),
                up: vec3(0.0, 1.0, 0.0),
                ..Default::default()
            });

            clear_background(color::BLACK);
            game_state.draw(&runner.resources.wall_texture);
            next_frame().await;
            continue;
        }

        client_frame_update(&mut runner);

        let ui_state = runner.session.prepare_ui_state();
        if ui_state.show_waiting_message {
            runner.ui.show_warning("Waiting for server...");
        }

        if !runner.session.is_countdown_active() {
            let should_show_input = matches!(ui_state.mode, InputMode::Enabled);
            let show_cursor = should_show_input;
            runner.ui.draw(should_show_input, show_cursor);
        }

        if runner.session.state().is_disconnected() {
            handle_disconnected_ui_loop(&mut runner).await;
            break;
        }

        next_frame().await;
    }
}

async fn handle_disconnected_ui_loop(runner: &mut ClientRunner) {
    loop {
        runner.ui.draw(false, false);
        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        next_frame().await;
    }
}

fn handle_user_escape(runner: &mut ClientRunner) {
    if !is_key_pressed(KeyCode::Escape) {
        return;
    }

    if !runner
        .session
        .state()
        .not_already_disconnecting_or_disconnected()
    {
        return;
    }

    runner
        .ui
        .show_sanitized_error("No connection: client closed by user.");
    apply_client_transition(
        &mut runner.session,
        &mut runner.ui,
        TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
            message: "client closed by user".to_string(),
        }),
    );
}

fn client_frame_update(runner: &mut ClientRunner) {
    if runner.session.state().is_disconnected() {
        return;
    }

    // Lobby input.
    if matches!(runner.session.input_mode(), InputMode::Enabled) {
        let ui_ref: &mut dyn LobbyUi = &mut runner.ui;
        match ui_ref.poll_input(shared::chat::MAX_CHAT_MESSAGE_BYTES, runner.session.is_host) {
            Ok(Some(input)) => {
                runner.session.add_input(input);
            }
            Err(e @ UiInputError::Disconnected) => {
                runner
                    .ui
                    .show_sanitized_error(&format!("No connection: {}.", e));
                apply_client_transition(
                    &mut runner.session,
                    &mut runner.ui,
                    TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
                        message: e.to_string(),
                    }),
                );
                return;
            }
            Ok(None) => {}
        }
    }

    let mut network_handle = RenetNetworkHandle::new(&mut runner.client, &mut runner.transport);
    update_client_state(&mut runner.session, &mut runner.ui, &mut network_handle);
}

fn update_client_state(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network_handle: &mut RenetNetworkHandle,
) {
    if session.is_countdown_finished() {
        apply_client_transition(session, ui, TransitionAction::StartGame);
        return;
    }

    let next_state_from_logic = match session.state() {
        ClientState::Lobby(Lobby::Startup { .. }) => handlers::startup::handle(session, ui),
        ClientState::Lobby(Lobby::Connecting { .. }) => {
            handlers::connecting::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::Authenticating { .. }) => {
            handlers::auth::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::ChoosingUsername { .. }) => {
            handlers::username::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::AwaitingUsernameConfirmation) => {
            handlers::waiting::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::InChat { .. }) => {
            handlers::chat::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::ChoosingDifficulty { .. }) => {
            handlers::difficulty::handle(session, ui, network_handle)
        }
        ClientState::Lobby(Lobby::Countdown { .. }) => {
            handlers::countdown::handle(session, ui, network_handle)
        }
        ClientState::TransitioningToDisconnected { .. } => None,
        ClientState::Disconnected { .. } => None,
        ClientState::InGame(_) => handlers::game::handle(session, ui, network_handle),
        ClientState::Debrief => None,
    };

    if let Some(new_state) = next_state_from_logic {
        apply_client_transition(session, ui, TransitionAction::ChangeTo(new_state));
    }
}

fn apply_client_transition(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    action: TransitionAction,
) {
    match action {
        TransitionAction::StartGame => {
            if session.transition_to_game().is_err() {
                ui.show_sanitized_error("Tried to start game from invalid state.");
                ui.show_message("GO!");
                return;
            }
        }
        TransitionAction::ChangeTo(new_state) => {
            let target_state = match new_state {
                ClientState::TransitioningToDisconnected { message } => {
                    ClientState::Disconnected { message }
                }
                other => other,
            };

            session.transition(target_state);
        }
    }
}

pub fn print_player_list(
    ui: &mut dyn LobbyUi,
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
