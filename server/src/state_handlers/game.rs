use crate::{
    net::ServerNetworkHandle,
    state::{InGame, ServerState},
};

pub fn handle(_network: &mut dyn ServerNetworkHandle, _state: &mut InGame) -> Option<ServerState> {
    None
}
