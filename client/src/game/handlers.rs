use bincode::{config::standard, serde::encode_to_vec};

use crate::{
    assets::Assets,
    game::{input::player_input_from_keys, state::Game},
    net::NetworkHandle,
    state::ClientState,
};
use common::{net::AppChannel, protocol::ClientMessage, ring::WireItem};

pub fn handle(
    game_state: &mut Game,
    assets: &Assets,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    game_state.update();
    game_state.draw(assets);

    // TODO: Replace with proper logic to calculate the target tick.
    let target_tick: u64 = 1;
    let wire_tick: u16 = target_tick as u16;

    let input = player_input_from_keys(target_tick);

    let wire_input = WireItem {
        id: wire_tick,
        data: input,
    };
    let client_message = ClientMessage::Input(wire_input);
    let payload =
        encode_to_vec(&client_message, standard()).expect("failed to encode player input");
    network.send_message(AppChannel::Unreliable, payload);

    game_state.input_history.insert(target_tick, input);

    println!("{:?}", client_message);

    None
}
