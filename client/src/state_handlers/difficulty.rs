use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::state::{ClientSession, ClientState};
use crate::{net::NetworkHandle, ui::ClientUi};
use shared::{
    net::AppChannel,
    protocol::{ClientMessage, ServerMessage},
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let is_correct_state = matches!(session.state(), ClientState::ChoosingDifficulty { .. });
    if !is_correct_state {
        panic!(
            "called difficulty::handle() when state was not ChoosingDifficulty; current state: {:?}",
            session.state()
        );
    };

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((
                ServerMessage::CountdownStarted {
                    end_time,
                    maze,
                    players,
                },
                _,
            )) => {
                session.countdown_end_time = Some(end_time);
                session.maze = Some(maze);
                session.players = Some(players);
                return Some(ClientState::Countdown);
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
                if let ClientState::ChoosingDifficulty { prompt_printed } = session.state_mut() {
                    *prompt_printed = false;
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    // CRITICAL FIX: Use session.take_input() instead of ui.poll_single_key()
    if let Some(input) = session.take_input() {
        let trimmed = input.trim();
        let level = match trimmed {
            "1" => Some(1),
            "2" => Some(2),
            "3" => Some(3),
            _ => {
                ui.show_sanitized_error("Invalid choice. Please press 1, 2, or 3.");
                None
            }
        };

        if let Some(level) = level {
            let msg = ClientMessage::SetDifficulty(level);
            let payload =
                encode_to_vec(&msg, standard()).expect("failed to serialize SetDifficulty");
            network.send_message(AppChannel::ReliableOrdered, payload);
        }
    }

    if network.is_disconnected() {
        return Some(ClientState::TransitioningToDisconnected {
            message: format!(
                "Disconnected while choosing difficulty: {}.",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}
