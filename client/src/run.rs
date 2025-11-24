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
    session::{self, ClientSession},
    state::{ClientState, InputMode},
    state_handlers,
    ui::{ClientUi, MacroquadUi, UiInputError},
};
use shared::{self, auth::MAX_ATTEMPTS, player::Player};

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
        handle_user_escape(&mut runner);

        client_frame_update(&mut runner);

        let ui_state = runner.session.prepare_ui_state();
        let is_difficulty_choice = matches!(
            runner.session.state(),
            ClientState::ChoosingDifficulty { .. }
        );

        if ui_state.show_waiting_message {
            runner.ui.show_warning("Waiting for server...");
        }

        if matches!(runner.session.input_mode(), InputMode::Enabled) {
            if is_difficulty_choice {
                match runner.ui.poll_single_key() {
                    Ok(Some(shared::input::UiKey::Char(c))) if matches!(c, '1' | '2' | '3') => {
                        runner.session.add_input(c.to_string());
                    }
                    Err(UiInputError::Disconnected) => {
                        apply_client_transition(
                            &mut runner.session,
                            &mut runner.ui,
                            None,
                            TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
                                message: "input source disconnected (Ctrl+C or window closed)"
                                    .to_string(),
                            }),
                        );
                        break;
                    }
                    _ => {}
                }
            } else {
                let ui_ref: &mut dyn ClientUi = &mut runner.ui;
                match ui_ref
                    .poll_input(shared::chat::MAX_CHAT_MESSAGE_BYTES, runner.session.is_host)
                {
                    Ok(Some(input)) => {
                        runner.session.add_input(input);
                    }
                    Err(UiInputError::Disconnected) => {
                        apply_client_transition(
                            &mut runner.session,
                            &mut runner.ui,
                            None,
                            TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
                                message: "input source disconnected (Ctrl+C or window closed)"
                                    .to_string(),
                            }),
                        );
                        break;
                    }
                    Ok(None) => {}
                }
            }
        }

        if !runner.session.is_countdown_active() {
            let should_show_input = matches!(ui_state.mode, InputMode::Enabled);
            let show_cursor = should_show_input && !is_difficulty_choice;
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

    apply_client_transition(
        &mut runner.session,
        &mut runner.ui,
        None,
        TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
            message: "client closed by user".to_string(),
        }),
    );
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
            TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
                message: format!("transport error: {}", e),
            }),
        );
        return;
    }

    if runner.session.state().is_disconnected() {
        return;
    }

    // Input processing during update (for prediction or immediate network handling).
    if matches!(runner.session.input_mode(), InputMode::Enabled) {
        let is_difficulty_choice = matches!(
            runner.session.state(),
            ClientState::ChoosingDifficulty { .. }
        );

        if is_difficulty_choice {
            match runner.ui.poll_single_key() {
                Ok(Some(shared::input::UiKey::Char(c))) if matches!(c, '1' | '2' | '3') => {
                    runner.session.add_input(c.to_string());
                }
                Err(UiInputError::Disconnected) => {
                    apply_client_transition(
                        &mut runner.session,
                        &mut runner.ui,
                        None,
                        TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
                            message: "input source disconnected (Ctrl+C or window closed)"
                                .to_string(),
                        }),
                    );
                    return;
                }
                _ => {}
            }
        }

        let ui_ref: &mut dyn ClientUi = &mut runner.ui;
        match ui_ref.poll_input(shared::chat::MAX_CHAT_MESSAGE_BYTES, false) {
            Ok(Some(input)) => {
                runner.session.add_input(input);
            }
            Err(UiInputError::Disconnected) => {
                apply_client_transition(
                    &mut runner.session,
                    &mut runner.ui,
                    None,
                    TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
                        message: "input source disconnected (Ctrl+C or window closed)".to_string(),
                    }),
                );
                return;
            }
            Ok(None) => {}
        }
    }

    runner.client.update(duration);
    runner.session.estimated_server_time += duration.as_secs_f64();

    {
        let mut network_handle = RenetNetworkHandle::new(&mut runner.client, &mut runner.transport);
        crate::time::update_estimated_server_time(&mut runner.session, &mut network_handle);
        update_client_state(&mut runner.session, &mut runner.ui, &mut network_handle);
    }

    if runner.session.state().is_disconnected() {
        return;
    }

    if let Err(e) = runner.transport.send_packets(&mut runner.client) {
        apply_client_transition(
            &mut runner.session,
            &mut runner.ui,
            None,
            TransitionAction::ChangeTo(ClientState::TransitioningToDisconnected {
                message: format!("{}", e),
            }),
        );
    }
}

fn update_client_state(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network_handle: &mut RenetNetworkHandle,
) {
    if session.is_countdown_finished() {
        apply_client_transition(
            session,
            ui,
            Some(network_handle),
            TransitionAction::StartGame,
        );
        return;
    }

    let next_state_from_logic = match session.state() {
        ClientState::Startup { .. } => state_handlers::startup::handle(session, ui),
        ClientState::Connecting { .. } => {
            state_handlers::connecting::handle(session, ui, network_handle)
        }
        ClientState::Authenticating { .. } => {
            state_handlers::auth::handle(session, ui, network_handle)
        }
        ClientState::ChoosingUsername { .. } => {
            state_handlers::username::handle(session, ui, network_handle)
        }
        ClientState::AwaitingUsernameConfirmation => {
            state_handlers::waiting::handle(session, ui, network_handle)
        }
        ClientState::InChat { .. } => state_handlers::chat::handle(session, ui, network_handle),
        ClientState::ChoosingDifficulty { .. } => {
            state_handlers::difficulty::handle(session, ui, network_handle)
        }
        ClientState::Countdown { .. } => {
            state_handlers::countdown::handle(session, ui, network_handle)
        }
        ClientState::TransitioningToDisconnected { .. } => None,
        ClientState::Disconnected { .. } => None,
        ClientState::InGame { .. } => state_handlers::game::handle(session, ui, network_handle),
    };

    if let Some(new_state) = next_state_from_logic {
        apply_client_transition(
            session,
            ui,
            Some(network_handle),
            TransitionAction::ChangeTo(new_state),
        );
    }
}

fn apply_client_transition(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    _network: Option<&mut dyn NetworkHandle>,
    action: TransitionAction,
) {
    match action {
        TransitionAction::ChangeTo(new_state) => {
            if let ClientState::TransitioningToDisconnected { message } = &new_state {
                let rest = if message.is_empty() {
                    ".".to_string()
                } else {
                    format!(": {}.", message.trim_end_matches('.'))
                };
                ui.show_sanitized_error(&format!("No connection{}", rest));
                session.transition(ClientState::Disconnected {
                    message: message.clone(),
                });
                return;
            }

            session.transition(new_state);
        }
        TransitionAction::StartGame => {
            if session.transition_to_game().is_err() {
                ui.show_sanitized_error("Error: Tried to start game from invalid state");
            }
        }
    }

    // Handle UI side-effects (post-transition).
    match session.state_mut() {
        ClientState::Startup { prompt_printed } => {
            if !*prompt_printed {
                ui.show_prompt(&state_handlers::auth::passcode_prompt(MAX_ATTEMPTS));
                *prompt_printed = true;
            }
        }
        ClientState::Authenticating {
            waiting_for_input,
            waiting_for_server,
            ..
        } => {
            if !*waiting_for_server {
                *waiting_for_input = true;
            }
        }
        ClientState::ChoosingUsername { prompt_printed } => {
            if !*prompt_printed {
                ui.show_prompt(&session::username_prompt());
                *prompt_printed = true;
            }
        }
        ClientState::InChat { .. } => {
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
        ClientState::InGame { .. } => {
            // The Countdown logic is handled by the state swap.
            // We just need to signal the visual start.
            ui.show_message("GO!");
        }
        ClientState::Disconnected { message } => ui.show_sanitized_error(message),
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
