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

    let n = state.players.len();
    for i in 0..n {
        let input_option = state.players[i].input_buffer.get(state.current_tick);
        let input = match input_option {
            Some(&input) => {
                state.players[i].last_input = input;
                input
            }
            None => {
                // println!(
                //     "Mismatched input ids for player '{}'; falling back to last known input",
                //     state.players[i].name
                // );
                state.players[i].last_input
            }
        };

        println!("{:?}", input);

        state.players[i].state.update(&state.maze, &input);
        state.players[i]
            .input_buffer
            .advance_tail(state.current_tick);
    }

    for i in 0..n {
        let snapshot = state.snapshot_for(i);

        // TODO: Send customized snapshot to each player: their own velocity and
        // everyone's position.
    }

    state.current_tick += 1;
    // println!("{}", state.current_tick);

    None
}
