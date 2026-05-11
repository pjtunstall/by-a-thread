use macroquad::prelude::*;

// While Macroquad does provide a `get_fps` function, it fluctuates wildly for me and gives unrealistic values, hence this struct to track the frame rate. It takes an average of the last 60 frames.
use std::collections::VecDeque;

#[derive(Debug)]
pub struct FrameRate {
    times: VecDeque<f32>,
    current_sum: f32,
    sample_size: usize,
    pub rate: f32,
}

impl Default for FrameRate {
    fn default() -> Self {
        Self::new(60)
    }
}

impl FrameRate {
    pub fn new(sample_size: usize) -> Self {
        FrameRate {
            times: VecDeque::with_capacity(sample_size),
            current_sum: 0.0,
            sample_size,
            rate: 60.0,
        }
    }

    pub fn update(&mut self) {
        let dt = get_frame_time();

        self.times.push_back(dt);
        self.current_sum += dt;

        if self.times.len() > self.sample_size {
            if let Some(oldest) = self.times.pop_front() {
                self.current_sum -= oldest;
            }
        }

        if !self.times.is_empty() {
            let average_dt = self.current_sum / self.times.len() as f32;
            if average_dt > 0.0 {
                self.rate = 1.0 / average_dt;
            }
        }
    }
}
