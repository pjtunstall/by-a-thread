use std::time::{Duration, Instant};

use bincode::{config::standard, serde::decode_from_slice};

use crate::{
    net::ServerNetworkHandle,
    player::ServerPlayer,
    state::{Game, ServerState},
};
use common::{
    net::AppChannel,
    player::{MAX_INPUTS_PER_TICK, MAX_OVER_CAP_STRIKES},
    protocol::ClientMessage,
};

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

enum InputCapAction {
    Process,
    Skip,
    Disconnect,
}

fn get_inputs(network: &mut dyn ServerNetworkHandle, state: &mut Game) {
    let start_time = Instant::now();
    let mut total_messages_received: u32 = 0;
    let mut messages_received = vec![0_u8; state.players.len()];
    let mut over_cap_recorded = vec![false; state.players.len()];

    for player in &mut state.players {
        if player.over_cap_strikes > 0 {
            player.over_cap_strikes -= 1;
        }
    }

    'client_loop: for client_id in network.clients_id() {
        let Some(&player_index) = state.client_id_to_index.get(&client_id) else {
            panic!("client_id {client_id} not found in client_id_to_index hash map");
        };

        while let Some(data) = network.receive_message(client_id, AppChannel::Unreliable) {
            if total_messages_received % 10 == 0 && start_time.elapsed() > NETWORK_TIME_BUDGET {
                eprintln!(
                    "network budget exceeded; deferring collection of any further messages till the next tick"
                );
                break 'client_loop;
            }

            total_messages_received = total_messages_received.saturating_add(1);

            match apply_input_cap(
                client_id,
                &mut state.players[player_index],
                &mut messages_received[player_index],
                &mut over_cap_recorded[player_index],
            ) {
                InputCapAction::Process => {}
                InputCapAction::Skip => {
                    continue;
                }
                InputCapAction::Disconnect => {
                    network.disconnect(client_id);
                    break;
                }
            }

            let Ok((message, _)) = decode_from_slice::<ClientMessage, _>(&data, standard()) else {
                eprintln!(
                    "client {client_id} ({}) sent malformed data; disconnecting them",
                    state.players[player_index].name
                );
                network.disconnect(client_id);
                break;
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

fn apply_input_cap(
    client_id: u64,
    player: &mut ServerPlayer,
    messages_received: &mut u8,
    over_cap_recorded: &mut bool,
) -> InputCapAction {
    if *messages_received >= MAX_INPUTS_PER_TICK {
        if *over_cap_recorded {
            return InputCapAction::Skip;
        }
        *over_cap_recorded = true;
        player.over_cap_strikes = player.over_cap_strikes.saturating_add(1);
        if player.over_cap_strikes >= MAX_OVER_CAP_STRIKES {
            eprintln!(
                "client {client_id} ({}) repeatedly exceeded the message limit; disconnecting them",
                player.name
            );
            return InputCapAction::Disconnect;
        }
        eprintln!(
            "client {client_id} ({}) exceeded the per-tick message limit; discarding further messages this tick",
            player.name
        );
        return InputCapAction::Skip;
    }

    *messages_received = messages_received.saturating_add(1);
    InputCapAction::Process
}
