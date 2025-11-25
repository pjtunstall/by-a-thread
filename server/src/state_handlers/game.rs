use crate::{
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};

pub fn handle(_network: &mut dyn ServerNetworkHandle, _state: &mut Game) -> Option<ServerState> {
    None
}
