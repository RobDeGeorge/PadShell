use padshell_core::ControllerState;

/// Implemented once per controller model/connection-type combination
/// (e.g. DS4-over-USB, DS4-over-Bluetooth, eventually Xbox-over-USB).
/// Kept separate from device I/O so it can be unit tested against
/// synthetic byte fixtures without touching /dev/hidraw*.
pub trait ControllerReportParser {
    /// Decode one raw HID input report into a ControllerState.
    /// Returns None for reports that are too short/malformed to trust
    /// rather than panicking - hardware behavior here is unverified
    /// until tested against a real device.
    fn parse(&self, raw: &[u8]) -> Option<ControllerState>;

    /// Expected report length in bytes, for callers sizing their read buffer.
    fn report_len(&self) -> usize;
}
