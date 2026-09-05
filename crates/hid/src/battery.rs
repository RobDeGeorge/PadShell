//! Controller battery level, read from the kernel's power_supply class.
//! `hid-playstation` registers a `ps-controller-battery-<mac>` supply
//! under the HID device's sysfs node for both USB and Bluetooth, so we
//! read `capacity`/`status` from there rather than decoding the battery
//! nibble in the HID report (coarser, and this matches what the rest of
//! the desktop shows).

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryStatus {
    /// 0..=100.
    pub percent: u8,
    /// True when charging or full on cable power.
    pub charging: bool,
}

/// Reads the battery for a HID device given its sysfs path (e.g.
/// `/sys/bus/hid/devices/0005:054C:09CC.0015`). `None` if the kernel
/// exposes no power supply for it.
pub fn read(hid_syspath: &Path) -> Option<BatteryStatus> {
    let supplies = fs::read_dir(hid_syspath.join("power_supply")).ok()?;
    for entry in supplies.filter_map(Result::ok) {
        let dir = entry.path();
        let Some(percent) = read_trimmed(&dir.join("capacity")).and_then(|s| s.parse::<u8>().ok()) else {
            continue;
        };
        let status = read_trimmed(&dir.join("status")).unwrap_or_default();
        let charging = matches!(status.as_str(), "Charging" | "Full");
        return Some(BatteryStatus { percent: percent.min(100), charging });
    }
    None
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}
