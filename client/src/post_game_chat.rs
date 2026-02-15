use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use glam::Vec3;
use macroquad::prelude::*;

use crate::{
    assets::Assets,
    info::map::{self, post_game::PostGameMap},
    lobby::ui::{LobbyUi, UiErrorKind, UiInputError},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, InputMode},
};
use common::{
    chat::MAX_CHAT_MESSAGE_BYTES,
    constants::TICK_SECS,
    net::AppChannel,
    player::{self, Color},
    protocol::{ClientMessage, ServerMessage},
    snapshot::Snapshot,
};

#[derive(Debug)]
pub struct PostGameChat {
    pub awaiting_initial_roster: bool,
    pub waiting_for_server: bool,
    pub leaderboard_received: bool,
    pub map_for_post_game: Option<PostGameMap>,
}

pub fn update(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    let state = std::mem::take(&mut session.state);

    let result = match state {
        ClientState::PostGameChat(mut chat_state) => {
            let result = handle(&mut chat_state, session, ui, network);
            session.state = ClientState::PostGameChat(chat_state);
            result
        }
        other_state => {
            session.state = other_state;
            None
        }
    };

    if result.is_some() {
        return result;
    }

    let ui_state = session.prepare_ui_state();
    if ui_state.show_waiting_message {
        ui.show_warning("Waiting for server...");
    }

    let should_show_input = matches!(ui_state.mode, InputMode::Enabled);
    let show_cursor = should_show_input;
    let font = assets.map(|assets| &assets.font);
    ui.draw(
        should_show_input,
        show_cursor,
        font,
        None::<crate::lobby::ui::LobbyTimerInfo>,
    );

    if let (Some(assets), ClientState::PostGameChat(chat)) = (assets, &session.state) {
        if let Some(data) = &chat.map_for_post_game {
            if !chat.leaderboard_received && !data.positions.is_empty() {
                map::post_game::draw_post_game_map(data, assets);
            }
        }
    }

    set_default_camera();

    None
}

fn apply_snapshot_to_positions(positions: &mut [(Vec3, Color)], snapshot: &Snapshot) {
    for (i, pos_color) in positions.iter_mut().enumerate() {
        if let Some(remote) = snapshot.remote.get(i) {
            pos_color.0 = vec3(remote.position.x, player::HEIGHT, remote.position.y);
        }
    }
}

fn handle(
    chat_state: &mut PostGameChat,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let PostGameChat {
        awaiting_initial_roster,
        waiting_for_server,
        leaderboard_received,
        map_for_post_game,
    } = chat_state;

    let input_enabled = !*waiting_for_server;
    if input_enabled {
        match ui.poll_input(MAX_CHAT_MESSAGE_BYTES, session.is_host) {
            Ok(Some(input)) => session.add_input(input),
            Err(UiInputError::Disconnected) => {
                ui.show_sanitized_error(&format!("No connection: {}.", UiInputError::Disconnected));
                return Some(ClientState::Disconnected {
                    message: UiInputError::Disconnected.to_string(),
                });
            }
            Ok(None) => {}
        }
    }

    while let Some(data) = network.receive_message(AppChannel::Unreliable) {
        if let Some(map_data) = map_for_post_game {
            if let Ok((ServerMessage::Snapshot(wire), _)) =
                decode_from_slice::<ServerMessage, _>(&data, standard())
            {
                apply_snapshot_to_positions(&mut map_data.positions, &wire.data);
            }
        }
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        *waiting_for_server = false;

        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((
                ServerMessage::ChatMessage {
                    username,
                    color,
                    content,
                },
                _,
            )) => {
                if *awaiting_initial_roster {
                    continue;
                }
                ui.show_sanitized_message_with_color(&format!("{}: {}", username, content), color);
            }
            Ok((ServerMessage::UserJoined { username }, _)) => {
                if *awaiting_initial_roster {
                    continue;
                }
                ui.show_sanitized_message(&format!("Server: {} joined the chat.", username));
            }
            Ok((ServerMessage::UserLeft { username }, _)) => {
                if *awaiting_initial_roster {
                    continue;
                }
                ui.show_sanitized_message(&format!("Server: {} left the chat.", username));
            }
            Ok((ServerMessage::PostGameRoster { hades_shades }, _)) => {
                if hades_shades.is_empty() {
                    ui.show_sanitized_message("Server: You are the only shade in Hades.");
                } else {
                    ui.show_sanitized_message("Server: Shades in Hades:");
                    for entry in hades_shades {
                        ui.show_sanitized_message_with_color(
                            &format!(" - {}", entry.username),
                            entry.color,
                        );
                    }
                }
                *awaiting_initial_roster = false;
            }
            Ok((ServerMessage::PostGameLeaderboard { entries }, _)) => {
                *leaderboard_received = true;
                *waiting_for_server = true;
                *map_for_post_game = None;
                ui.show_message(" ");
                ui.show_sanitized_message("Leaderboard:");
                let mut current_rank = 1;
                let mut prev_ticks: Option<u64> = None;
                for entry in entries.iter() {
                    if prev_ticks.is_some() && prev_ticks != Some(entry.ticks_survived) {
                        current_rank += 1;
                    }
                    prev_ticks = Some(entry.ticks_survived);

                    let seconds = (entry.ticks_survived as f64 * TICK_SECS) as u64;
                    let minutes = seconds / 60;
                    let remainder = seconds % 60;
                    ui.show_sanitized_message_with_color(
                        &format!(
                            "  {}. {}  {:02}:{:02}  ({})",
                            current_rank, entry.username, minutes, remainder, entry.exit_reason
                        ),
                        entry.color,
                    );
                }
                ui.show_message(" ");
                ui.show_sanitized_message("Server: That's your lot.");
                ui.show_message(" ");
                ui.show_warning("Press Escape to exit.");
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
            }
            Ok((ServerMessage::SessionEnded { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
                ui.show_message(" ");
                ui.show_warning("Press Escape to exit.");
                return Some(ClientState::EndAfterLeaderboard);
            }
            Ok((_, _)) => {}
            Err(error) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[Deserialization error: {}]", error),
            ),
        }
    }

    while let Some(input) = session.take_input() {
        let trimmed_input = input.trim();

        if trimmed_input.is_empty() {
            continue;
        }

        let message = ClientMessage::SendChat(trimmed_input.to_string());

        let payload = encode_to_vec(&message, standard()).expect("failed to serialize chat");
        network.send_message(AppChannel::ReliableOrdered, payload);

        *waiting_for_server = true;
    }

    if network.is_disconnected() {
        if *leaderboard_received {
            return Some(ClientState::EndAfterLeaderboard);
        }
        return Some(ClientState::Disconnected {
            message: format!(
                "Disconnected from chat: {}.",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}
