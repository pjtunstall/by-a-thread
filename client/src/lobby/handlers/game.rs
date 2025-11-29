use macroquad::{color, prelude::*, window::clear_background};

use crate::{assets::Assets, session::ClientSession, state::ClientState};

pub fn update(session: &mut ClientSession, assets: &Assets) -> Option<ClientState> {
    let game_state = match session.state() {
        ClientState::Game(game) => game,
        other => {
            panic!(
                "called game::handle_frame when not in Game state: {:?}",
                other
            );
        }
    };

    let yaw: f32 = 0.0;
    let pitch: f32 = 0.1;

    let mut position = Default::default();
    for (id, player) in &game_state.players {
        if *id == session.client_id {
            position = vec3(player.position.x, 24.0, player.position.z)
        }
    }

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
