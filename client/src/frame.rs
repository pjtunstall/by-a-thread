use macroquad::prelude::*;

// While Macroquad does provide a `get_fps` function, it fluctuates wildly for me and gives unrealistic values, hence this struct to track the frame rate. It takes an average of the last 60 frames.
#[derive(Default, Debug)]
pub struct FrameRate {
    pub times: Vec<f32>,
    pub rate: f32,
}

impl FrameRate {
    pub fn new() -> Self {
        FrameRate {
            times: Vec::new(),
            rate: 60.0,
        }
    }

    pub fn update(&mut self) {
        self.times.push(get_frame_time());
        if self.times.len() > 60 {
            let sum: f32 = self.times.iter().sum();
            self.rate = 1.0 / (sum / self.times.len() as f32);
            self.times.clear();
        }
    }
}
