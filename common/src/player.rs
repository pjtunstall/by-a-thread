use std::{f32::consts::PI, fmt};

use glam::{Vec3, vec3};
use serde::{Deserialize, Serialize};
use strum::{Display, IntoStaticStr};

use crate::{constants::TICK_SECS_F32, maze::Maze};

pub const MAX_USERNAME_LENGTH: usize = 16;

// TODO: Consider changing to `f64` for simulation, and converting to `f32` to
// send and to render.

pub const HEIGHT: f32 = 24.0; // Height of the player's eye level from the ground.
pub const RADIUS: f32 = 8.0;
pub const MAX_SPEED: f32 = 240.0; // Units per second.
pub const ACCELERATION: f32 = 1200.0; // Reaches max in 0.2 seconds.
pub const FRICTION: f32 = 5.0;
pub const MAX_ROTATION_SPEED: f32 = 4.0 * PI; // 2 turns per second.
pub const ROTATION_ACCELERATION: f32 = (MAX_ROTATION_SPEED / 0.4) * PI; // Max in 0.4 seconds.
pub const ROTATION_FRICTION: f32 = 10.0; // Stop in ~0.2 seconds when key is released.
pub const MAX_HEALTH: u8 = 9;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Player {
    pub index: usize,
    pub client_id: u64,
    pub name: String,
    pub state: PlayerState,
    pub color: Color,
    pub disconnected: bool,
    pub current_tick: u64,
    pub health: u8,
}

impl Player {
    pub fn new(index: usize, client_id: u64, name: String, position: Vec3, color: Color) -> Self {
        Self {
            index,
            client_id,
            name,
            state: PlayerState::new(position),
            color,
            disconnected: false,
            current_tick: 0,
            health: MAX_HEALTH,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0 && !self.disconnected
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct PlayerState {
    pub position: Vec3,
    pub velocity: Vec3,

    pub yaw: f32,
    pub pitch: f32,

    pub yaw_velocity: f32,
    pub pitch_velocity: f32,

    pub is_zoomed: bool,
}

impl PlayerState {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: vec3(0.0, 0.0, 0.0),
            pitch: 0.1,
            yaw: 0.0,
            pitch_velocity: 0.0,
            yaw_velocity: 0.0,
            is_zoomed: false,
        }
    }

    pub fn update(
        &mut self,
        maze: &Maze,
        input: &PlayerInput,
        own_index: usize,
        player_positions: &Vec<(usize, Vec3)>,
        repulsion_strength: f32,
    ) {
        let forward = self.apply_rotation(input);
        self.apply_translation(input, forward);
        self.resolve_collision_with_walls(maze);
        self.resolve_collision_with_other_players(own_index, player_positions, repulsion_strength);
        self.is_zoomed = input.is_zoomed;
    }

    fn apply_rotation(&mut self, input: &PlayerInput) -> Vec3 {
        let mut yaw_wish = 0.0;
        if input.yaw_left {
            yaw_wish += 1.0;
        }
        if input.yaw_right {
            yaw_wish -= 1.0;
        }

        let mut pitch_wish = 0.0;
        if input.pitch_up {
            pitch_wish += 1.0;
        }
        if input.pitch_down {
            pitch_wish -= 1.0;
        }

        Self::apply_axis_rotation(
            &mut self.yaw,
            &mut self.yaw_velocity,
            yaw_wish,
            self.is_zoomed,
        );
        Self::apply_axis_rotation(
            &mut self.pitch,
            &mut self.pitch_velocity,
            pitch_wish,
            self.is_zoomed,
        );

        self.pitch = self.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.1,
            std::f32::consts::FRAC_PI_2 - 0.1,
        );

        let forward = vec3(-self.yaw.sin(), 0.0, -self.yaw.cos());

        forward
    }

    fn apply_translation(&mut self, input: &PlayerInput, forward: Vec3) {
        let mut move_wish = Vec3::ZERO;
        let right = vec3(-forward.z, 0.0, forward.x);
        if input.forward {
            move_wish += forward;
        }
        if input.backward {
            move_wish -= forward;
        }
        if input.right {
            move_wish += right;
        }
        if input.left {
            move_wish -= right;
        }

        if move_wish.length_squared() > 0.001 {
            move_wish = move_wish.normalize();
        }

        self.velocity += move_wish * ACCELERATION * TICK_SECS_F32;

        let current_speed = self.velocity.length();
        if current_speed > 0.0 {
            let drop = current_speed * FRICTION * TICK_SECS_F32;
            let new_speed = (current_speed - drop).max(0.0);

            if current_speed > MAX_SPEED {
                self.velocity = self.velocity.normalize() * MAX_SPEED;
            } else {
                self.velocity *= new_speed / current_speed;
            }
        }

        if self.velocity.length_squared() < 0.001 {
            self.velocity = Vec3::ZERO;
        }
    }

    fn resolve_collision_with_walls(&mut self, maze: &Maze) {
        if self.velocity.length_squared() < 0.001 {
            return;
        }

        let dt = TICK_SECS_F32;
        let move_step = self.velocity * dt;

        let test_pos_x = self.position + Vec3::new(move_step.x, 0.0, 0.0);
        if maze.is_sphere_clear(&test_pos_x, RADIUS) {
            self.position.x = test_pos_x.x;
        } else {
            self.velocity.x = 0.0;
        }

        let test_pos_z = self.position + Vec3::new(0.0, 0.0, move_step.z);
        if maze.is_sphere_clear(&test_pos_z, RADIUS) {
            self.position.z = test_pos_z.z;
        } else {
            self.velocity.z = 0.0;
        }
    }

    fn resolve_collision_with_other_players(
        &mut self,
        own_index: usize,
        player_positions: &[(usize, Vec3)],
        repulsion_strength: f32,
    ) {
        const MIN_DIST: f32 = RADIUS * 2.0;
        const MIN_DIST_SQ: f32 = MIN_DIST * MIN_DIST;

        for &(index, other_pos) in player_positions {
            if index == own_index {
                continue;
            }

            let diff = self.position - other_pos;
            let dist_sq = diff.length_squared();

            if dist_sq < MIN_DIST_SQ && dist_sq > 0.0001 {
                let dist = dist_sq.sqrt();
                let overlap = MIN_DIST - dist;
                let normal = diff / dist;

                self.position += normal * (overlap * repulsion_strength);

                let vel_along_normal = self.velocity.dot(normal);
                if vel_along_normal < 0.0 {
                    self.velocity -= normal * vel_along_normal;
                }
            }
        }
    }

    fn apply_axis_rotation(angle: &mut f32, velocity: &mut f32, wish: f32, is_zoomed: bool) {
        let is_driving =
            wish.abs() > 0.0 && (velocity.abs() < 0.001 || wish.signum() == velocity.signum());

        match is_driving {
            true => {
                let current_ratio = velocity.abs() / MAX_ROTATION_SPEED;

                // "Initial responsiveness" (proportion of maximum acceleration
                // available initially) + "the rest of the acceleration" * "the
                // ratio of current speed to maximum speed".
                let ramp_multiplier = if is_zoomed {
                    0.05 + (0.95 * current_ratio)
                } else {
                    0.2 + (0.8 * current_ratio)
                };

                *velocity += wish * (ROTATION_ACCELERATION * ramp_multiplier) * TICK_SECS_F32;

                if velocity.abs() > MAX_ROTATION_SPEED {
                    *velocity = velocity.signum() * MAX_ROTATION_SPEED;
                }
            }
            false => {
                let speed = velocity.abs();
                if speed > 0.001 {
                    let drop = speed * ROTATION_FRICTION * TICK_SECS_F32;
                    let new_speed = (speed - drop).max(0.0);
                    *velocity = velocity.signum() * new_speed;
                } else {
                    *velocity = 0.0;
                }

                if wish != 0.0 {
                    *velocity += wish * ROTATION_ACCELERATION * TICK_SECS_F32;
                }
            }
        }

        *angle += *velocity * TICK_SECS_F32;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub struct WirePlayerRemote {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl From<PlayerState> for WirePlayerRemote {
    fn from(player_state: PlayerState) -> Self {
        Self {
            position: player_state.position,
            yaw: player_state.yaw,
            pitch: player_state.pitch,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub struct WirePlayerLocal {
    pub position: Vec3,
    pub velocity: Vec3,

    pub yaw: f32,
    pub pitch: f32,

    pub yaw_velocity: f32,
    pub pitch_velocity: f32,

    pub is_zoomed: bool,
}

impl From<PlayerState> for WirePlayerLocal {
    fn from(player_state: PlayerState) -> Self {
        Self {
            position: player_state.position,
            velocity: player_state.velocity,

            yaw: player_state.yaw,
            pitch: player_state.pitch,

            yaw_velocity: player_state.yaw_velocity,
            pitch_velocity: player_state.pitch_velocity,

            is_zoomed: player_state.is_zoomed,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub struct PlayerInput {
    pub sim_tick: u64,
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub yaw_left: bool,
    pub yaw_right: bool,
    pub pitch_up: bool,
    pub pitch_down: bool,
    pub fire: bool,
    pub fire_nonce: Option<u32>,
    pub is_zoomed: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Display, IntoStaticStr, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum Color {
    RED,
    LIME,
    PINK,
    YELLOW,
    GREEN,
    BLUE,
    MAROON,
    ORANGE,
    PURPLE,
    #[strum(serialize = "sky blue")]
    SKYBLUE,
}

pub const COLORS: [Color; 10] = [
    Color::ORANGE,
    Color::BLUE,
    Color::LIME,
    Color::PINK,
    Color::SKYBLUE,
    Color::GREEN,
    Color::MAROON,
    Color::PURPLE,
    Color::YELLOW,
    Color::RED,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsernameError {
    Empty,
    TooLong,
    InvalidCharacter(char),
    Reserved,
}

impl fmt::Display for UsernameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsernameError::Empty => write!(f, "username cannot be empty"),
            UsernameError::TooLong => write!(f, "username is too long"),
            UsernameError::InvalidCharacter(c) => {
                write!(f, "username contains invalid character '{}'", c)
            }
            UsernameError::Reserved => write!(f, "username is reserved"),
        }
    }
}

impl std::error::Error for UsernameError {}

pub fn sanitize_username(input: &str) -> Result<String, UsernameError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(UsernameError::Empty);
    }

    if trimmed.chars().count() > MAX_USERNAME_LENGTH {
        return Err(UsernameError::TooLong);
    }

    if let Some(invalid) = trimmed
        .chars()
        .find(|ch| !ch.is_ascii_alphanumeric() && *ch != '_' && *ch != '-')
    {
        return Err(UsernameError::InvalidCharacter(invalid));
    }

    let lowercase = trimmed.to_lowercase();
    if lowercase == "server"
        || lowercase == "admin"
        || lowercase == "host"
        || lowercase == "system"
        || lowercase == "you"
    {
        return Err(UsernameError::Reserved);
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_empty_usernames() {
        assert_eq!(sanitize_username("   "), Err(UsernameError::Empty));
    }

    #[test]
    fn sanitize_rejects_usernames_that_are_too_long() {
        let long_name = "abcdefghijklmnopq"; // 17 characters, one more than permitted.
        assert_eq!(sanitize_username(long_name), Err(UsernameError::TooLong));
    }

    #[test]
    fn sanitize_rejects_usernames_with_invalid_characters() {
        assert_eq!(
            sanitize_username("user!"),
            Err(UsernameError::InvalidCharacter('!'))
        );
    }

    #[test]
    fn sanitize_accepts_valid_usernames() {
        let name = "Player_1";
        assert_eq!(sanitize_username(name), Ok(name.to_string()));
    }

    #[test]
    fn sanitize_trims_whitespace() {
        let name = "  Player-2  ";
        assert_eq!(sanitize_username(name), Ok("Player-2".to_string()));
    }
}
