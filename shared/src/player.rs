use std::fmt;

use serde::{Deserialize, Serialize};

use crate::math::Vec3;

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
