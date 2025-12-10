use crate::{
    assets::Assets,
    game::input::{INPUT_HISTORY_LENGTH, player_input_as_bytes, player_input_from_keys},
    net::NetworkHandle,
    session::ClientSession,
    state::ClientState,
};
use common::net::AppChannel;

// TODO: Instead of checking state each time, reorganize so as to exploit the type system.
pub fn handle(
    session: &mut ClientSession,
    assets: &Assets,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    update(session, network);
    draw(session, assets);

    None
}

fn update(session: &mut ClientSession, network: &mut dyn NetworkHandle) {
    let game_state = match session.state_mut() {
        ClientState::Game(game) => game,
        other => {
            panic!(
                "called game::handlers::handle when not in Game state: {:?}",
                other
            );
        }
    };

    // TODO: Replace this placeholder with actual `current_tick`.
    // We'll need to coerce the tick to usize, unless we can make
    // that it's original type.
    let current_tick = 0;

    let player_input = player_input_from_keys();
    let message = player_input_as_bytes(&player_input);
    network.send_message(AppChannel::Unreliable, message);
    game_state.input_history.history[current_tick % (INPUT_HISTORY_LENGTH - 1)] =
        Some(player_input);

    // TODO: Replace the following placeholder positioning with full reconciliation and prediction logic.
}

fn draw(session: &mut ClientSession, assets: &Assets) {
    let game_state = match session.state() {
        ClientState::Game(game) => game,
        other => {
            panic!(
                "called game::handlers::handle when not in Game state: {:?}",
                other
            );
        }
    };

    let position = game_state.snapshot.players[session.player_index]
        .state
        .position;
    game_state.draw(&assets.wall_texture, position);
}
