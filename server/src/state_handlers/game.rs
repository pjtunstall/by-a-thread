use crate::{
    input,
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};

// TODO: Consider if any of this logic belongs with the `Game` struct iN `server/src/state.rs`.

// TODO: If connection times out during game (and elsewhere), show a suitable
// message in the UI; currently it just goes black.
pub fn handle(network: &mut dyn ServerNetworkHandle, state: &mut Game) -> Option<ServerState> {
    input::receive_inputs(network, state);

    for player in &mut state.players {
        if let Some(&input) = player.input_buffer.get(state.current_tick) {
            player.last_input = input;
        }

        let input = player.last_input;
        // println!("{:?}", input);

        player.state.update(&state.maze, &input);
        player.input_buffer.advance_tail(state.current_tick);
    }

    for i in 0..state.players.len() {
        let _snapshot = state.snapshot_for(i);

        // TODO: Send customized snapshot to each player: their own velocity and
        // everyone's position.
    }

    state.current_tick += 1;
    // println!("{}", state.current_tick);

    None
}
