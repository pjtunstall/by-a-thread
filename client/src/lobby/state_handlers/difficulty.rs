use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use super::start_countdown::handle_countdown_started;
use crate::{
    assets::Assets,
    lobby::ui::{LobbyUi, UiErrorKind, UiInputError},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{
    constants::NUM_DIFFICULTY_LEVELS,
    input::UiKey,
    net::AppChannel,
    player::Color,
    protocol::{ClientMessage, ServerMessage},
};

fn pale_for_difficulty(color: Color) -> Color {
    match color {
        Color::GREEN => Color::PaleGreen,
        Color::CHARTREUSE => Color::PaleChartreuse,
        Color::YELLOW => Color::PaleYellow,
        Color::ORANGE => Color::PaleOrange,
        Color::RED => Color::PaleRed,
        _ => color,
    }
}

const MENU_ITEMS: &[(&str, Color)] = &[
    ("Four-Quadrants Binary Tree (trivial)", Color::GREEN),
    ("Standard Recursive Division (easy)", Color::GREEN),
    ("Meander (fair)", Color::CHARTREUSE),
    (
        "Territorial Recursive Division (fair to middling)",
        Color::CHARTREUSE,
    ),
    ("Hecate's Key (middling)", Color::CHARTREUSE),
    ("Prim (middling hard)", Color::YELLOW),
    ("Kruskal (hard)", Color::YELLOW),
    ("Drunkard's Walk (Herculean)", Color::ORANGE),
    ("Backtracker (Sisyphean)", Color::ORANGE),
    ("Wilson (next level)", Color::RED),
];

const _: () = assert!(
    MENU_ITEMS.len() == NUM_DIFFICULTY_LEVELS as usize,
    "MENU_ITEMS must have NUM_DIFFICULTY_LEVELS entries"
);

fn format_menu_lines(selected_index: u8) -> Vec<(String, Color)> {
    let mut lines = Vec::with_capacity(NUM_DIFFICULTY_LEVELS as usize + 2);
    for (i, (label, color)) in MENU_ITEMS.iter().enumerate() {
        let prefix = if i == selected_index as usize {
            "  * "
        } else {
            "    "
        };
        let line_color = if i == selected_index as usize {
            pale_for_difficulty(*color)
        } else {
            *color
        };
        lines.push((format!("{}{}", prefix, label), line_color));
    }
    lines.push((" ".to_string(), Color::CHARTREUSE));
    lines.push((
        "Use up/down arrows to select, Enter to confirm.".to_string(),
        Color::LightGray,
    ));
    lines
}

fn handle_difficulty_input(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    choice_sent: bool,
    selected_index: &mut u8,
) -> Option<ClientState> {
    if choice_sent {
        return None;
    }

    match ui.poll_single_key() {
        Ok(key_result) => match key_result {
            Some(UiKey::Up) => {
                *selected_index = (selected_index.saturating_add(NUM_DIFFICULTY_LEVELS - 1))
                    % NUM_DIFFICULTY_LEVELS;
            }
            Some(UiKey::Down) => {
                *selected_index = (*selected_index + 1) % NUM_DIFFICULTY_LEVELS;
            }
            Some(UiKey::Enter) => {
                session.add_input(selected_index.to_string());
            }
            _ => {}
        },
        Err(UiInputError::Disconnected) => {
            ui.show_sanitized_error("No connection: disconnected.");
            return Some(ClientState::Disconnected {
                message: "disconnected".to_string(),
            });
        }
    }

    None
}

pub fn handle(
    lobby_state: &mut Lobby,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    let Lobby::ChoosingDifficulty {
        prompt_printed,
        choice_sent,
        selected_index,
    } = lobby_state
    else {
        unreachable!();
    };

    if let Some(next_state) = handle_difficulty_input(session, ui, *choice_sent, selected_index) {
        return Some(next_state);
    }

    if !*prompt_printed && !*choice_sent {
        ui.show_message("Server: What manner of maze will it be?");
        ui.show_message(" ");
        let menu_lines = format_menu_lines(*selected_index);
        for (line, color) in &menu_lines {
            ui.show_message_with_color(line, *color);
        }
        *prompt_printed = true;
    }

    if *prompt_printed && !*choice_sent {
        let menu_lines = format_menu_lines(*selected_index);
        ui.replace_last_messages(NUM_DIFFICULTY_LEVELS as usize + 2, menu_lines);
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((
                ServerMessage::CountdownStarted {
                    end_time,
                    game_data,
                },
                _,
            )) => {
                return Some(handle_countdown_started(end_time, game_data, assets));
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
                return Some(ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed: false,
                    choice_sent: false,
                    selected_index: 0,
                }));
            }
            Ok((ServerMessage::LobbyTimer { end_time }, _)) => {
                session.lobby_timer_end = Some(end_time);
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("[DESERIALIZATION ERROR: {}]", e),
            ),
        }
    }

    let choice_already_sent = *choice_sent;

    if !choice_already_sent {
        if let Some(input) = session.take_input() {
            let trimmed = input.trim();
            let level = trimmed
                .parse::<u8>()
                .ok()
                .filter(|&l| l < NUM_DIFFICULTY_LEVELS);

            if let Some(level) = level {
                let msg = ClientMessage::SetDifficulty(level);
                let payload =
                    encode_to_vec(&msg, standard()).expect("failed to serialize SetDifficulty");
                network.send_message(AppChannel::ReliableOrdered, payload);

                return Some(ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed: *prompt_printed,
                    choice_sent: true,
                    selected_index: *selected_index,
                }));
            }
        } else {
            session.take_input();
        }
    } else {
        session.take_input();
    }

    if network.is_disconnected() {
        ui.show_typed_error(
            UiErrorKind::NetworkDisconnect,
            &format!(
                "disconnected while choosing difficulty: {}",
                network.get_disconnect_reason()
            ),
        );
        return Some(ClientState::Disconnected {
            message: format!(
                "disconnected while choosing difficulty: {}",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{server_address, test_helpers::MockNetwork, test_helpers::MockUi};
    use common::protocol::{ClientMessage, ServerMessage};

    #[test]
    fn guards_does_not_panic_in_correct_state() {
        let mut session = ClientSession::new(0, server_address::default_server_address().ok());
        session.transition(ClientState::Lobby(Lobby::ChoosingDifficulty {
            prompt_printed: false,
            choice_sent: false,
            selected_index: 0,
        }));
        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        assert!(
            {
                let mut temp_state = std::mem::take(&mut session.state);
                let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                    handle(lobby_state, &mut session, &mut ui, &mut network, None)
                } else {
                    panic!("expected Lobby state");
                };
                session.state = temp_state;
                result
            }
            .is_none(),
            "should not panic and should return None"
        );
    }

    #[test]
    fn re_enables_input_after_server_info() {
        let mut session = ClientSession::new(0, server_address::default_server_address().ok());
        session.transition(ClientState::Lobby(Lobby::ChoosingDifficulty {
            prompt_printed: true,
            choice_sent: true,
            selected_index: 0,
        }));

        let mut ui = MockUi::default();
        let mut network = MockNetwork::new();
        network.queue_server_message(ServerMessage::ServerInfo {
            message: "Invalid choice.".to_string(),
        });

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network, None)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert!(
            matches!(
                _next_state,
                Some(ClientState::Lobby(Lobby::ChoosingDifficulty {
                    prompt_printed: false,
                    choice_sent: false,
                    selected_index: 0
                }))
            ),
            "state should reset prompt_printed and choice_sent to false"
        );

        assert_eq!(
            ui.messages.len(),
            1,
            "server info should be surfaced to the user"
        );
    }

    #[test]
    fn polls_single_key_for_choice() {
        let mut session = ClientSession::new(0, server_address::default_server_address().ok());
        session.transition(ClientState::Lobby(Lobby::ChoosingDifficulty {
            prompt_printed: true,
            choice_sent: false,
            selected_index: 2,
        }));

        let mut ui = MockUi::default();
        ui.keys.push_back(Ok(Some(UiKey::Enter)));
        let mut network = MockNetwork::new();

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network, None)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert!(
            matches!(
                _next_state,
                Some(ClientState::Lobby(Lobby::ChoosingDifficulty {
                    choice_sent: true,
                    ..
                }))
            ),
            "choice should be marked as sent after pressing Enter"
        );

        let (channel, payload) = network
            .sent_messages
            .pop_front()
            .expect("expected difficulty choice to be sent");
        assert_eq!(channel, AppChannel::ReliableOrdered);
        let (msg, _) =
            decode_from_slice::<ClientMessage, _>(&payload, standard()).expect("decode message");
        assert_eq!(msg, ClientMessage::SetDifficulty(2));
    }

    #[test]
    fn returns_disconnect_on_input_source_drop() {
        let mut session = ClientSession::new(0, server_address::default_server_address().ok());
        session.transition(ClientState::Lobby(Lobby::ChoosingDifficulty {
            prompt_printed: true,
            choice_sent: false,
            selected_index: 0,
        }));

        let mut ui = MockUi::default();
        ui.keys.push_back(Err(UiInputError::Disconnected));
        let mut network = MockNetwork::new();

        let _next_state = {
            let mut temp_state = std::mem::take(&mut session.state);
            let result = if let ClientState::Lobby(lobby_state) = &mut temp_state {
                handle(lobby_state, &mut session, &mut ui, &mut network, None)
            } else {
                panic!("expected Lobby state");
            };
            session.state = temp_state;
            result
        };

        assert!(
            matches!(_next_state, Some(ClientState::Disconnected { .. })),
            "expected transition to disconnected, got {:?}",
            _next_state
        );
    }
}
