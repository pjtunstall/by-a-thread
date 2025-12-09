#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKey {
    Char(char),
    Enter,
    Backspace,
    Esc,
    Tab,
}

pub fn sanitize(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Check if the next char is '[' (CSI - Control Sequence Introducer).
            if let Some(&'[') = chars.peek() {
                chars.next(); // Consume '['

                // Skip all parameter bytes (0-9, ;, etc).
                // Range 0x30–0x3F (ASCII 48-63).
                while let Some(&p) = chars.peek() {
                    if (0x30..=0x3F).contains(&(p as u8)) {
                        chars.next();
                    } else {
                        break;
                    }
                }

                // Consume the final byte (the command, e.g., 'm' for color).
                // Range 0x40–0x7E (ASCII 64-126).
                if let Some(&f) = chars.peek() {
                    if (0x40..=0x7E).contains(&(f as u8)) {
                        chars.next();
                    }
                }
            }
        } else if !c.is_control() {
            // Filter out other control chars like Bell (\x07).
            output.push(c);
        }
    }

    output
}

pub const W_KEY_HELD: u32 = 1 << 0;
pub const A_KEY_HELD: u32 = 1 << 1;
pub const S_KEY_HELD: u32 = 1 << 2;
pub const D_KEY_HELD: u32 = 1 << 3;

pub const UP_KEY_HELD: u32 = 1 << 4;
pub const LEFT_KEY_HELD: u32 = 1 << 5;
pub const DOWN_KEY_HELD: u32 = 1 << 6;
pub const RIGHT_KEY_HELD: u32 = 1 << 7;

pub const W_KEY_PRESSED: u32 = 1 << 8;
pub const A_KEY_PRESSED: u32 = 1 << 9;
pub const S_KEY_PRESSED: u32 = 1 << 10;
pub const D_KEY_PRESSED: u32 = 1 << 11;

pub const UP_KEY_PRESSED: u32 = 1 << 12;
pub const LEFT_KEY_PRESSED: u32 = 1 << 13;
pub const DOWN_KEY_PRESSED: u32 = 1 << 14;
pub const RIGHT_KEY_PRESSED: u32 = 1 << 15;

pub const SPACE_KEY_HELD: u32 = 1 << 16;
pub const SPACE_KEY_PRESSED: u32 = 1 << 17;

pub fn has_input(bitfield: u32, flag: u32) -> bool {
    bitfield & flag != 0
}

pub fn bitfield_to_bytes(bitfield: u32) -> Vec<u8> {
    bitfield.to_le_bytes().to_vec()
}

pub fn bytes_to_bitfield(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != 4 {
        eprintln!(
            "{}",
            format!("Expected 4 input bytes, got {}.", bytes.len())
        );
        return None;
    }
    let mut array = [0u8; 4];
    array.copy_from_slice(bytes);
    Some(u32::from_le_bytes(array))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_input() {
        let bitfield = W_KEY_HELD | UP_KEY_HELD;
        assert!(has_input(bitfield, W_KEY_HELD));
        assert!(has_input(bitfield, UP_KEY_HELD));
        assert!(!has_input(bitfield, A_KEY_PRESSED));
    }

    #[test]
    fn test_bytes_to_bitfield() {
        let original = W_KEY_HELD | LEFT_KEY_PRESSED | LEFT_KEY_HELD | SPACE_KEY_HELD;
        let bytes = bitfield_to_bytes(original);
        let recovered = bytes_to_bitfield(&bytes).unwrap();
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_bytes_to_bitfield_invalid_length() {
        assert_eq!(bytes_to_bitfield(&[1, 2, 3]), None);
        assert_eq!(bytes_to_bitfield(&[]), None);
    }
}
