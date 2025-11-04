use rand::Rng;

pub struct Passcode {
    pub bytes: Vec<u8>,
    pub string: String,
}

impl Passcode {
    pub fn generate(length: usize) -> Self {
        let mut rng = rand::rng();
        let bytes: Vec<u8> = (0..length).map(|_| rng.random_range(0..10)).collect();

        let string = bytes.iter().map(|d| d.to_string()).collect();

        Self { bytes, string }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_numeric_bytes_and_string_of_requested_length() {
        let length = 6;
        let passcode = Passcode::generate(length);

        assert_eq!(passcode.bytes.len(), length);
        assert_eq!(passcode.string.len(), length);
        assert!(passcode.string.chars().all(|c| c.is_ascii_digit()));

        for (index, ch) in passcode.string.chars().enumerate() {
            let digit = ch.to_digit(10).expect("expected ASCII digit") as u8;
            assert_eq!(passcode.bytes[index], digit);
            assert!(digit < 10);
        }
    }

    #[test]
    fn generate_supports_zero_length_passcodes() {
        let passcode = Passcode::generate(0);

        assert!(passcode.bytes.is_empty());
        assert!(passcode.string.is_empty());
    }
}
