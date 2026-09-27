//! Rebindable keys: the actions, their default keys and the key names shown in the options.

use crate::lang::is_hungarian;
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bind {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Sneak,
    Sprint,
    Zoom,
    Inventory,
    Drop,
    Chat,
    Command,
    PlayerList,
    Fly,
    Perspective,
    HideHud,
    Debug,
    Fullscreen,
    Hotbar1,
    Hotbar2,
    Hotbar3,
    Hotbar4,
    Hotbar5,
    Hotbar6,
    Hotbar7,
    Hotbar8,
    Hotbar9,
}

/// (action, name in options.txt and the translation key `key.<name>`, default key), in the
/// order the options list them.
pub const BINDS: [(Bind, &str, KeyCode); 27] = [
    (Bind::Forward, "forward", KeyCode::KeyW),
    (Bind::Back, "back", KeyCode::KeyS),
    (Bind::Left, "left", KeyCode::KeyA),
    (Bind::Right, "right", KeyCode::KeyD),
    (Bind::Jump, "jump", KeyCode::Space),
    (Bind::Sneak, "sneak", KeyCode::ShiftLeft),
    (Bind::Sprint, "sprint", KeyCode::ControlLeft),
    (Bind::Zoom, "zoom", KeyCode::KeyV),
    (Bind::Inventory, "inventory", KeyCode::KeyE),
    (Bind::Drop, "drop", KeyCode::KeyQ),
    (Bind::Chat, "chat", KeyCode::KeyT),
    (Bind::Command, "command", KeyCode::Slash),
    (Bind::PlayerList, "playerlist", KeyCode::Tab),
    (Bind::Fly, "fly", KeyCode::KeyF),
    (Bind::Perspective, "perspective", KeyCode::F5),
    (Bind::HideHud, "hidehud", KeyCode::F1),
    (Bind::Debug, "debug", KeyCode::F3),
    (Bind::Fullscreen, "fullscreen", KeyCode::F11),
    (Bind::Hotbar1, "hotbar1", KeyCode::Digit1),
    (Bind::Hotbar2, "hotbar2", KeyCode::Digit2),
    (Bind::Hotbar3, "hotbar3", KeyCode::Digit3),
    (Bind::Hotbar4, "hotbar4", KeyCode::Digit4),
    (Bind::Hotbar5, "hotbar5", KeyCode::Digit5),
    (Bind::Hotbar6, "hotbar6", KeyCode::Digit6),
    (Bind::Hotbar7, "hotbar7", KeyCode::Digit7),
    (Bind::Hotbar8, "hotbar8", KeyCode::Digit8),
    (Bind::Hotbar9, "hotbar9", KeyCode::Digit9),
];

pub const COUNT: usize = BINDS.len();

/// The groups the key binds screen shows (translation key of the heading, the actions).
pub const CATEGORIES: [(&str, &[Bind]); 4] = [
    (
        "keys.cat.movement",
        &[
            Bind::Forward,
            Bind::Back,
            Bind::Left,
            Bind::Right,
            Bind::Jump,
            Bind::Sneak,
            Bind::Sprint,
            Bind::Fly,
        ],
    ),
    (
        "keys.cat.inventory",
        &[
            Bind::Inventory,
            Bind::Drop,
            Bind::Hotbar1,
            Bind::Hotbar2,
            Bind::Hotbar3,
            Bind::Hotbar4,
            Bind::Hotbar5,
            Bind::Hotbar6,
            Bind::Hotbar7,
            Bind::Hotbar8,
            Bind::Hotbar9,
        ],
    ),
    (
        "keys.cat.multiplayer",
        &[Bind::Chat, Bind::Command, Bind::PlayerList],
    ),
    (
        "keys.cat.view",
        &[
            Bind::Zoom,
            Bind::Perspective,
            Bind::HideHud,
            Bind::Debug,
            Bind::Fullscreen,
        ],
    ),
];

pub const HOTBAR: [Bind; 9] = [
    Bind::Hotbar1,
    Bind::Hotbar2,
    Bind::Hotbar3,
    Bind::Hotbar4,
    Bind::Hotbar5,
    Bind::Hotbar6,
    Bind::Hotbar7,
    Bind::Hotbar8,
    Bind::Hotbar9,
];

/// The key of every action, indexed by `Bind as usize`.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyMap(pub [KeyCode; COUNT]);

impl Default for KeyMap {
    fn default() -> Self {
        Self(BINDS.map(|(_, _, k)| k))
    }
}

impl KeyMap {
    pub fn get(&self, b: Bind) -> KeyCode {
        self.0[b as usize]
    }

    pub fn is(&self, b: Bind, code: KeyCode) -> bool {
        self.get(b) == code
    }

    /// Another action on the same key.
    pub fn conflicts(&self, i: usize) -> bool {
        let k = self.0[i];
        self.0.iter().enumerate().any(|(j, &o)| j != i && o == k)
    }
}

/// Keys that can be bound (Escape stays the menu key).
const BINDABLE: &[KeyCode] = {
    use KeyCode::*;
    &[
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN,
        KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ, Digit0, Digit1,
        Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, F1, F2, F3, F4, F5, F6,
        F7, F8, F9, F10, F11, F12, Space, Tab, CapsLock, ShiftLeft, ShiftRight, ControlLeft,
        ControlRight, AltLeft, AltRight, Enter, Backspace, ArrowUp, ArrowDown, ArrowLeft,
        ArrowRight, Insert, Delete, Home, End, PageUp, PageDown, Numpad0, Numpad1, Numpad2,
        Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9, NumpadAdd,
        NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal, NumpadEnter, Minus, Equal,
        BracketLeft, BracketRight, Backslash, Semicolon, Quote, Comma, Period, Slash, Backquote,
        IntlBackslash,
    ]
};

pub fn bindable(code: KeyCode) -> bool {
    BINDABLE.contains(&code)
}

/// The name saved in options.txt (winit's name, e.g. "KeyW").
pub fn code_name(code: KeyCode) -> String {
    format!("{code:?}")
}

pub fn parse(name: &str) -> Option<KeyCode> {
    BINDABLE.iter().copied().find(|&k| code_name(k) == name)
}

/// Name on the key cap, e.g. "W", "Left Shift".
pub fn display(code: KeyCode) -> String {
    use KeyCode::*;
    let hu = is_hungarian();
    let pick = |en: &str, h: &str| if hu { h } else { en }.to_string();
    let name = code_name(code);
    if let Some(c) = name.strip_prefix("Key") {
        return c.into();
    }
    if let Some(c) = name.strip_prefix("Digit") {
        return c.into();
    }
    if let Some(c) = name.strip_prefix("Numpad").filter(|c| c.len() == 1) {
        return format!("Num {c}");
    }
    match code {
        Space => pick("Space", "Szóköz"),
        ShiftLeft => pick("Left Shift", "Bal Shift"),
        ShiftRight => pick("Right Shift", "Jobb Shift"),
        ControlLeft => pick("Left Ctrl", "Bal Ctrl"),
        ControlRight => pick("Right Ctrl", "Jobb Ctrl"),
        AltLeft => pick("Left Alt", "Bal Alt"),
        AltRight => "Alt Gr".into(),
        CapsLock => "Caps Lock".into(),
        ArrowUp => pick("Up", "Fel"),
        ArrowDown => pick("Down", "Le"),
        ArrowLeft => pick("Left", "Balra"),
        ArrowRight => pick("Right", "Jobbra"),
        PageUp => "Page Up".into(),
        PageDown => "Page Down".into(),
        NumpadAdd => "Num +".into(),
        NumpadSubtract => "Num -".into(),
        NumpadMultiply => "Num *".into(),
        NumpadDivide => "Num /".into(),
        NumpadDecimal => "Num ,".into(),
        NumpadEnter => "Num Enter".into(),
        Minus => "-".into(),
        Equal => "=".into(),
        BracketLeft => "[".into(),
        BracketRight => "]".into(),
        Backslash => "\\".into(),
        Semicolon => ";".into(),
        Quote => "'".into(),
        Comma => ",".into(),
        Period => ".".into(),
        Slash => "/".into(),
        Backquote => "`".into(),
        IntlBackslash => "<".into(),
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bind_is_shown_once() {
        let mut seen = [0; COUNT];
        for (_, binds) in CATEGORIES {
            for &b in binds {
                seen[b as usize] += 1;
            }
        }
        assert!(seen.iter().all(|&n| n == 1), "{seen:?}");
    }

    #[test]
    fn defaults_are_bindable_and_distinct() {
        let map = KeyMap::default();
        for (i, (b, name, k)) in BINDS.iter().enumerate() {
            assert_eq!(*b as usize, i, "{name} is out of order");
            assert!(bindable(*k));
            assert_eq!(parse(&code_name(*k)), Some(*k));
            assert!(!map.conflicts(i), "{name} shares its key");
        }
    }
}
