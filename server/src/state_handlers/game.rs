use std::time::{Duration, Instant};

use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    net::ServerNetworkHandle,
    state::{Game, ServerState},
};
use common::{net::AppChannel, protocol::ClientMessage};

const NETWORK_TIME_BUDGET: Duration = Duration::from_millis(2);

// TODO:
// - Have the server increment its tick.
// - Process inputs for current tick.
// - Send customized snapshot to each player.
// (See also the `Game` struct in `server/src/state.rs`.)

// TODO: If connection times out during game (and elsewhere), show a suitable
// message in the UI; currently it just goes black.
pub fn handle(network: &mut dyn ServerNetworkHandle, state: &mut Game) -> Option<ServerState> {
    get_inputs(network, state);

    None
}

fn get_inputs(network: &mut dyn ServerNetworkHandle, state: &mut Game) {
    let start_time = Instant::now();
    let mut messages_processed: usize = 0;

    'client_loop: for client_id in network.clients_id() {
        while let Some(data) = network.receive_message(client_id, AppChannel::Unreliable) {
            if messages_processed % 10 == 0 && start_time.elapsed() > NETWORK_TIME_BUDGET {
                eprintln!("Network budget exceeded. Deferring remaining packets to next tick.");
                break 'client_loop;
            }

            messages_processed += 1;

            let Ok((message, _)) = decode_from_slice::<ClientMessage, _>(&data, standard()) else {
                eprintln!(
                    "Client {} sent malformed data. Disconnecting them.",
                    client_id
                );
                network.disconnect(client_id);
                continue;
            };

            let n = state.players.len();

            match message {
                ClientMessage::Input(input) => {
                    for i in 0..n {
                        if state.players[i].client_id == client_id {
                            // TODO: Once input-handling is in place, delete
                            // this comment. This comment is just a reminder
                            // that the input is being received correctly. It's
                            // not being inserted because the buffer insert
                            // method only inserts an item if there isn't
                            // already one for the given tick.
                            state.players[i].input_buffer.insert(input);
                            break;
                        }
                    }
                }

                _ => {
                    continue;
                }
            }
        }
    }
}
