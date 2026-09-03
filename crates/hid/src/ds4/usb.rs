//! DS4 USB input report (report ID 0x01) parser.
//!
//! Byte layout below is the standard documented DS4 HID report layout
//! (matches the widely-referenced PS4 reverse-engineering notes and the
//! Linux `hid-sony`/`hid-playstation` driver's understanding of the
//! device). NOT YET VERIFIED against a physical controller on this
//! machine - the parser is bounds-checked defensively rather than
//! assuming exact offsets are correct, and this must be confirmed once
//! a DS4 is connected (see padshell CLI `devices` / `test-report`).
//!
//! Offsets used here (byte 0 = report ID, included since hidraw reads
//! include the report ID for numbered-report devices):
//!   0      report ID (0x01)
//!   1      left stick X   (0x00 left .. 0x80 center .. 0xff right)
//!   2      left stick Y   (0x00 up   .. 0x80 center .. 0xff down)
//!   3      right stick X
//!   4      right stick Y
//!   5      bits0-3 D-pad hat (0-7 = 8 directions clockwise from N, 8 = neutral)
//!          bit4 Square, bit5 Cross, bit6 Circle, bit7 Triangle
//!   6      bit0 L1, bit1 R1, bit2 L2(digital), bit3 R2(digital),
//!          bit4 Share, bit5 Options, bit6 L3, bit7 R3
//!   7      bit0 PS, bit1 Touchpad click, bits2-7 report counter
//!   8      L2 analog (0x00-0xff)
//!   9      R2 analog (0x00-0xff)
//!
//! Gyro/accel/touchpad-position/battery bytes exist further in the report
//! but are out of scope for v1 (buttons/sticks/triggers only).

use crate::report::ControllerReportParser;
use padshell_core::{ButtonId, ButtonSet, ControllerState, StickPos};

/// Minimum length we require before trusting the buttons/sticks/triggers
/// region (offsets 0..=9). The full report is documented as 64 bytes on
/// USB, but we only need the first 10 for v1.
const MIN_REPORT_LEN: usize = 10;
const REPORT_ID_INPUT: u8 = 0x01;

pub struct Ds4UsbParser;

impl Ds4UsbParser {
    pub fn new() -> Self {
        Ds4UsbParser
    }

    fn stick_axis(raw: u8) -> f32 {
        // Center at 0x80, normalize to [-1.0, 1.0]. Deadzone is applied
        // downstream by the binding engine, not here.
        (raw as f32 - 128.0) / 127.0
    }

    fn trigger_axis(raw: u8) -> f32 {
        raw as f32 / 255.0
    }
}

impl Default for Ds4UsbParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerReportParser for Ds4UsbParser {
    fn parse(&self, raw: &[u8]) -> Option<ControllerState> {
        if raw.len() < MIN_REPORT_LEN {
            return None;
        }
        if raw[0] != REPORT_ID_INPUT {
            return None;
        }

        let left_stick = StickPos {
            x: Self::stick_axis(raw[1]),
            y: Self::stick_axis(raw[2]),
        };
        let right_stick = StickPos {
            x: Self::stick_axis(raw[3]),
            y: Self::stick_axis(raw[4]),
        };

        let byte5 = raw[5];
        let byte6 = raw[6];
        let byte7 = raw[7];

        let mut buttons = ButtonSet::empty();
        buttons.set(ButtonId::Square, byte5 & 0b0001_0000 != 0);
        buttons.set(ButtonId::Cross, byte5 & 0b0010_0000 != 0);
        buttons.set(ButtonId::Circle, byte5 & 0b0100_0000 != 0);
        buttons.set(ButtonId::Triangle, byte5 & 0b1000_0000 != 0);

        let hat = byte5 & 0x0f;
        buttons.set(ButtonId::DpadUp, matches!(hat, 0 | 1 | 7));
        buttons.set(ButtonId::DpadRight, matches!(hat, 1 | 2 | 3));
        buttons.set(ButtonId::DpadDown, matches!(hat, 3 | 4 | 5));
        buttons.set(ButtonId::DpadLeft, matches!(hat, 5 | 6 | 7));

        buttons.set(ButtonId::L1, byte6 & 0b0000_0001 != 0);
        buttons.set(ButtonId::R1, byte6 & 0b0000_0010 != 0);
        buttons.set(ButtonId::L2, byte6 & 0b0000_0100 != 0);
        buttons.set(ButtonId::R2, byte6 & 0b0000_1000 != 0);
        buttons.set(ButtonId::Share, byte6 & 0b0001_0000 != 0);
        buttons.set(ButtonId::Options, byte6 & 0b0010_0000 != 0);
        buttons.set(ButtonId::L3, byte6 & 0b0100_0000 != 0);
        buttons.set(ButtonId::R3, byte6 & 0b1000_0000 != 0);

        buttons.set(ButtonId::Ps, byte7 & 0b0000_0001 != 0);
        buttons.set(ButtonId::TouchpadClick, byte7 & 0b0000_0010 != 0);

        Some(ControllerState {
            left_stick,
            right_stick,
            l2_analog: Self::trigger_axis(raw[8]),
            r2_analog: Self::trigger_axis(raw[9]),
            buttons,
        })
    }

    fn report_len(&self) -> usize {
        64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a synthetic 10-byte report (enough for our v1 offsets) with
    /// sticks centered, triggers released, no buttons, dpad neutral.
    fn neutral_report() -> Vec<u8> {
        vec![
            REPORT_ID_INPUT,
            0x80, // left stick X
            0x80, // left stick Y
            0x80, // right stick X
            0x80, // right stick Y
            0x08, // dpad neutral (hat=8), no face buttons
            0x00, // no shoulder/L3/R3/share/options
            0x00, // no PS/touchpad, counter=0
            0x00, // L2 analog
            0x00, // R2 analog
        ]
    }

    #[test]
    fn rejects_short_report() {
        let parser = Ds4UsbParser::new();
        assert!(parser.parse(&[REPORT_ID_INPUT, 0x80]).is_none());
    }

    #[test]
    fn rejects_wrong_report_id() {
        let parser = Ds4UsbParser::new();
        let mut report = neutral_report();
        report[0] = 0x11; // bluetooth report id, not ours
        assert!(parser.parse(&report).is_none());
    }

    #[test]
    fn parses_neutral_state() {
        let parser = Ds4UsbParser::new();
        let state = parser.parse(&neutral_report()).unwrap();
        assert_eq!(state.left_stick, StickPos { x: 0.0, y: 0.0 });
        assert_eq!(state.right_stick, StickPos { x: 0.0, y: 0.0 });
        assert_eq!(state.l2_analog, 0.0);
        assert_eq!(state.r2_analog, 0.0);
        for &b in ButtonId::ALL {
            assert!(!state.buttons.is_pressed(b), "{b} should not be pressed");
        }
    }

    #[test]
    fn parses_cross_pressed() {
        let parser = Ds4UsbParser::new();
        let mut report = neutral_report();
        report[5] |= 0b0010_0000; // Cross bit
        let state = parser.parse(&report).unwrap();
        assert!(state.buttons.is_pressed(ButtonId::Cross));
        assert!(!state.buttons.is_pressed(ButtonId::Circle));
    }

    #[test]
    fn parses_left_stick_full_right() {
        let parser = Ds4UsbParser::new();
        let mut report = neutral_report();
        report[1] = 0xff;
        let state = parser.parse(&report).unwrap();
        assert!((state.left_stick.x - 1.0).abs() < 0.01);
    }

    #[test]
    fn parses_dpad_up() {
        let parser = Ds4UsbParser::new();
        let mut report = neutral_report();
        report[5] = (report[5] & 0xf0) | 0x00; // hat = 0 (N)
        let state = parser.parse(&report).unwrap();
        assert!(state.buttons.is_pressed(ButtonId::DpadUp));
        assert!(!state.buttons.is_pressed(ButtonId::DpadDown));
    }

    #[test]
    fn parses_shoulder_and_trigger_buttons() {
        let parser = Ds4UsbParser::new();
        let mut report = neutral_report();
        report[6] = 0b0000_0011; // L1 + R1
        report[8] = 0xff; // L2 analog full
        let state = parser.parse(&report).unwrap();
        assert!(state.buttons.is_pressed(ButtonId::L1));
        assert!(state.buttons.is_pressed(ButtonId::R1));
        assert_eq!(state.l2_analog, 1.0);
    }
}
