use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};
use common::{net::AppChannel, protocol::ClientMessage};

pub fn handle(network: &mut dyn ServerNetworkHandle, _state: &mut Game) -> Option<ServerState> {
    for client_id in network.clients_id() {
        while let Some(data) = network.receive_message(client_id, AppChannel::Unreliable) {
            let Ok((message, _)) = decode_from_slice::<ClientMessage, _>(&data, standard()) else {
                eprintln!("Client {} sent malformed data. Disconnecting.", client_id);
                network.disconnect(client_id);
                continue;
            };

            match message {
                ClientMessage::Input(input) => {
                    // TODO
                }

                _ => {
                    continue;
                }
            }
        }
    }

    None
}
