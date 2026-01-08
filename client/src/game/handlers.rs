use bincode::{config::standard, serde::encode_to_vec};

use crate::{
    game::{input::player_input_from_keys, state::Game},
    net::NetworkHandle,
    state::ClientState,
};
use common::{net::AppChannel, protocol::ClientMessage, ring::WireItem};

pub fn handle(
    game_state: &mut Game,
    network: &mut dyn NetworkHandle,
    sim_tick: u64,
) -> Option<ClientState> {
    game_state.update();

    let wire_tick: u16 = sim_tick as u16;

    let input = player_input_from_keys(sim_tick);

    let wire_input = WireItem {
        id: wire_tick,
        data: input,
    };
    let client_message = ClientMessage::Input(wire_input);
    let payload =
        encode_to_vec(&client_message, standard()).expect("failed to encode player input");
    network.send_message(AppChannel::Unreliable, payload);

    game_state.input_history.insert(sim_tick, input);

    // println!("{:?}", client_message);

    None
}
