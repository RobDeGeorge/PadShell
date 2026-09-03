//! Maps the string key names used in TOML config (e.g. "space", "super",
//! "a") to Linux input-event key codes.

use input_linux::Key;

pub fn lookup(name: &str) -> Option<Key> {
    let key = match name.to_ascii_lowercase().as_str() {
        "a" => Key::A,
        "b" => Key::B,
        "c" => Key::C,
        "d" => Key::D,
        "e" => Key::E,
        "f" => Key::F,
        "g" => Key::G,
        "h" => Key::H,
        "i" => Key::I,
        "j" => Key::J,
        "k" => Key::K,
        "l" => Key::L,
        "m" => Key::M,
        "n" => Key::N,
        "o" => Key::O,
        "p" => Key::P,
        "q" => Key::Q,
        "r" => Key::R,
        "s" => Key::S,
        "t" => Key::T,
        "u" => Key::U,
        "v" => Key::V,
        "w" => Key::W,
        "x" => Key::X,
        "y" => Key::Y,
        "z" => Key::Z,
        "0" => Key::Num0,
        "1" => Key::Num1,
        "2" => Key::Num2,
        "3" => Key::Num3,
        "4" => Key::Num4,
        "5" => Key::Num5,
        "6" => Key::Num6,
        "7" => Key::Num7,
        "8" => Key::Num8,
        "9" => Key::Num9,
        "space" => Key::Space,
        "enter" | "return" => Key::Enter,
        "escape" | "esc" => Key::Esc,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "shift" | "leftshift" => Key::LeftShift,
        "rightshift" => Key::RightShift,
        "ctrl" | "leftctrl" => Key::LeftCtrl,
        "rightctrl" => Key::RightCtrl,
        "alt" | "leftalt" => Key::LeftAlt,
        "rightalt" => Key::RightAlt,
        "super" | "meta" | "leftmeta" | "win" => Key::LeftMeta,
        "rightmeta" => Key::RightMeta,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        _ => return None,
    };
    Some(key)
}

/// All key codes this virtual device may ever emit, for UI_SET_KEYBIT
/// registration at device-creation time.
pub fn all_supported_keys() -> Vec<Key> {
    // Cheap way to get every key `lookup` can produce without listing it
    // twice: try every name we recognize above.
    const NAMES: &[&str] = &[
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
        "space", "enter", "escape", "tab", "backspace", "up", "down", "left", "right",
        "leftshift", "rightshift", "leftctrl", "rightctrl", "leftalt", "rightalt", "leftmeta",
        "rightmeta", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
    ];
    NAMES.iter().filter_map(|n| lookup(n)).collect()
}
