use crate::{
    net::ServerNetworkHandle,
    state::{InGame, ServerState},
};

pub fn handle_in_game(
    _network: &mut dyn ServerNetworkHandle,
    _state: &mut InGame,
) -> Option<ServerState> {
    None
}
