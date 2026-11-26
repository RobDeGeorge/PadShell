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

    /// Parses a canonical name (`cross`, `l1`, ...) or one of the Xbox /
    /// generic aliases (`a`, `lb`, `view`, `guide`, ...). Config files
    /// always store the canonical name; aliases are for hand-edited
    /// configs written in Xbox vocabulary.
    pub fn from_str(s: &str) -> Option<ButtonId> {
        Self::ALL
            .iter()
            .copied()
            .find(|b| b.as_str() == s)
            .or_else(|| Self::from_alias(s))
    }

    fn from_alias(s: &str) -> Option<ButtonId> {
        Some(match s {
            "a" => ButtonId::Cross,
            "b" => ButtonId::Circle,
            "x" => ButtonId::Square,
            "y" => ButtonId::Triangle,
            "lb" => ButtonId::L1,
            "rb" => ButtonId::R1,
            "lt" => ButtonId::L2,
            "rt" => ButtonId::R2,
            "ls" | "left_stick" => ButtonId::L3,
            "rs" | "right_stick" => ButtonId::R3,
            "back" | "view" | "select" => ButtonId::Share,
            "start" | "menu" => ButtonId::Options,
            "guide" | "xbox" | "home" | "mode" => ButtonId::Ps,
            "capture" | "share_xbox" | "extra" => ButtonId::TouchpadClick,
            _ => return None,
        })
    }

    /// Short label for the button as printed on this kind of controller
    /// (what the GUI shows on the diagram and in binding lists).
    pub fn label(&self, kind: ControllerKind) -> &'static str {
        match kind {
            ControllerKind::Ds4 => match self {
                ButtonId::Cross => "✕",
                ButtonId::Circle => "○",
                ButtonId::Square => "◻",
                ButtonId::Triangle => "△",
                ButtonId::L1 => "L1",
                ButtonId::R1 => "R1",
                ButtonId::L2 => "L2",
                ButtonId::R2 => "R2",
                ButtonId::L3 => "L3",
                ButtonId::R3 => "R3",
                ButtonId::Share => "SHARE",
                ButtonId::Options => "OPTIONS",
                ButtonId::Ps => "PS",
                ButtonId::TouchpadClick => "TOUCHPAD",
                ButtonId::DpadUp => "▲",
                ButtonId::DpadDown => "▼",
                ButtonId::DpadLeft => "◀",
                ButtonId::DpadRight => "▶",
            },
            ControllerKind::Xbox | ControllerKind::Generic => match self {
                ButtonId::Cross => "A",
                ButtonId::Circle => "B",
                ButtonId::Square => "X",
                ButtonId::Triangle => "Y",
                ButtonId::L1 => "LB",
                ButtonId::R1 => "RB",
                ButtonId::L2 => "LT",
                ButtonId::R2 => "RT",
                ButtonId::L3 => "LS",
                ButtonId::R3 => "RS",
                ButtonId::Share => if kind == ControllerKind::Xbox { "VIEW" } else { "SELECT" },
                ButtonId::Options => if kind == ControllerKind::Xbox { "MENU" } else { "START" },
                ButtonId::Ps => if kind == ControllerKind::Xbox { "XBOX" } else { "HOME" },
                ButtonId::TouchpadClick => if kind == ControllerKind::Xbox { "SHARE" } else { "EXTRA" },
                ButtonId::DpadUp => "▲",
                ButtonId::DpadDown => "▼",
                ButtonId::DpadLeft => "◀",
                ButtonId::DpadRight => "▶",
            },
        }
    }
}

/// Which family of controller a device is, for choosing button labels
/// in the GUI and which optional features (lightbar, touchpad) exist.
/// Every kind decodes to the same `ButtonId` vocabulary: an Xbox A is
/// `Cross`, View is `Share`, the Guide button is `Ps`, and so on, so
/// one profile format serves every controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerKind {
    Ds4,
    Xbox,
    #[default]
    Generic,
}

impl ControllerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ControllerKind::Ds4 => "ds4",
            ControllerKind::Xbox => "xbox",
            ControllerKind::Generic => "generic",
        }
    }

    pub fn from_str(s: &str) -> Option<ControllerKind> {
        match s {
            "ds4" => Some(ControllerKind::Ds4),
            "xbox" => Some(ControllerKind::Xbox),
            "generic" => Some(ControllerKind::Generic),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ControllerKind::Ds4 => "DualShock 4",
            ControllerKind::Xbox => "Xbox controller",
            ControllerKind::Generic => "Gamepad",
        }
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
    fn xbox_aliases_map_onto_canonical_buttons() {
        assert_eq!(ButtonId::from_str("a"), Some(ButtonId::Cross));
        assert_eq!(ButtonId::from_str("lb"), Some(ButtonId::L1));
        assert_eq!(ButtonId::from_str("view"), Some(ButtonId::Share));
        assert_eq!(ButtonId::from_str("guide"), Some(ButtonId::Ps));
        assert_eq!(ButtonId::from_str("nope"), None);
    }

    #[test]
    fn labels_follow_controller_kind() {
        assert_eq!(ButtonId::Cross.label(ControllerKind::Ds4), "✕");
        assert_eq!(ButtonId::Cross.label(ControllerKind::Xbox), "A");
        assert_eq!(ButtonId::Share.label(ControllerKind::Xbox), "VIEW");
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
