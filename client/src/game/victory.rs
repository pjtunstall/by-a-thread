pub struct VictoryEffect {
    _placeholder: (),
}

impl VictoryEffect {
    pub fn new() -> Self {
        Self { _placeholder: () }
    }

    pub fn update(&mut self) {
        // Currently just a simple fade to black with no camera effects.
        // This can be extended later to add celebratory effects.
    }
}
