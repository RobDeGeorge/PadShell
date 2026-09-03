//! DS4 Bluetooth input report parser - stub for v1.
//!
//! Bluetooth reports use report ID 0x11, add a small header and a CRC32
//! trailer, and shift the button/stick offsets versus the USB layout in
//! `usb.rs`. Deferred until USB support is verified against real
//! hardware and Bluetooth becomes a priority.

use crate::report::ControllerReportParser;
use padshell_core::ControllerState;

pub struct Ds4BtParser;

impl Ds4BtParser {
    pub fn new() -> Self {
        Ds4BtParser
    }
}

impl Default for Ds4BtParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerReportParser for Ds4BtParser {
    fn parse(&self, _raw: &[u8]) -> Option<ControllerState> {
        // Not implemented yet - see module docs.
        None
    }

    fn report_len(&self) -> usize {
        547
    }
}
