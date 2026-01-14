use std::{
    fmt,
    time::{Duration, Instant},
};

use bincode::{config::standard, serde::decode_from_slice};
use rand::{rng, seq::SliceRandom};

use crate::{net::ServerNetworkHandle, player::ServerPlayer, state::Game};
use common::{net::AppChannel, protocol::ClientMessage};

const NETWORK_TIME_BUDGET: Duration = Duration::from_millis(2);
const MAX_MESSAGES_PER_CLIENT_PER_TICK: u32 = 128;
const MAX_OVER_CAP_STRIKES: u8 = 8;

pub fn receive_inputs(network: &mut dyn ServerNetworkHandle, state: &mut Game) {
    let start_time = Instant::now();
    let mut total_messages_received: u32 = 0;

    let mut is_shedding_load = false;

    let mut client_ids: Vec<_> = network.clients_id().into_iter().collect();

    // This randomization ensures that if the server is overloaded, message loss
    // is distributed fairly rather than punishing the same player every tick.
    client_ids.shuffle(&mut rng());

    for client_id in client_ids {
        if state.after_game_chat_clients.contains(&client_id) {
            let mut ingress_bytes = 0usize;
            while let Some(data) = network.receive_message(client_id, AppChannel::Unreliable) {
                ingress_bytes = ingress_bytes.saturating_add(data.len());
            }
            state.note_ingress_bytes(ingress_bytes);
            continue;
        }

        let Some(&player_index) = state.client_id_to_index.get(&client_id) else {
            eprintln!("client {client_id} connected but not in player index yet; skipping");
            continue;
        };

        let mut messages_this_client = 0;
        let mut ingress_bytes = 0usize;

        {
            let player = &mut state.players[player_index];

            while let Some(data) = network.receive_message(client_id, AppChannel::Unreliable) {
                ingress_bytes = ingress_bytes.saturating_add(data.len());
                if total_messages_received % 10 == 0 && start_time.elapsed() > NETWORK_TIME_BUDGET {
                    if !is_shedding_load {
                        println!("{}", TimeBudgetEvent::Exceeded.message());
                        is_shedding_load = true;
                    }
                }

                if is_shedding_load {
                    continue;
                }

                total_messages_received += 1;

                let cap_outcome = apply_input_cap(player, &mut messages_this_client);

                if let Some(event) = cap_outcome.event {
                    match event {
                        InputCapEvent::OverLimit { .. } => {
                            println!("{}", event.message(&player.name));
                        }
                        InputCapEvent::Disconnected => {
                            eprintln!("{}", event.message(&player.name));
                        }
                    }
                }

                match cap_outcome.action {
                    InputCapAction::Process => {}
                    InputCapAction::Skip => {
                        continue;
                    }
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

            // We forgive one strike if the client stayed under the message limit
            // this tick.
            if messages_this_client < MAX_MESSAGES_PER_CLIENT_PER_TICK
                && player.over_cap_strikes > 0
            {
                player.over_cap_strikes -= 1;
            }
        }

        state.note_ingress_bytes(ingress_bytes);
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
                "player '{player_name}' exceeded the per-tick message limit; discarding further messages this tick"
            ),
            InputCapEvent::Disconnected => format!(
                "player '{player_name}' repeatedly exceeded the message limit; disconnecting them"
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
                "Time budget exceeded; dropping remaining messages to flush the queue."
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
        format!("client {client_id} ({player_name}) {self}; disconnecting them")
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
        ClientMessage::EnterAfterGameChat | ClientMessage::SendChat(_) => Ok(()),
        _ => Err(InputError::UnknownType),
    }
}

fn apply_input_cap(player: &mut ServerPlayer, messages_received: &mut u32) -> InputCapOutcome {
    if *messages_received >= MAX_MESSAGES_PER_CLIENT_PER_TICK {
        let mut event = None;

        // Only apply a strike when they first hit the limit.
        if *messages_received == MAX_MESSAGES_PER_CLIENT_PER_TICK {
            player.over_cap_strikes += 1;

            if player.over_cap_strikes >= MAX_OVER_CAP_STRIKES {
                event = Some(InputCapEvent::Disconnected);
            } else {
                event = Some(InputCapEvent::OverLimit {
                    strikes: player.over_cap_strikes,
                });
            }
        }

        *messages_received += 1;

        let action = if player.over_cap_strikes >= MAX_OVER_CAP_STRIKES {
            InputCapAction::Disconnect
        } else {
            InputCapAction::Skip
        };

        return InputCapOutcome { action, event };
    }

    *messages_received += 1;
    InputCapOutcome {
        action: InputCapAction::Process,
        event: None,
    }
}
