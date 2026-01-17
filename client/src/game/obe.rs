use ::rand::{Rng, rng};

use common::player;

const OBE_TIME_STEP: f32 = 1.0 / 60.0;
const OBE_SAFE_PITCH_LIMIT: f32 = -1.56;
const OBE_SMOOTHING_FACTOR: f32 = 0.01;
const OBE_RISE_PER_STEP: f32 = 6.0;
const OBE_YAW_RANGE: std::ops::Range<f32> = 0.01..0.03;

pub struct ObeEffect {
    accumulator: f32,
    pub yaw_increment: f32,
    pub yaw_offset: f32,
    pub pitch: f32,
    pub height_offset: f32,
}

impl ObeEffect {
    pub fn new(local_state: player::PlayerState) -> Self {
        let mut rng = rng();
        let mut yaw_increment = rng.random_range(OBE_YAW_RANGE);
        if rng.random_range(0..2) == 0 {
            yaw_increment = -yaw_increment;
        }

        Self {
            accumulator: 0.0,
            yaw_increment,
            yaw_offset: 0.0,
            pitch: local_state.pitch,
            height_offset: 0.0,
        }
    }

    pub fn update(&mut self) {
        self.accumulator += macroquad::prelude::get_frame_time();

        while self.accumulator >= OBE_TIME_STEP {
            self.height_offset += OBE_RISE_PER_STEP;
            self.yaw_offset += self.yaw_increment;
            self.pitch += (OBE_SAFE_PITCH_LIMIT - self.pitch) * OBE_SMOOTHING_FACTOR;
            self.accumulator -= OBE_TIME_STEP;
        }
    }
}
