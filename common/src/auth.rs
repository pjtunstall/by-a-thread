use rand::Rng;
use serde::{Deserialize, Serialize};

const DEFAULT_PRIVATE_KEY: [u8; 32] = [
    211, 120, 2, 54, 202, 170, 80, 236, 225, 33, 220, 193, 223, 199, 20, 80, 202, 88, 77, 123, 88,
    129, 160, 222, 33, 251, 99, 37, 145, 18, 199, 199,
];

pub fn private_key() -> [u8; 32] {
    DEFAULT_PRIVATE_KEY
}

pub const MAX_ATTEMPTS: u8 = 3;
pub const MAX_PASSCODE_LENGTH: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Passcode {
    pub bytes: [u8; 6],
    pub string: String,
}

impl Passcode {
    pub fn generate() -> Self {
        let mut rng = rand::rng();
        let bytes: [u8; 6] = std::array::from_fn(|_| rng.random_range(0..10));

        let string = bytes.iter().map(|d| d.to_string()).collect();

        Self { bytes, string }
    }

    pub fn from_bytes(bytes: [u8; 6]) -> Self {
        let string = bytes.iter().map(|d| d.to_string()).collect();
        Self { bytes, string }
    }

    pub fn from_string(string: &str) -> Option<Self> {
        if !Self::is_valid_format(string) {
            return None;
        }
        let mut bytes = [0u8; 6];
        for (i, ch) in string.chars().enumerate() {
            bytes[i] = ch.to_digit(10).unwrap() as u8;
        }
        Some(Self {
            bytes,
            string: string.to_string(),
        })
    }

    pub fn is_valid_format(s: &str) -> bool {
        s.len() == MAX_PASSCODE_LENGTH && s.chars().all(|c| c.is_ascii_digit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_numeric_bytes_and_string_of_requested_length() {
        let passcode = Passcode::generate();

        assert_eq!(passcode.bytes.len(), MAX_PASSCODE_LENGTH);
        assert_eq!(passcode.string.len(), MAX_PASSCODE_LENGTH);
        assert!(passcode.string.chars().all(|c| c.is_ascii_digit()));

        for (index, ch) in passcode.string.chars().enumerate() {
            let digit = ch.to_digit(10).expect("expected ASCII digit") as u8;
            assert_eq!(passcode.bytes[index], digit);
            assert!(digit < 10);
        }
    }
}
