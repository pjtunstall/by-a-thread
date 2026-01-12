use glam::{Vec3, vec3};

use crate::{
    constants::{TICK_SECS, TICK_SECS_F32},
    maze::{CELL_SIZE, Maze},
    player,
};

pub const MAX_BULLETS_PER_PLAYER: usize = 24;
pub const BULLET_RADIUS: f32 = 0.1;
pub const FIRE_COOLDOWN_SECS: f64 = 0.1;
pub const SPEED: f32 = 720.0;
pub const LIFESPAN_SECS: f64 = 2.5;
pub const MAX_BOUNCES: u8 = 5;
pub const BULLET_SPAWN_OFFSET: f32 = player::RADIUS + BULLET_RADIUS + 0.1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bullet {
    pub id: u32,
    pub owner_index: usize,
    pub position: Vec3,
    pub velocity: Vec3,
    pub spawn_tick: u64,
    pub bounces: u8,
}

impl Bullet {
    pub fn new(
        id: u32,
        owner_index: usize,
        position: Vec3,
        velocity: Vec3,
        spawn_tick: u64,
    ) -> Self {
        Self {
            id,
            owner_index,
            position,
            velocity,
            spawn_tick,
            bounces: 0,
        }
    }

    pub fn advance(&mut self, ticks: u64) {
        let delta = ticks as f32 * TICK_SECS_F32;
        self.position += self.velocity * delta;
    }

    pub fn is_expired(&self, current_tick: u64) -> bool {
        let age = (current_tick - self.spawn_tick) as f64 * TICK_SECS;
        age > LIFESPAN_SECS
    }

    pub fn has_bounced_enough(&self) -> bool {
        self.bounces > MAX_BOUNCES
    }

    pub fn bounce_off_ground(&mut self) -> bool {
        if self.position.y < 0.0 && self.velocity.y < 0.0 {
            self.velocity.y *= -1.0;
            self.bounces += 1;
            true
        } else {
            false
        }
    }

    pub fn bounce_off_wall(&mut self, maze: &Maze) -> WallBounce {
        let bullet_is_not_above_wall_height = self.position.y < CELL_SIZE;
        let has_bullet_crossed_a_wall = !maze.is_way_clear(&self.position);
        let is_bullet_colliding_with_a_wall =
            bullet_is_not_above_wall_height && has_bullet_crossed_a_wall;

        if !is_bullet_colliding_with_a_wall {
            return WallBounce::None;
        }

        let direction = self.velocity.normalize_or_zero();
        if direction == Vec3::ZERO {
            return WallBounce::Stuck;
        }

        let speed = self.velocity.length() * TICK_SECS_F32;
        let normal = maze.get_wall_normal(self.position, direction, speed);
        if normal == Vec3::ZERO {
            WallBounce::Stuck
        } else {
            self.redirect(normal);
            WallBounce::HitInanimate
        }
    }

    pub fn redirect(&mut self, normal: Vec3) {
        self.velocity = reflect(self.velocity, normal);
        self.bounces += 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallBounce {
    None,
    HitInanimate,
    Stuck,
}

pub fn is_bullet_colliding_with_player(bullet_position: Vec3, player_position: Vec3) -> bool {
    bullet_position.distance(player_position) < BULLET_RADIUS + player::RADIUS
}

pub fn direction_from_yaw_pitch(yaw: f32, pitch: f32) -> Vec3 {
    let direction = vec3(-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
    direction.normalize_or_zero()
}

pub fn spawn_position(player_position: Vec3, direction: Vec3) -> Vec3 {
    player_position + direction * BULLET_SPAWN_OFFSET
}

pub fn cooldown_ticks() -> u64 {
    (FIRE_COOLDOWN_SECS / TICK_SECS).ceil() as u64
}

fn reflect(direction: Vec3, normal: Vec3) -> Vec3 {
    direction - 2.0 * direction.dot(normal) * normal
}
