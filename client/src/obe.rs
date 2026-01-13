use ::rand::prelude::Rng;

use macroquad::prelude::{clear_background, get_frame_time, miniquad, set_default_camera}; // Imports from the prelude are restricted here so as not to clash with `rand`.

use crate::state::{Game, State, context::OBE_YAW_RANGE};

const TIME_STEP: f32 = 1.0 / 60.0;
const SAFE_PITCH_LIMIT: f32 = -1.56;
const SMOOTHING_FACTOR: f32 = 0.01;

pub trait GameOver {
    fn game_over(&mut self) -> State;
}

impl GameOver for Game {
    fn game_over(&mut self) -> State {
        let ctx = &mut self.ctx;
        let session = ctx
            .session
            .as_mut()
            .expect("There should be a Session by now, supplied by `transition_to_playing`");

        if session.fade_to_black_finished {
            set_default_camera();
            ctx.start_time = miniquad::date::now();

            let mut rng = rand::rng();
            ctx.obe_yaw_increment = rng.random_range(OBE_YAW_RANGE);
            return State::Intro;
        }

        clear_background(session.background_color);

        ctx.accumulator += get_frame_time();
        while ctx.accumulator >= TIME_STEP {
            session.update();
            session.local_player.position.y += 6.0;
            session.local_player.yaw += ctx.obe_yaw_increment;
            session.local_player.pitch +=
                (SAFE_PITCH_LIMIT - session.local_player.pitch) * SMOOTHING_FACTOR;
            ctx.accumulator -= TIME_STEP;
        }

        session.draw();

        State::GameOver
    }
}
