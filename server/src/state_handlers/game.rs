use crate::{
    input,
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};

// TODO: If connection times out during game (and elsewhere), show a suitable
// message in the UI; currently it just goes black.
pub fn handle(network: &mut dyn ServerNetworkHandle, state: &mut Game) -> Option<ServerState> {
    input::receive_inputs(network, state);

    // TODO:
    // - Process inputs for current tick.
    // - Send customized snapshot to each player.
    // (See also the `Game` struct in `server/src/state.rs`.)

    state.current_tick += 1;
    println!("{}", state.current_tick);

    None
}
