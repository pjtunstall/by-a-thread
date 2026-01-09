use crate::{
    input,
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};

// TODO:
// - Have the server increment its tick.
// - Process inputs for current tick.
// - Send customized snapshot to each player.
// (See also the `Game` struct in `server/src/state.rs`.)

// TODO: If connection times out during game (and elsewhere), show a suitable
// message in the UI; currently it just goes black.
pub fn handle(network: &mut dyn ServerNetworkHandle, state: &mut Game) -> Option<ServerState> {
    input::receive_inputs(network, state);

    None
}
