use std::{
    fmt,
    time::{Duration, Instant},
};

use bincode::{config::standard, serde::decode_from_slice};

use crate::{net::ServerNetworkHandle, player::ServerPlayer, state::Game};
use common::{net::AppChannel, protocol::ClientMessage};

// A guard against getting stuck here if messages are coming faster than we can
// drain the queue.
const NETWORK_TIME_BUDGET: Duration = Duration::from_millis(2);
// An independent guard against excessive messages arriving from one client;
// when this limit is reached, we skip subsequent messages till there are no
// more messages from that client or the time limit is reached.
const MAX_MESSAGES_PER_CLIENT_PER_TICK: u32 = 32;
// This is how many ticks we'll allow a client to exceed their message limit before
// disconnecting them. `over_cap_strikes` decays by 1 on ticks where the client
// stays under the cap, so we slowly forgive a client who improves.
const MAX_OVER_CAP_STRIKES: u8 = 8;

// TODO: Consider how realistic these numbers are?

pub fn receive_inputs(network: &mut dyn ServerNetworkHandle, state: &mut Game) {
    let start_time = Instant::now();
    let mut total_messages_received: u32 = 0;
    let total_players = state.players.len();
    let mut messages_received = vec![0_u32; total_players];
    let mut over_cap_recorded = vec![false; total_players];

    'client_loop: for client_id in network.clients_id() {
        let Some(&player_index) = state.client_id_to_index.get(&client_id) else {
            eprintln!("client_id {client_id} not found in `client_id_to_index` `HashMap`");
            continue;
        };

        while let Some(data) = network.receive_message(client_id, AppChannel::Unreliable) {
            // Doesn't protect against a client sending a huge message.
            if total_messages_received % 10 == 0 && start_time.elapsed() > NETWORK_TIME_BUDGET {
                println!("{}", TimeBudgetEvent::Exceeded.message());
                break 'client_loop;
            }

            total_messages_received += 1;

            let player = &mut state.players[player_index];
            let cap_outcome = apply_input_cap(
                player,
                &mut messages_received[player_index],
                &mut over_cap_recorded[player_index],
            );
            if let Some(event) = cap_outcome.event {
                match event {
                    InputCapEvent::OverLimit { .. } => {
                        println!("{}", event.message(&player.name))
                    }
                    InputCapEvent::Disconnected => {
                        eprintln!("{}", event.message(&player.name))
                    }
                }
            }
            match cap_outcome.action {
                InputCapAction::Process => {}
                InputCapAction::Skip => continue,
                InputCapAction::Disconnect => {
                    network.disconnect(client_id);
                    break;
                }
            }

            let message = match decode_message(&data) {
                Ok(message) => message,
                Err(error) => {
                    println!("{}", error.message(client_id, &player.name));
                    network.disconnect(client_id);
                    break;
                }
            };

            if let Err(error) = handle_message(player, message) {
                println!("{}", error.message(client_id, &player.name));
                network.disconnect(client_id);
                break;
            }
        }
    }

    for (player_index, player) in state.players.iter_mut().enumerate() {
        if !over_cap_recorded[player_index] && player.over_cap_strikes > 0 {
            player.over_cap_strikes -= 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputCapAction {
    Process,
    Skip,
    Disconnect,
}

struct InputCapOutcome {
    action: InputCapAction,
    event: Option<InputCapEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputCapEvent {
    OverLimit { strikes: u8 },
    Disconnected,
}

impl InputCapEvent {
    fn message(&self, player_name: &str) -> String {
        match self {
            InputCapEvent::OverLimit { .. } => format!(
                "Player '{player_name}' exceeded the per-tick message limit; discarding further messages this tick."
            ),
            InputCapEvent::Disconnected => format!(
                "Player '{player_name}' repeatedly exceeded the message limit; disconnecting them."
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeBudgetEvent {
    Exceeded,
}

impl TimeBudgetEvent {
    fn message(&self) -> &'static str {
        match self {
            TimeBudgetEvent::Exceeded => {
                "Time budget exceeded; deferring collection of any further messages till the next tick."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputError {
    Malformed,
    UnknownType,
}

impl InputError {
    fn message(&self, client_id: u64, player_name: &str) -> String {
        format!("Client {client_id} ({player_name}) {self}; disconnecting them.")
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputError::Malformed => formatter.write_str("sent malformed data"),
            InputError::UnknownType => formatter.write_str("sent an unsupported message type"),
        }
    }
}

impl std::error::Error for InputError {}

fn decode_message(data: &[u8]) -> Result<ClientMessage, InputError> {
    decode_from_slice::<ClientMessage, _>(data, standard())
        .map(|(message, _)| message)
        .map_err(|_| InputError::Malformed)
}

fn handle_message(player: &mut ServerPlayer, message: ClientMessage) -> Result<(), InputError> {
    match message {
        ClientMessage::Input(input) => {
            player.input_buffer.insert(input);
            Ok(())
        }
        _ => Err(InputError::UnknownType),
    }
}

fn apply_input_cap(
    player: &mut ServerPlayer,
    messages_received: &mut u32,
    over_cap_recorded: &mut bool,
) -> InputCapOutcome {
    if *messages_received >= MAX_MESSAGES_PER_CLIENT_PER_TICK {
        let mut event = None;
        if !*over_cap_recorded {
            *over_cap_recorded = true;
            player.over_cap_strikes += 1;
            if player.over_cap_strikes >= MAX_OVER_CAP_STRIKES {
                event = Some(InputCapEvent::Disconnected);
            } else {
                event = Some(InputCapEvent::OverLimit {
                    strikes: player.over_cap_strikes,
                });
            }
        }
        let action = match event {
            Some(InputCapEvent::Disconnected) => InputCapAction::Disconnect,
            _ => InputCapAction::Skip,
        };
        return InputCapOutcome { action, event };
    }

    *messages_received += 1;
    InputCapOutcome {
        action: InputCapAction::Process,
        event: None,
    }
}
