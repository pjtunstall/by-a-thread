use macroquad::{color, prelude::*, window::clear_background};

use crate::{assets::Assets, session::ClientSession, state::ClientState};

pub fn handle(session: &mut ClientSession, assets: &Assets) -> Option<ClientState> {
    let game_state = match session.state() {
        ClientState::Game(game) => game,
        other => {
            panic!(
                "called game::handlers::handle when not in Game state: {:?}",
                other
            );
        }
    };

    // TODO: Handle input:
    // let bitfield = bitfield_from_input();
    // let message = common::input::bitfield_to_bytes(bitfield);
    // Send message to server.
    // Insert bitfield into `input_history`.

    // TODO: Replace the following placeholder positioning with full reconciliation and prediction logic.

    let yaw: f32 = 0.0;
    let pitch: f32 = 0.1;

    let position = game_state.snapshot.players[session.player_index]
        .state
        .position;

    set_camera(&Camera3D {
        position,
        target: position
            + vec3(
                yaw.sin() * pitch.cos(),
                pitch.sin(),
                yaw.cos() * pitch.cos(),
            ),
        up: vec3(0.0, 1.0, 0.0),
        ..Default::default()
    });

    clear_background(color::BEIGE);
    game_state.draw(&assets.wall_texture);

    None
}
