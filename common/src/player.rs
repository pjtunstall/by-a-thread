use std::{f32::consts::PI, fmt};

use glam::{Vec3, vec3};
use serde::{Deserialize, Serialize};

use crate::{
    maze::{self, Maze},
    time::TICK_RATE,
};

pub const MAX_USERNAME_LENGTH: usize = 16;

pub const HEIGHT: f32 = 24.0; // Height of the player's eye level from the ground.
pub const RADIUS: f32 = 8.0;
pub const MAX_SPEED: f32 = 240.0; // Units per second.
pub const ACCELERATION: f32 = 1200.0; // Reaches MAX_SPEED in 0.2 seconds.
pub const FRICTION: f32 = 5.0;
pub const ROTATION_SPEED: f32 = (12.0 * TICK_RATE) * (PI / 180.0);

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

    pub fn update_position(&mut self, maze: &Maze, input: &PlayerInput, dt: f32) {
        let forward = vec3(self.state.yaw.sin(), 0.0, self.state.yaw.cos());
        let right = vec3(self.state.yaw.cos(), 0.0, -self.state.yaw.sin());

        let mut wish_dir = Vec3::ZERO;

        if input.forward {
            wish_dir += forward;
        }
        if input.backward {
            wish_dir -= forward;
        }
        if input.right {
            wish_dir += right;
        }
        if input.left {
            wish_dir -= right;
        }

        if wish_dir.length_squared() > 0.001 {
            wish_dir = wish_dir.normalize();
        }

        self.state.velocity += wish_dir * ACCELERATION * dt;

        let current_speed = self.state.velocity.length();
        if current_speed > 0.0 {
            let drop = current_speed * FRICTION * dt;
            let new_speed = (current_speed - drop).max(0.0);

            if current_speed > MAX_SPEED {
                self.state.velocity = self.state.velocity.normalize() * MAX_SPEED;
            } else {
                self.state.velocity *= new_speed / current_speed;
            }
        }

        if self.state.velocity.length_squared() < 0.001 {
            self.state.velocity = Vec3::ZERO;
            return;
        }

        let p = self.state.position;
        let move_step = self.state.velocity * dt;
        let new_position = p + move_step;

        let contact_point = p + self.state.velocity.normalize() * RADIUS;

        let is_moving_forward = self.state.velocity.dot(forward) > 0.0;

        if maze.is_way_clear(&contact_point) {
            self.state.position = new_position;
        } else {
            self.state.velocity = Vec3::ZERO;

            if is_moving_forward {
                let turn_direction = maze::which_way_to_turn(&p, &contact_point);
                self.state.yaw += ROTATION_SPEED * turn_direction * dt;
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PlayerInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub yaw_delta: f32,
    pub pitch_delta: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct PlayerState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub pitch: f32,
    pub yaw: f32,
}

impl PlayerState {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: vec3(0.0, 0.0, 0.0),
            pitch: 0.1,
            yaw: 0.0,
        }
    }
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
