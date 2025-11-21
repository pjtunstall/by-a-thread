use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    net::NetworkHandle,
    state::{ClientSession, ClientState},
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
            // FIX: Adjust the time remaining before taking the ceiling to synchronize with the server's count.
            // This makes the number change when the fractional part drops to 0.0, instead of 0.000...001.
            let countdown_value = (time_remaining_secs - 0.999999).ceil() as u32;

            let countdown_text = format!("{}", countdown_value.max(1));

            ui.draw_countdown(&countdown_text);
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
        ui.show_status_line("Waiting for server to start countdown...");
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

#[cfg(test)]
mod tests {
    use shared::maze::Algorithm;

    use super::*;
    use crate::test_helpers::{MockNetwork, MockUi};
    use shared::maze::Maze;

    #[test]
    fn test_countdown_waiting_for_time() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.transition(ClientState::Countdown);

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_none());
        assert!(matches!(session.state(), ClientState::Countdown));

        assert_eq!(ui.status_lines.len(), 1);
        assert!(ui.status_lines[0].contains("Waiting for server to start countdown"));

        assert!(ui.countdown_draws.is_empty());
    }

    #[test]
    fn test_countdown_active() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.estimated_server_time = 10.0;
        session.countdown_end_time = Some(15.0);

        session.transition(ClientState::Countdown);

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_none());
        assert!(matches!(session.state(), ClientState::Countdown));

        assert_eq!(ui.countdown_draws.len(), 1);
        assert_eq!(ui.countdown_draws[0], "5");
    }

    #[test]
    fn test_countdown_active_one_second() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.estimated_server_time = 14.5;
        session.countdown_end_time = Some(15.0);

        session.transition(ClientState::Countdown);

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_none());

        session.estimated_server_time = 14.001;

        assert_eq!(ui.countdown_draws.len(), 1);
        assert_eq!(ui.countdown_draws[0], "1");
    }

    #[test]
    fn test_countdown_game_start() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.maze = Some(Maze::new(Algorithm::Backtrack));
        session.players = Some(std::collections::HashMap::new());

        session.estimated_server_time = 10.0;
        session.countdown_end_time = Some(9.0);

        session.transition(ClientState::Countdown);

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_some());
        assert!(matches!(next_state, Some(ClientState::InGame { .. })));
    }

    #[test]
    fn test_countdown_disconnected() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.transition(ClientState::Countdown);
        network.set_disconnected(true, "Server hung up.");

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_some());
        if let Some(ClientState::TransitioningToDisconnected { message }) = next_state {
            assert!(message.contains("Server hung up."));
        } else {
            panic!("Did not transition to disconnected state.");
        }
    }
}
