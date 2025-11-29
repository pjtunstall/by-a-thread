use std::fmt;

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const MAX_USERNAME_LENGTH: usize = 16;
pub const MAX_SPEED: f32 = 4.0;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Player {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
    pub orientation: Vec3,
    pub color: Color,
}

impl Player {
    pub fn new(id: u64, name: String, position: Vec3, orientation: Vec3, color: Color) -> Self {
        Self {
            id,
            name,
            position,
            orientation,
            color,
        }
    }
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Player {{\n    id: {},\n    name: {},\n    position: {:?},\n    orientation: {:?},\n    color: {:?}\n}}\n",
            self.id, self.name, self.position, self.orientation, self.color
        )
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
