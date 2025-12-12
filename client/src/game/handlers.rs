use crate::{
    assets::Assets,
    game::{
        input::{player_input_as_bytes, player_input_from_keys},
        state::Game,
    },
    net::NetworkHandle,
    state::ClientState,
};
use common::{constants::INPUT_HISTORY_LENGTH, net::AppChannel};

pub fn handle(
    game_state: &mut Game,
    assets: &Assets,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    game_state.update();
    game_state.draw(assets);

    // TODO: Replace this placeholder with actual `target_tick`.
    let target_tick: u16 = 0;
    let tick_index_u16 = target_tick % (INPUT_HISTORY_LENGTH as u16 - 1);
    let tick_index = tick_index_u16 as usize;

    let player_input = player_input_from_keys(target_tick);
    let message = player_input_as_bytes(&player_input);
    network.send_message(AppChannel::Unreliable, message);

    // Encasulate as a method.
    game_state.input_history[tick_index] = player_input;

    None
}
