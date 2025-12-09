use std::{f32::consts::PI, fmt};

use glam::{Vec3, vec3};
use serde::{Deserialize, Serialize};

use crate::{
    maze::{self, Maze},
    time::TICK_SECS,
};

pub const MAX_USERNAME_LENGTH: usize = 16;

pub const HEIGHT: f32 = 24.0; // Height of the player's eye level from the ground.
pub const RADIUS: f32 = 8.0;
pub const MAX_SPEED: f32 = 240.0; // Units per second.
pub const ACCELERATION: f32 = 1200.0; // Reaches max in 0.2 seconds.
pub const FRICTION: f32 = 5.0;
pub const MAX_ROTATION_SPEED: f32 = 4.0 * PI; // 2 turns per second.
pub const ROTATION_ACCELERATION: f32 = (MAX_ROTATION_SPEED / 0.2) * PI; // Max in 0.2 seconds.
pub const ROTATION_FRICTION: f32 = 10.0; // Stop in ~0.2 seconds when key is released.

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Player {
    pub index: usize,
    pub client_id: u64,
    pub name: String,
    pub state: PlayerState,
    pub color: Color,
    pub disconnected: bool,
    pub alive: bool,
    // pub input_history: [Option<PlayerInput>; 128], // This really belongs in a ClientPlayer struct and doesn't need to be serialized.
    pub current_tick: u64,
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
            alive: true,
            current_tick: 0,
        }
    }

    pub fn update(&mut self, maze: &Maze, input: &PlayerInput) {
        let forward = self.apply_rotations(input);
        self.apply_translation(input, forward);
        self.resolve_collision(maze, forward);
    }

    fn apply_rotations(&mut self, input: &PlayerInput) -> Vec3 {
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

        Self::apply_axis_rotation(&mut self.state.yaw, &mut self.state.yaw_velocity, yaw_wish);
        Self::apply_axis_rotation(
            &mut self.state.pitch,
            &mut self.state.pitch_velocity,
            pitch_wish,
        );

        self.state.pitch = self.state.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.1,
            std::f32::consts::FRAC_PI_2 - 0.1,
        );

        let forward = vec3(self.state.yaw.sin(), 0.0, self.state.yaw.cos());

        forward
    }

    fn apply_translation(&mut self, input: &PlayerInput, forward: Vec3) {
        let mut move_wish = Vec3::ZERO;
        let right = vec3(forward.z, 0.0, -forward.x);
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

        self.state.velocity += move_wish * ACCELERATION * TICK_SECS;

        let current_speed = self.state.velocity.length();
        if current_speed > 0.0 {
            let drop = current_speed * FRICTION * TICK_SECS;
            let new_speed = (current_speed - drop).max(0.0);

            if current_speed > MAX_SPEED {
                self.state.velocity = self.state.velocity.normalize() * MAX_SPEED;
            } else {
                self.state.velocity *= new_speed / current_speed;
            }
        }

        if self.state.velocity.length_squared() < 0.001 {
            self.state.velocity = Vec3::ZERO;
        }
    }

    fn resolve_collision(&mut self, maze: &Maze, forward: Vec3) {
        if self.state.velocity.length_squared() < 0.001 {
            self.state.velocity = Vec3::ZERO;
            return;
        }

        let p = self.state.position;
        let move_step = self.state.velocity * TICK_SECS;
        let new_position = p + move_step;

        let contact_point = p + self.state.velocity.normalize() * RADIUS;

        let is_moving_forward = self.state.velocity.dot(forward) > 0.0;

        if maze.is_way_clear(&contact_point) {
            self.state.position = new_position;
        } else {
            self.state.velocity = Vec3::ZERO;

            if is_moving_forward {
                self.state.yaw_velocity = 0.0;
                let turn_direction = maze::which_way_to_turn(&p, &contact_point);
                self.state.yaw += MAX_ROTATION_SPEED * turn_direction * TICK_SECS;
            }
        }
    }

    #[inline(always)]
    fn apply_axis_rotation(angle: &mut f32, velocity: &mut f32, wish: f32) {
        *velocity += wish * ROTATION_ACCELERATION * TICK_SECS;

        let speed = (*velocity).abs();
        if speed > 0.001 {
            if speed > MAX_ROTATION_SPEED {
                *velocity = velocity.signum() * MAX_ROTATION_SPEED;
            } else {
                let drop = speed * ROTATION_FRICTION * TICK_SECS;
                let new_speed = (speed - drop).max(0.0);
                *velocity *= new_speed / speed;
            }
        } else {
            *velocity = 0.0;
        }

        *angle += *velocity * TICK_SECS;
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
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PlayerInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub yaw_left: bool,
    pub yaw_right: bool,
    pub pitch_up: bool,
    pub pitch_down: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
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
    SKYBLUE,
}

impl Color {
    pub fn as_str(&self) -> &'static str {
        match self {
            Color::RED => "red",
            Color::LIME => "lime",
            Color::PINK => "pink",
            Color::YELLOW => "yellow",
            Color::GREEN => "green",
            Color::BLUE => "blue",
            Color::MAROON => "maroon",
            Color::ORANGE => "orange",
            Color::PURPLE => "purple",
            Color::SKYBLUE => "sky blue",
        }
    }
}

pub const COLORS: [Color; 10] = [
    Color::RED,
    Color::LIME,
    Color::PINK,
    Color::YELLOW,
    Color::GREEN,
    Color::BLUE,
    Color::MAROON,
    Color::ORANGE,
    Color::PURPLE,
    Color::SKYBLUE,
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
        let long_name = "abcdefghijklmnopq"; // 17 characters.
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
