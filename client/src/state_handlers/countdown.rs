use bincode::{config::standard, serde::decode_from_slice};

use crate::state::{ClientSession, ClientState};
use crate::{
    net::NetworkHandle,
    ui::{ClientUi, UiInputError},
};
use shared::{net::AppChannel, protocol::ServerMessage};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Countdown) {
        panic!(
            "called countdown::handle() when state was not Countdown; current state: {:?}",
            session.state()
        );
    }

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_status_line(&format!("[Deserialization error: {}.]", e)),
        }
    }

    if let Some(end_time) = session.countdown_end_time {
        let time_remaining_secs = end_time - session.estimated_server_time;

        if time_remaining_secs > 0.0 {
            let status_message = format!("Game starting in {}s...", time_remaining_secs.floor());
            ui.show_status_line(&status_message);
        } else {
            if let Some(maze) = session.maze.take() {
                if let Some(players) = session.players.take() {
                    return Some(ClientState::InGame { maze, players });
                } else {
                    return Some(ClientState::TransitioningToDisconnected {
                        message: "Failed to receive players data.".to_string(),
                    });
                }
            } else {
                return Some(ClientState::TransitioningToDisconnected {
                    message: "Failed to receive maze data".to_string(),
                });
            }
        }
    } else {
        ui.show_status_line("Waiting for server...");
    }

    if let Err(UiInputError::Disconnected) = ui.poll_single_key() {
        return Some(ClientState::TransitioningToDisconnected {
            message: "input thread disconnected.".to_string(),
        });
    }

    if network.is_disconnected() {
        return Some(ClientState::TransitioningToDisconnected {
            message: format!(
                "Disconnected during countdown: {}.",
                network.get_disconnect_reason()
            ),
        });
    }

    None
}
