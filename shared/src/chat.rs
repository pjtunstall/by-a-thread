use std::fmt;

pub const MAX_USERNAME_LENGTH: usize = 16;

#[derive(Debug, PartialEq)]
pub enum UsernameError {
    Empty,
    TooLong,
    InvalidCharacter(char),
}

impl fmt::Display for UsernameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsernameError::Empty => write!(f, "Username must not be empty."),
            UsernameError::TooLong => write!(
                f,
                "Username must be at most {} characters long.",
                MAX_USERNAME_LENGTH
            ),
            UsernameError::InvalidCharacter(ch) => {
                write!(f, "Username contains invalid character: '{}'.", ch)
            }
        }
    }
}

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
