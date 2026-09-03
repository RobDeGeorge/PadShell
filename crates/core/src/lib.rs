//! Shared vocabulary used by every padshell crate: button/axis identifiers,
//! the decoded controller state, and the string<->enum tables config parsing
//! and (eventually) a GUI both rely on.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ButtonId {
    Cross,
    Circle,
    Square,
    Triangle,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    Share,
    Options,
    Ps,
    TouchpadClick,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

impl ButtonId {
    pub const ALL: &'static [ButtonId] = &[
        ButtonId::Cross,
        ButtonId::Circle,
        ButtonId::Square,
        ButtonId::Triangle,
        ButtonId::L1,
        ButtonId::R1,
        ButtonId::L2,
        ButtonId::R2,
        ButtonId::L3,
        ButtonId::R3,
        ButtonId::Share,
        ButtonId::Options,
        ButtonId::Ps,
        ButtonId::TouchpadClick,
        ButtonId::DpadUp,
        ButtonId::DpadDown,
        ButtonId::DpadLeft,
        ButtonId::DpadRight,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ButtonId::Cross => "cross",
            ButtonId::Circle => "circle",
            ButtonId::Square => "square",
            ButtonId::Triangle => "triangle",
            ButtonId::L1 => "l1",
            ButtonId::R1 => "r1",
            ButtonId::L2 => "l2",
            ButtonId::R2 => "r2",
            ButtonId::L3 => "l3",
            ButtonId::R3 => "r3",
            ButtonId::Share => "share",
            ButtonId::Options => "options",
            ButtonId::Ps => "ps",
            ButtonId::TouchpadClick => "touchpad_click",
            ButtonId::DpadUp => "dpad_up",
            ButtonId::DpadDown => "dpad_down",
            ButtonId::DpadLeft => "dpad_left",
            ButtonId::DpadRight => "dpad_right",
        }
    }

    pub fn from_str(s: &str) -> Option<ButtonId> {
        Self::ALL.iter().copied().find(|b| b.as_str() == s)
    }
}

impl fmt::Display for ButtonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StickId {
    Left,
    Right,
}

impl StickId {
    pub fn as_str(&self) -> &'static str {
        match self {
            StickId::Left => "left",
            StickId::Right => "right",
        }
    }

    pub fn from_str(s: &str) -> Option<StickId> {
        match s {
            "left" => Some(StickId::Left),
            "right" => Some(StickId::Right),
            _ => None,
        }
    }
}

/// A stick position, normalized to [-1.0, 1.0] on each axis with the raw
/// 0x80 center already subtracted out. Deadzone is applied by consumers
/// (the binding engine), not the HID parser.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StickPos {
    pub x: f32,
    pub y: f32,
}

/// Fully decoded state of one controller input report. Produced by
/// padshell-hid parsers, consumed by padshell-binding.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ControllerState {
    pub left_stick: StickPos,
    pub right_stick: StickPos,
    /// Analog trigger values, normalized to [0.0, 1.0].
    pub l2_analog: f32,
    pub r2_analog: f32,
    pub buttons: ButtonSet,
}

/// Bitset over ButtonId, small enough to be Copy.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonSet(u32);

impl fmt::Debug for ButtonSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pressed: Vec<_> = ButtonId::ALL
            .iter()
            .copied()
            .filter(|&b| self.is_pressed(b))
            .map(|b| b.as_str())
            .collect();
        write!(f, "ButtonSet{pressed:?}")
    }
}

impl ButtonSet {
    pub fn empty() -> Self {
        ButtonSet(0)
    }

    fn bit(id: ButtonId) -> u32 {
        1 << (id as u8 as u32)
    }

    pub fn set(&mut self, id: ButtonId, pressed: bool) {
        if pressed {
            self.0 |= Self::bit(id);
        } else {
            self.0 &= !Self::bit(id);
        }
    }

    pub fn is_pressed(&self, id: ButtonId) -> bool {
        self.0 & Self::bit(id) != 0
    }

    /// Buttons pressed in `self` but not in `prev` (i.e. rising edges).
    pub fn pressed_since(self, prev: ButtonSet) -> impl Iterator<Item = ButtonId> {
        ButtonId::ALL
            .iter()
            .copied()
            .filter(move |&b| self.is_pressed(b) && !prev.is_pressed(b))
    }

    /// Buttons released since `prev` (falling edges).
    pub fn released_since(self, prev: ButtonSet) -> impl Iterator<Item = ButtonId> {
        ButtonId::ALL
            .iter()
            .copied()
            .filter(move |&b| !self.is_pressed(b) && prev.is_pressed(b))
    }
}

/// Events the binding engine emits and the output crate consumes to drive
/// the virtual keyboard/mouse. Lives in `core` (rather than `output`) so
/// `binding` can produce these without depending on `output`.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputEvent {
    KeyDown(String),
    KeyUp(String),
    /// Convenience for chord/macro steps: press all keys, then release
    /// them in reverse order.
    KeyTap(Vec<String>),
    MouseButtonDown(MouseButton),
    MouseButtonUp(MouseButton),
    MouseMove { dx: f32, dy: f32 },
    Scroll { dx: f32, dy: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn from_str(s: &str) -> Option<MouseButton> {
        match s {
            "left" => Some(MouseButton::Left),
            "right" => Some(MouseButton::Right),
            "middle" => Some(MouseButton::Middle),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_round_trips_through_string() {
        for &b in ButtonId::ALL {
            assert_eq!(ButtonId::from_str(b.as_str()), Some(b));
        }
    }

    #[test]
    fn button_set_edge_detection() {
        let mut prev = ButtonSet::empty();
        let mut cur = ButtonSet::empty();
        cur.set(ButtonId::Cross, true);

        let pressed: Vec<_> = cur.pressed_since(prev).collect();
        assert_eq!(pressed, vec![ButtonId::Cross]);

        prev = cur;
        cur.set(ButtonId::Cross, false);
        let released: Vec<_> = cur.released_since(prev).collect();
        assert_eq!(released, vec![ButtonId::Cross]);
    }
}
