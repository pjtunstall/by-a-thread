use std::fmt;

use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};
use glam::Vec3;
use macroquad::prelude::*;

use crate::{
    assets::Assets,
    info::{self, map::MapOverlay},
    lobby::ui::{LobbyUi, UiErrorKind, UiInputError},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, InputMode},
};
use common::{
    chat::MAX_CHAT_MESSAGE_BYTES,
    constants::TICK_SECS,
    maze::Maze,
    net::AppChannel,
    player::{Color, Color::YELLOW},
    protocol::{ClientMessage, ServerMessage},
    snapshot::Snapshot,
};

pub struct AfterGameMap {
    pub map_overlay: MapOverlay,
    pub maze: Maze,
    pub positions: Vec<(Vec3, Color)>,
}

impl fmt::Debug for AfterGameMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AfterGameMap")
            .field("maze", &self.maze)
            .field("positions", &self.positions)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct AfterGameChat {
    pub awaiting_initial_roster: bool,
    pub waiting_for_server: bool,
    pub leaderboard_received: bool,
    pub map_for_after_game: Option<AfterGameMap>,
}

pub fn update(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    let state = std::mem::take(&mut session.state);

    let result = match state {
        ClientState::AfterGameChat(mut chat_state) => {
            let result = handle(&mut chat_state, session, ui, network);
            session.state = ClientState::AfterGameChat(chat_state);
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
    ui.draw(should_show_input, show_cursor, font);

    if let (Some(assets), ClientState::AfterGameChat(chat)) = (assets, &session.state) {
        if let Some(data) = &chat.map_for_after_game {
            if !chat.leaderboard_received && !data.positions.is_empty() {
                draw_after_game_map(data, assets);
            }
        }
    }

    set_default_camera();

    None
}

fn apply_snapshot_to_positions(positions: &mut [(Vec3, Color)], snapshot: &Snapshot) {
    for (i, pos_color) in positions.iter_mut().enumerate() {
        if let Some(remote) = snapshot.remote.get(i) {
            pos_color.0 = remote.position;
        }
    }
}

fn handle(
    chat_state: &mut AfterGameChat,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let AfterGameChat {
        awaiting_initial_roster,
        waiting_for_server,
        leaderboard_received,
        map_for_after_game,
    } = chat_state;

    let input_enabled = !*leaderboard_received && !*waiting_for_server;
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
        if let Some(map_data) = map_for_after_game {
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
            Ok((ServerMessage::AfterGameRoster { hades_shades }, _)) => {
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
            Ok((ServerMessage::AfterGameLeaderboard { entries }, _)) => {
                *leaderboard_received = true;
                *map_for_after_game = None;
                ui.show_message(" ");
                ui.show_sanitized_message("Leaderboard:");
                let mut current_rank = 1;
                let mut prev_ticks: Option<u64> = None;
                for entry in entries.iter() {
                    if prev_ticks.is_some() && prev_ticks != Some(entry.ticks_survived) {
                        current_rank += 1;
                    }
                    prev_ticks = Some(entry.ticks_survived);

                    let seconds = entry.ticks_survived as f64 * TICK_SECS;
                    ui.show_sanitized_message_with_color(
                        // TODO: Format as minutes and seconds.
                        &format!(
                            "  {}. {}  {:.1} s  ({})",
                            current_rank, entry.username, seconds, entry.exit_reason
                        ),
                        entry.color,
                    );
                }
                ui.show_message(" ");
                ui.show_message_with_color("That's your lot. Press escape to exit.", YELLOW);
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
            }
            Ok((_, _)) => {}
            Err(error) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[Deserialization error: {}]", error),
            ),
        }
    }

    if !*leaderboard_received {
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
    }

    if network.is_disconnected() {
        if !*leaderboard_received {
            ui.show_typed_error(
                UiErrorKind::NetworkDisconnect,
                &format!(
                    "Disconnected from chat: {}.",
                    network.get_disconnect_reason()
                ),
            );
            Some(ClientState::Disconnected {
                message: format!(
                    "Disconnected from chat: {}.",
                    network.get_disconnect_reason()
                ),
            })
        } else {
            Some(ClientState::EndAfterLeaderboard)
        }
    } else {
        None
    }
}

const BORDER_THICKNESS: f32 = 16.0;
const BORDER_ALPHA: f32 = 0.5;

fn draw_after_game_map(data: &AfterGameMap, assets: &Assets) {
    push_camera_state();
    set_default_camera();

    let font_size = (info::FONT_SIZE * info::INFO_SCALE).round().max(1.0) as u16;
    let map_scale = font_size as f32 / info::FONT_SIZE;
    let map_w = data.map_overlay.rect.w * map_scale;
    let map_h = data.map_overlay.rect.h * map_scale;
    let margin = info::BASE_INDENTATION;
    let border_w = map_w + 2.0 * BORDER_THICKNESS;
    let border_h = map_h + 2.0 * BORDER_THICKNESS;
    let border_x = screen_width() - margin - border_w;
    let border_y = margin;
    draw_rectangle(
        border_x,
        border_y,
        border_w,
        border_h,
        macroquad::prelude::Color::new(0.0, 0.0, 0.0, BORDER_ALPHA),
    );

    let map_x = screen_width() - margin - BORDER_THICKNESS - map_w;
    let map_y = margin + BORDER_THICKNESS;
    info::draw_map_at(
        map_x,
        map_y,
        &data.map_overlay,
        &data.maze,
        &data.positions,
        assets,
    );

    pop_camera_state();
}
