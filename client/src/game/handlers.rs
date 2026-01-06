use bincode::{config::standard, serde::encode_to_vec};

use crate::{
    assets::Assets,
    game::{input::player_input_from_keys, state::Game},
    net::NetworkHandle,
    state::ClientState,
};
use common::{net::AppChannel, protocol::ClientMessage, ring::WireItem};

pub fn handle(
    game_state: &mut Game,
    assets: &Assets,
    network: &mut dyn NetworkHandle,
    target_tick: u64,
) -> Option<ClientState> {
    game_state.update();
    game_state.draw(assets);

    let wire_tick: u16 = target_tick as u16;

    let input = player_input_from_keys(target_tick);

    let wire_input = WireItem {
        id: wire_tick,
        data: input,
    };
    let client_message = ClientMessage::Input(wire_input);
    let payload =
        encode_to_vec(&client_message, standard()).expect("failed to encode player input");
    network.send_message(AppChannel::Unreliable, payload);

    game_state.input_history.insert(target_tick, input);

    println!("{:?}", client_message);

    None
}

// // TODO: Extract the "physics loop" and `render`.
// fn get_target_tick(network: &mut dyn NetworkHandle) -> u64 {
//     const SERVER_TICK_RATE: f64 = 60.0;
//     const TICK_DURATION_IDEAL: f64 = 1.0 / SERVER_TICK_RATE;
//     // Three ticks (50ms) is probably a safe starting buffer.
//     // If inputs arrive late on the server, increase this.
//     const JITTER_SAFETY_MARGIN: f64 = 0.05; // Consider raising to 4 ticks?

//     let raw_delta_time = macroquad::time::get_frame_time(); // Consider using std::time.
//     let tick_duration_actual = std::time::Duration::from_secs_f32(raw_delta_time);

//     // Update the "road conditions" (RTT).
//     // We use asymmetric smoothing:
//     // - If RTT goes UP (lag spike), we adapt QUICKLY (0.1) to prevent input starvation.
// - If RTT goes DOWN (improvement), we adapt SLOWLY (0.01) to keep simulation stable.
// let current_rtt = network.rtt().clamp(0.0, 1.0); // Discard excessively long rtt.
// let rtt_alpha = if current_rtt > session.smoothed_rtt {
//     0.1
// } else {
//     0.01
// };

// // Simple linear interpolation.
// // Encapsulate as `lerp(session.smoothed_rtt, renet.rtt(), alpha)`.
// session.smoothed_rtt = session.smoothed_rtt * (1.0 - rtt_alpha) + current_rtt * rtt_alpha;

// // Target = "What time is it now" + "Travel Time" + "Safety Margin".
// let travel_time = session.smoothed_rtt / 2.0;
// let target_sim_time = session.estimated_server_time + travel_time + JITTER_SAFETY_MARGIN; // 'input arrival time'
// let target_tick = (target_sim_time / TICK_DURATION_IDEAL).floor() as u64;

// // "Where we should be" minus "Where we are".
// let error = target_sim_time - session.simulated_time;

// let adjustment = if error.abs() > 0.25 {
//     // CASE A: HARD SNAP
//     // We are > 250ms off. The internet choked or we just connected.
//     // Teleport immediately to avoid speeding up for 10 seconds.
//     println!("Simulation lag spike: catching up by {:.4}s", error);

//     // We force the error to be exactly enough to close the gap instantly.
//     error
// } else {
//     // CASE B: CLAMPED NUDGE
//     // We are slightly off. Nudge the clock by +/- 10% of the error.
//     // Limit the nudge to +/- 2ms per frame to prevent visual stutter.
//     (error * 0.1).clamp(-0.002, 0.002)
// };

// // We add real time + adjustment.
// // If we are behind, adjustment is positive (simulation runs faster).
// // If we are ahead, adjustment is negative (simulation runs slower).
// session.accumulator += raw_delta_time + adjustment;

// const MAX_TICKS_PER_FRAME: u8 = 8; // A failsafe to prevent the accumulator from growing
// let mut ticks_processed = 0; // ever greater if we fall behind.
// while session.accumulator >= TICK_DURATION_IDEAL && ticks_processed < MAX_TICKS_PER_FRAME {
//     process_input(&mut session, target_tick); // Insert into history, send to server.
//     perform_tick(&mut session); // Run physics: reconcile and predict.

//     // C. Advance State.
//     session.accumulator -= TICK_DURATION_IDEAL;
//     session.current_tick += 1;
//     ticks_processed += 1;
//     session.simulated_time += TICK_DURATION_IDEAL;

//     // Track our time using the fixed step to stay perfectly in sync with ticks.
//     session.simulated_time += TICK_DURATION_IDEAL;

//     // If we hit the limit, discard the remaining accumulator to prevent spiral.
//     if ticks_processed >= MAX_TICKS_PER_FRAME {
//         session.accumulator = 0.0; // Or keep a small remainder, but discard the bulk.
//             println!("Physics spiral detected: skipped ticks to catch up.");
//         }
//     }

//     // 8. RENDER INTERPOLATION
//     let alpha = session.accumulator / TICK_DURATION_IDEAL;
//     render(alpha);

//     target_tick

//     // Placeholder.
//     1
// }
