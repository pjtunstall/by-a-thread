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
