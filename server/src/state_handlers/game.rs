use bincode::{config::standard, serde::encode_to_vec};

use crate::{
    input,
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};
use common::{net::AppChannel, protocol::ServerMessage, ring::WireItem, snapshot::Snapshot};

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
        let snapshot = state.snapshot_for(i);
        let message = ServerMessage::Snapshot(WireItem::<Snapshot> {
            id: state.current_tick as u16,
            data: snapshot,
        });
        let payload = encode_to_vec(&message, standard()).expect("failed to serialize ServerTime");
        network.send_message(
            state.players[i].client_id,
            AppChannel::ReliableOrdered,
            payload,
        );
    }

    state.current_tick += 1;
    // println!("{}", state.current_tick);

    None
}
