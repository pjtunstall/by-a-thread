#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKey {
    Char(char),
    Enter,
    Backspace,
    Esc,
    Tab,
}
