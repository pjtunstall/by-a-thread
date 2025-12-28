use bincode::{config::standard, serde::encode_to_vec};

use crate::{
    assets::Assets,
    game::{input::player_input_from_keys, state::Game},
    net::NetworkHandle,
    state::ClientState,
};
use common::{
    net::AppChannel,
    ring::{WireItem, u16_from_u64},
};

pub fn handle(
    game_state: &mut Game,
    assets: &Assets,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    game_state.update();
    game_state.draw(assets);

    let target_tick: u64 = 0;
    let wire_tick: u16 = u16_from_u64(target_tick);

    let input = player_input_from_keys(target_tick);
    game_state.input_history.insert(target_tick, input);

    let wire_input = WireItem {
        id: wire_tick,
        data: input,
    };
    let message = encode_to_vec(&wire_input, standard()).expect("failed to encode player input");
    network.send_message(AppChannel::Unreliable, message);

    None
}
