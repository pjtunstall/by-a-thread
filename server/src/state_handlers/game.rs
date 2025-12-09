use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};
use common::{net::AppChannel, player::PlayerInput};

pub fn handle(network: &mut dyn ServerNetworkHandle, _state: &mut Game) -> Option<ServerState> {
    for client_id in network.clients_id() {
        while let Some(data) = network.receive_message(client_id, AppChannel::Unreliable) {
            let decoded = decode_from_slice::<PlayerInput, _>(&data, standard());
            if let Ok((input, _)) = decoded {
                // TODO: Pass PlayerInput to this player's update method.
                let _ = input;
            }
        }
    }

    None
}
