use crate::{assets::Assets, game::state::Game, net::NetworkHandle, state::ClientState};

pub fn handle(
    game_state: &mut Game,
    assets: &Assets,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    game_state.update(network);
    game_state.draw(assets);

    None
}
