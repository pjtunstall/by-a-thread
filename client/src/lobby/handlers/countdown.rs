use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    assets::Assets,
    game::world::maze,
    lobby::ui::{LobbyUi, UiErrorKind, UiInputError},
    net::NetworkHandle,
    session::ClientSession,
    state::{ClientState, Lobby},
};
use common::{net::AppChannel, protocol::ServerMessage};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    network: &mut dyn NetworkHandle,
    assets: Option<&Assets>,
) -> Option<ClientState> {
    if !matches!(session.state(), ClientState::Lobby(Lobby::Countdown { .. })) {
        panic!(
            "called countdown::handle() when state was not Countdown; current state: {:?}",
            session.state()
        );
    }

    if let (
        Some(assets),
        ClientState::Lobby(Lobby::Countdown {
            game_data,
            maze_meshes,
            ..
        }),
    ) = (assets, session.state_mut())
    {
        if game_data.maze.grid.is_empty()
            || game_data.maze.grid[0].is_empty()
            || game_data.maze.spaces.is_empty()
        {
            return Some(ClientState::Disconnected {
                message: "maze data is missing".to_string(),
            });
        }

        if maze_meshes.is_none() {
            let built_meshes = maze::build_maze_meshes(
                &game_data.maze,
                &assets.wall_texture,
                &assets.floor_texture,
            );
            *maze_meshes = Some(built_meshes);
        }
    }

    if network.is_disconnected() {
        ui.show_typed_error(
            UiErrorKind::NetworkDisconnect,
            &format!(
                "disconnected during countdown: {}",
                network.get_disconnect_reason()
            ),
        );
        return Some(ClientState::Disconnected {
            message: format!(
                "disconnected during countdown: {}",
                network.get_disconnect_reason()
            ),
        });
    }

    if let Err(UiInputError::Disconnected) = ui.poll_single_key() {
        ui.show_sanitized_error("No connection: input thread disconnected.");
        return Some(ClientState::Disconnected {
            message: "input thread disconnected.".to_string(),
        });
    }

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

    let end_time = match session.state() {
        ClientState::Lobby(Lobby::Countdown { end_time, .. }) => *end_time,
        _ => return None,
    };

    let time_remaining = end_time - session.clock.estimated_server_time;

    let countdown_value = if time_remaining < 0.0 {
        0
    } else {
        time_remaining.floor() as u64
    };

    let font = assets.map(|assets| &assets.font);
    ui.draw_countdown(&format!("{}", countdown_value), font);

    // We return None here always. The transition to the next state,
    // Game, is triggered elsewhere: when the time reaches 0, run.rs
    // detects it and performs a zero-copy swap via
    // TransitionAction::StartGame.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{MockNetwork, MockUi};
    use common::snapshot::InitialData;

    fn countdown_state_with(end_time: f64) -> ClientState {
        ClientState::Lobby(Lobby::Countdown {
            end_time,
            game_data: InitialData::default(),
            maze_meshes: None,
        })
    }

    #[test]
    fn test_countdown_waiting_for_time() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.clock.estimated_server_time = 0.1;
        session.transition(countdown_state_with(0.4));

        let next_state = handle(&mut session, &mut ui, &mut network, None);

        assert!(
            next_state.is_none(),
            "should remain in countdown while time remains"
        );
        assert!(matches!(
            session.state(),
            ClientState::Lobby(Lobby::Countdown { .. })
        ));

        assert_eq!(ui.countdown_draws, vec!["0".to_string()]);
        assert!(ui.messages.is_empty(), "no messages should be emitted");
    }

    #[test]
    fn test_countdown_active() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.clock.estimated_server_time = 10.0;
        session.transition(countdown_state_with(15.0));

        let next_state = handle(&mut session, &mut ui, &mut network, None);

        assert!(next_state.is_none());
        assert!(matches!(
            session.state(),
            ClientState::Lobby(Lobby::Countdown { .. })
        ));

        assert_eq!(ui.countdown_draws.len(), 1);
        assert_eq!(ui.countdown_draws[0], "5");
    }

    #[test]
    fn test_countdown_active_one_second() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.clock.estimated_server_time = 13.5;
        session.transition(countdown_state_with(15.0));

        let next_state = handle(&mut session, &mut ui, &mut network, None);

        assert!(next_state.is_none());
        assert_eq!(ui.countdown_draws.len(), 1);
        assert_eq!(ui.countdown_draws[0], "1");
    }

    #[test]
    fn test_countdown_remains_none_when_finished() {
        // This test confirms that handle() relies on the external runner
        // to perform the transition. The handler itself simply updates
        // the view until that external event fires.
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.clock.estimated_server_time = 10.0;
        session.transition(countdown_state_with(9.0));

        let next_state = handle(&mut session, &mut ui, &mut network, None);

        assert!(next_state.is_none());
    }

    #[test]
    fn test_countdown_disconnected() {
        let client_id = 123;
        let mut session = ClientSession::new(client_id);
        let mut ui = MockUi::new();
        let mut network = MockNetwork::new();

        session.transition(countdown_state_with(15.0));
        network.set_disconnected(true, "Server hung up.");

        let next_state = handle(&mut session, &mut ui, &mut network, None);

        assert!(next_state.is_some());
        if let Some(ClientState::Disconnected { message }) = next_state {
            assert!(message.contains("Server hung up."));
        } else {
            panic!("did not transition to disconnected state");
        }
    }
}
