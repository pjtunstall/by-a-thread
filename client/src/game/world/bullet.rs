use macroquad::prelude::*;

use crate::game::world::maze::Maze;
use common::{
    bullets,
    constants::{TICK_SECS, TICK_SECS_F32},
};

// `ConfirmOnRed` mode is for debugging. When `BULLET_COLOR_MODE` is in this
// mode, a provisional bullet fired by the local player is white, and turns red
// on promotion (confirmation from server). A bullet fired by a
// remote player is spawned as white at the player's interpolated position, and
// turns red after fast-forwarding (blending) towards the extrapolated position
// has finished.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulletColorMode {
    ConfirmThenRed, // Debug mode.
    FadeToRed,      // Default mode.
}

pub const BULLET_COLOR_MODE: BulletColorMode = BulletColorMode::FadeToRed;

#[derive(Debug, Clone, Copy)]
pub struct ClientBullet {
    pub id: Option<u32>,
    pub fire_nonce: Option<u32>,
    pub position: Vec3,
    pub velocity: Vec3,
    pub last_update_tick: u64,
    pub spawn_tick: u64,
    pub confirmed: bool,
    pub is_local: bool,
    pub bounces: u8,
    pub blend_step: Vec3,
    pub blend_ticks_left: u8,
}

impl ClientBullet {
    pub fn new_confirmed(id: u32, position: Vec3, velocity: Vec3, last_update_tick: u64) -> Self {
        Self {
            id: Some(id),
            fire_nonce: None,
            position,
            velocity,
            last_update_tick,
            spawn_tick: last_update_tick,
            confirmed: true,
            is_local: false,
            bounces: 0,
            blend_step: Vec3::ZERO,
            blend_ticks_left: 0,
        }
    }

    pub fn new_confirmed_local(
        id: u32,
        position: Vec3,
        velocity: Vec3,
        last_update_tick: u64,
    ) -> Self {
        Self {
            id: Some(id),
            fire_nonce: None,
            position,
            velocity,
            last_update_tick,
            spawn_tick: last_update_tick,
            confirmed: true,
            is_local: true,
            bounces: 0,
            blend_step: Vec3::ZERO,
            blend_ticks_left: 0,
        }
    }

    pub fn new_provisional(
        fire_nonce: u32,
        position: Vec3,
        velocity: Vec3,
        last_update_tick: u64,
    ) -> Self {
        Self {
            id: None,
            fire_nonce: Some(fire_nonce),
            position,
            velocity,
            last_update_tick,
            spawn_tick: last_update_tick,
            confirmed: false,
            is_local: true,
            bounces: 0,
            blend_step: Vec3::ZERO,
            blend_ticks_left: 0,
        }
    }

    pub fn confirm(&mut self, id: u32, velocity: Vec3, tick: u64) {
        self.id = Some(id);
        self.fire_nonce = None;
        self.velocity = velocity;
        self.last_update_tick = tick;
        self.confirmed = true;
        self.is_local = true;
        self.bounces = 0;
        self.blend_step = Vec3::ZERO;
        self.blend_ticks_left = 0;
    }

    pub fn advance(&mut self, ticks: u64) {
        let delta = ticks as f32 * common::constants::TICK_SECS_F32;
        self.position += self.velocity * delta;
    }

    pub fn is_provisional_for(&self, fire_nonce: u32) -> bool {
        !self.confirmed && self.fire_nonce == Some(fire_nonce)
    }

    pub fn fade_amount(&self, sim_tick: u64) -> f32 {
        let age = sim_tick.saturating_sub(self.spawn_tick);
        let lifespan_ticks = (bullets::LIFESPAN_SECS / TICK_SECS).ceil() as u64;
        if lifespan_ticks == 0 {
            return 1.0;
        }
        let t = age as f32 / lifespan_ticks as f32;
        (1.0 - t).clamp(0.0, 1.0)
    }

    pub fn start_blend(&mut self, target: Vec3, ticks: u8) {
        if ticks == 0 {
            self.position = target;
            self.blend_step = Vec3::ZERO;
            self.blend_ticks_left = 0;
            return;
        }

        self.blend_step = (target - self.position) / ticks as f32;
        self.blend_ticks_left = ticks;
    }

    // TODO: Push `if` up?
    pub fn apply_blend(&mut self, ticks: u64) {
        if self.blend_ticks_left == 0 {
            return;
        }

        let steps = ticks.min(self.blend_ticks_left as u64) as u8;
        self.position += self.blend_step * steps as f32;
        self.blend_ticks_left -= steps;
    }

    pub fn has_bounced_enough(&self) -> bool {
        self.bounces > bullets::MAX_BOUNCES
    }

    pub fn bounce_off_ground(&mut self) -> bool {
        bullets::bounce_off_ground(&mut self.position, &mut self.velocity, &mut self.bounces)
    }

    pub fn bounce_off_wall(&mut self, maze: &Maze) -> bullets::WallBounce {
        bullets::bounce_off_wall(&self.position, &mut self.velocity, &mut self.bounces, maze)
    }
}

pub fn extrapolate_position(
    position: Vec3,
    velocity: Vec3,
    event_tick: u64,
    sim_tick: u64,
) -> Vec3 {
    if sim_tick <= event_tick {
        return position;
    }

    let ticks = sim_tick - event_tick;
    position + velocity * (ticks as f32 * TICK_SECS_F32)
}
