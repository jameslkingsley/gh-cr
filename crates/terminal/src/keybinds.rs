use crossterm::event::{KeyCode, KeyModifiers};

pub struct Keybind {
    code: KeyCode,
    modifiers: Vec<KeyModifiers>,
    func: fn(),
}

impl Keybind {
    pub const fn new(code: KeyCode, modifiers: Vec<KeyModifiers>, func: fn()) -> Self {
        Keybind {
            code,
            modifiers,
            func,
        }
    }
}

inventory::collect!(Keybind);
