use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    net::NetworkHandle,
    session::ClientSession,
    state::ClientState,
    ui::{ClientUi, UiErrorKind, UiInputError},
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

    if network.is_disconnected() {
        ui.show_typed_error(
            UiErrorKind::NetworkDisconnect,
            &format!(
                "disconnected during countdown: {}",
                network.get_disconnect_reason()
            ),
        );
        return Some(ClientState::TransitioningToDisconnected {
            message: format!(
                "disconnected during countdown: {}",
                network.get_disconnect_reason()
            ),
        });
    }

    if let Err(UiInputError::Disconnected) = ui.poll_single_key() {
        return Some(ClientState::TransitioningToDisconnected {
            message: "input thread disconnected.".to_string(),
        });
    }

    // TODO: Handle relevant ServerMessages here if needed.
    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((msg, _)) => {
                let _ = msg;
            }
            Err(e) => ui.show_typed_error(
                UiErrorKind::Deserialization,
                &format!("Deserialization error: {}.", e),
            ),
        }
    }

    let Some(end_time) = session.countdown_end_time else {
        return None;
    };

    let time_remaining = end_time - session.estimated_server_time;

    if time_remaining > 0.0 {
        let countdown_value = time_remaining.floor() as u64;
        ui.draw_countdown(&format!("{}", countdown_value));
        None
    } else {
        transition_to_game(session)
    }
}

fn transition_to_game(session: &mut ClientSession) -> Option<ClientState> {
    match (session.maze.take(), session.players.take()) {
        (Some(maze), Some(players)) => Some(ClientState::InGame { maze, players }),
        (None, _) => Some(ClientState::TransitioningToDisconnected {
            message: "failed to receive maze data".to_string(),
        }),
        (_, None) => Some(ClientState::TransitioningToDisconnected {
            message: "failed to receive players data.".to_string(),
        }),
    }
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

        assert!(ui.countdown_draws.is_empty());
        assert!(ui.messages.is_empty());
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

        session.estimated_server_time = 13.5;
        session.countdown_end_time = Some(15.0);

        session.transition(ClientState::Countdown);

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_none());
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
            panic!("did not transition to disconnected state");
        }
    }
}
