```rust
// Normalize radians to 0.0 - 1.0 range, then scale to 0-255/
fn encode_yaw(yaw_radians: f32) -> u8 {
    use std::f32::consts::TAU;
    let normalized = yaw_radians % TAU;
    let normalized = if normalized < 0.0 { normalized + TAU } else { normalized };
    ((normalized / TAU) * 255.0) as u8
}

fn decode_yaw(encoded: u8) -> f32 {
    use std::f32::consts::TAU;
    (encoded as f32 / 255.0) * TAU
}

fn interpolate_yaw(start: f32, end: f32, t: f32) -> f32 {
    let mut diff = end - start;

    // Handle wrapping (shortest path).
    if diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    } else if diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    }

    start + diff * t
}

pub struct MazeSnapshot {
    pub tick: u64,
    pub active_mask: u32, // Indicates which indices the elements of the x etc. Vec corresponds to in the stored array of all players, active or otherwise.
    pub pos_x: Vec<f32>,
    pub pos_z: Vec<f32>,
    pub yaw: Vec<u8>, // The 1-byte orientation.
}
```
