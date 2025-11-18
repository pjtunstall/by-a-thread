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
