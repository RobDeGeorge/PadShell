//! DS4 Bluetooth input report (report ID 0x11) parser.
//!
//! Layout VERIFIED against a real DualShock 4 (CUH-ZCT2x) connected over
//! Bluetooth on 2026-09-03: 78-byte reports, identical button/stick/
//! trigger layout to the USB report in `usb.rs` but shifted +2 bytes for
//! a 2-byte BT header, plus a trailing CRC32 we don't need to parse:
//!
//!   0      report ID (0x11)
//!   1      BT header flags (observed constant 0xc0 - poll-rate related,
//!          not parsed)
//!   2      unused/sequence in observed captures (constant 0x00)
//!   3      left stick X    (same 0x80-centered encoding as USB)
//!   4      left stick Y
//!   5      right stick X
//!   6      right stick Y
//!   7      dpad hat + face buttons (same bit layout as USB offset 5)
//!   8      shoulder/L2/R2-digital/share/options/L3/R3 (same as USB offset 6)
//!   9      PS/touchpad-click/report-counter (same as USB offset 7) -
//!          counter observed incrementing by 4 per report in captures
//!   10     L2 analog
//!   11     R2 analog
//!   12-13  timestamp (not parsed in v1)
//!   ...    gyro/accel/touchpad/battery, then a CRC32 trailer - out of
//!          v1 scope (buttons/sticks/triggers only, matching usb.rs)

use crate::report::ControllerReportParser;
use padshell_core::{ButtonId, ButtonSet, ControllerState, StickPos};

const MIN_REPORT_LEN: usize = 12;
const REPORT_ID_INPUT: u8 = 0x11;
/// Offset of the byte layout in usb.rs, relative to this report's layout.
const BT_OFFSET: usize = 2;

pub struct Ds4BtParser;

impl Ds4BtParser {
    pub fn new() -> Self {
        Ds4BtParser
    }

    fn stick_axis(raw: u8) -> f32 {
        (raw as f32 - 128.0) / 127.0
    }

    fn trigger_axis(raw: u8) -> f32 {
        raw as f32 / 255.0
    }
}

impl Default for Ds4BtParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerReportParser for Ds4BtParser {
    fn parse(&self, raw: &[u8]) -> Option<ControllerState> {
        if raw.len() < MIN_REPORT_LEN {
            return None;
        }
        if raw[0] != REPORT_ID_INPUT {
            return None;
        }

        // Same field offsets as usb.rs, shifted by BT_OFFSET.
        let left_stick = StickPos {
            x: Self::stick_axis(raw[1 + BT_OFFSET]),
            y: Self::stick_axis(raw[2 + BT_OFFSET]),
        };
        let right_stick = StickPos {
            x: Self::stick_axis(raw[3 + BT_OFFSET]),
            y: Self::stick_axis(raw[4 + BT_OFFSET]),
        };

        let byte5 = raw[5 + BT_OFFSET];
        let byte6 = raw[6 + BT_OFFSET];
        let byte7 = raw[7 + BT_OFFSET];

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
            l2_analog: Self::trigger_axis(raw[8 + BT_OFFSET]),
            r2_analog: Self::trigger_axis(raw[9 + BT_OFFSET]),
            buttons,
        })
    }

    fn report_len(&self) -> usize {
        78
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real idle report captured from a CUH-ZCT2x DS4 over Bluetooth
    /// (2026-09-03), truncated to the bytes this parser reads.
    fn captured_idle_report() -> Vec<u8> {
        let hex = "11c00083 7a86 7d08 00b0 0000 b86e 1204 0004 0015 00fc 0158 1e6d 0a00 0000 0006 0000 01f0 83c1 d527 8000 0000 8000 0000 8000 0000 8000 0000 8000 0000 8000 0000 8000 0000 003a 2342 9c";
        hex.split_whitespace()
            .flat_map(|chunk| (0..chunk.len()).step_by(2).map(move |i| &chunk[i..i + 2]))
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }

    #[test]
    fn rejects_short_report() {
        let parser = Ds4BtParser::new();
        assert!(parser.parse(&[REPORT_ID_INPUT, 0xc0]).is_none());
    }

    #[test]
    fn rejects_wrong_report_id() {
        let parser = Ds4BtParser::new();
        let mut report = captured_idle_report();
        report[0] = 0x01;
        assert!(parser.parse(&report).is_none());
    }

    #[test]
    fn parses_captured_idle_report() {
        let parser = Ds4BtParser::new();
        let state = parser.parse(&captured_idle_report()).unwrap();
        // Sticks near-centered (real hardware isn't perfectly at 0.0).
        assert!(state.left_stick.x.abs() < 0.1);
        assert!(state.left_stick.y.abs() < 0.1);
        assert!(state.right_stick.x.abs() < 0.1);
        assert!(state.right_stick.y.abs() < 0.1);
        assert_eq!(state.l2_analog, 0.0);
        assert_eq!(state.r2_analog, 0.0);
        for &b in ButtonId::ALL {
            assert!(!state.buttons.is_pressed(b), "{b} should not be pressed");
        }
    }

    #[test]
    fn parses_cross_pressed() {
        let parser = Ds4BtParser::new();
        let mut report = captured_idle_report();
        report[5 + BT_OFFSET] |= 0b0010_0000;
        let state = parser.parse(&report).unwrap();
        assert!(state.buttons.is_pressed(ButtonId::Cross));
    }
}
