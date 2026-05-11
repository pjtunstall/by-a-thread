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

    // Current State.
    pub yaw_offset: f32,
    pub pitch: f32,
    pub height_offset: f32,

    // Previous State.
    pub prev_yaw_offset: f32,
    pub prev_pitch: f32,
    pub prev_height_offset: f32,
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

            prev_yaw_offset: 0.0,
            prev_pitch: local_state.pitch,
            prev_height_offset: 0.0,
        }
    }

    pub fn update(&mut self) {
        self.accumulator += macroquad::prelude::get_frame_time();

        while self.accumulator >= OBE_TIME_STEP {
            // Capture Previous State.
            self.prev_height_offset = self.height_offset;
            self.prev_yaw_offset = self.yaw_offset;
            self.prev_pitch = self.pitch;

            // Advance Simulation.
            self.height_offset += OBE_RISE_PER_STEP;
            self.yaw_offset += self.yaw_increment;
            self.pitch += (OBE_SAFE_PITCH_LIMIT - self.pitch) * OBE_SMOOTHING_FACTOR;

            self.accumulator -= OBE_TIME_STEP;
        }
    }

    pub fn interpolate(&self) -> [f32; 3] {
        let alpha = self.accumulator / OBE_TIME_STEP;

        let interp_height_offset =
            self.prev_height_offset + (self.height_offset - self.prev_height_offset) * alpha;
        let interp_yaw_offset =
            self.prev_yaw_offset + (self.yaw_offset - self.prev_yaw_offset) * alpha;
        let interp_pitch = self.prev_pitch + (self.pitch - self.prev_pitch) * alpha;

        [interp_height_offset, interp_yaw_offset, interp_pitch]
    }
}
