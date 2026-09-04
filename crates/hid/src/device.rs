//! hidraw device enumeration and raw I/O.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HidError {
    #[error("udev enumeration failed: {0}")]
    Udev(#[from] std::io::Error),
    #[error("no matching hidraw device found")]
    NotFound,
}

/// The transport a HID device is connected over - determines which
/// report parser applies, since DS4 (and most controllers) use a
/// different report layout per transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    Usb,
    Bluetooth,
    Other(u16),
}

impl BusType {
    /// Linux HID bus type constants (linux/hid.h): BUS_USB = 0x03, BUS_BLUETOOTH = 0x05.
    fn from_code(code: u16) -> BusType {
        match code {
            0x03 => BusType::Usb,
            0x05 => BusType::Bluetooth,
            other => BusType::Other(other),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HidrawInfo {
    pub path: PathBuf,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus: BusType,
    /// Sysfs path of the parent HID device (e.g.
    /// `/sys/bus/hid/devices/0005:054C:09CC.0016`) - used to locate the
    /// device's lightbar LEDs and force-feedback evdev node, which live
    /// under this same HID device rather than the hidraw node itself.
    pub hid_syspath: PathBuf,
    /// The HID `uniq` attribute - a real per-device serial when the
    /// hardware/transport provides one. For Bluetooth this is the
    /// controller's MAC address (stable across reconnects). USB DS4s
    /// don't expose one, so this is `None` there.
    pub uniq: Option<String>,
    /// Sysname of the physical USB device ancestor (e.g. `"5-2.2"`),
    /// stable for as long as the controller stays in the same USB port.
    /// Used as a per-device-identity fallback when `uniq` is unavailable
    /// (i.e. USB, where there's no hardware serial to key off of).
    pub port_path: Option<String>,
}

impl HidrawInfo {
    /// A stable identity for this controller, used to key its saved
    /// profile/settings across disconnects and reconnects. Prefers the
    /// hardware `uniq` (a real per-controller serial, e.g. a Bluetooth
    /// MAC address); falls back to the physical USB port path when no
    /// serial is available, so two identical controllers plugged into
    /// different ports still get distinct, stable profiles; falls back
    /// to a bare vendor/product id (shared by any matching device) only
    /// when neither is available.
    pub fn device_id(&self) -> String {
        let suffix = self
            .uniq
            .as_deref()
            .map(|u| format!("uniq-{u}"))
            .or_else(|| self.port_path.as_deref().map(|p| format!("port-{p}")))
            .unwrap_or_else(|| "default".to_string());
        let id = format!("{:04x}{:04x}-{suffix}", self.vendor_id, self.product_id);
        id.chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
            .collect()
    }
}

/// List every /dev/hidraw* device along with its HID vendor/product ID
/// and transport, for `padshell devices` and for matching against known
/// controllers. Works for both USB and Bluetooth HID devices: both
/// expose a "hid" subsystem ancestor whose sysname is
/// "<bus>:<vendor>:<product>.<instance>" (e.g. "0005:054C:09CC.0016"
/// for a Bluetooth DS4), which is more reliable than walking to a
/// USB-specific ancestor that Bluetooth devices don't have.
pub fn enumerate() -> Result<Vec<HidrawInfo>, HidError> {
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_subsystem("hidraw")?;

    let mut out = Vec::new();
    for device in enumerator.scan_devices()? {
        let Some(devnode) = device.devnode() else {
            continue;
        };
        let Some(hid_parent) = device.parent_with_subsystem("hid")? else {
            continue;
        };
        let Some(sysname) = hid_parent.sysname().to_str() else {
            continue;
        };
        let Some(ids) = parse_hid_sysname(sysname) else {
            continue;
        };

        let uniq = hid_parent
            .attribute_value("uniq")
            .and_then(|v| v.to_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let port_path = device
            .parent_with_subsystem("usb")
            .ok()
            .flatten()
            .and_then(|usb| usb.sysname().to_str().map(str::to_string));

        out.push(HidrawInfo {
            path: devnode.to_path_buf(),
            vendor_id: ids.0,
            product_id: ids.1,
            bus: BusType::from_code(ids.2),
            hid_syspath: hid_parent.syspath().to_path_buf(),
            uniq,
            port_path,
        });
    }

    Ok(out)
}

/// Parses "<bus>:<vendor>:<product>.<instance>" into (vendor, product, bus),
/// all as raw hex values from the sysname.
fn parse_hid_sysname(sysname: &str) -> Option<(u16, u16, u16)> {
    let mut parts = sysname.split(':');
    let bus = u16::from_str_radix(parts.next()?, 16).ok()?;
    let vendor = u16::from_str_radix(parts.next()?, 16).ok()?;
    let product_and_instance = parts.next()?;
    let product = u16::from_str_radix(product_and_instance.split('.').next()?, 16).ok()?;
    Some((vendor, product, bus))
}

/// Find the first hidraw device matching a given vendor/product ID,
/// preferring one already connected (both USB and Bluetooth may be
/// enumerable even when only one is actually active, so this returns
/// every match and lets the caller pick).
pub fn find_all(vendor_id: u16, product_id: u16) -> Result<Vec<HidrawInfo>, HidError> {
    Ok(enumerate()?
        .into_iter()
        .filter(|d| d.vendor_id == vendor_id && d.product_id == product_id)
        .collect())
}

pub fn find(vendor_id: u16, product_id: u16) -> Result<HidrawInfo, HidError> {
    find_all(vendor_id, product_id)?
        .into_iter()
        .next()
        .ok_or(HidError::NotFound)
}

/// A blocking-read handle on an open hidraw device.
pub struct HidrawDevice {
    file: File,
}

impl HidrawDevice {
    pub fn open(path: &std::path::Path) -> Result<Self, HidError> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(HidrawDevice { file })
    }

    /// Blocking read of one input report into `buf`, returning the number
    /// of bytes read.
    pub fn read_report(&mut self, buf: &mut [u8]) -> Result<usize, HidError> {
        Ok(self.file.read(buf)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bluetooth_sysname() {
        let (vendor, product, bus) = parse_hid_sysname("0005:054C:09CC.0016").unwrap();
        assert_eq!(vendor, 0x054c);
        assert_eq!(product, 0x09cc);
        assert_eq!(BusType::from_code(bus), BusType::Bluetooth);
    }

    fn info(uniq: Option<&str>, port_path: Option<&str>) -> HidrawInfo {
        HidrawInfo {
            path: PathBuf::from("/dev/hidraw0"),
            vendor_id: 0x054c,
            product_id: 0x05c4,
            bus: BusType::Usb,
            hid_syspath: PathBuf::from("/sys/bus/hid/devices/0003:054C:05C4.0002"),
            uniq: uniq.map(str::to_string),
            port_path: port_path.map(str::to_string),
        }
    }

    #[test]
    fn device_id_prefers_uniq_over_port_path() {
        let id = info(Some("AA:BB:CC:DD:EE:FF"), Some("5-2.2")).device_id();
        assert_eq!(id, "054c05c4-uniq-AA_BB_CC_DD_EE_FF");
    }

    #[test]
    fn device_id_falls_back_to_port_path() {
        let id = info(None, Some("5-2.2")).device_id();
        assert_eq!(id, "054c05c4-port-5-2_2");
    }

    #[test]
    fn device_id_falls_back_to_default_when_no_identity() {
        let id = info(None, None).device_id();
        assert_eq!(id, "054c05c4-default");
    }

    #[test]
    fn parses_usb_sysname() {
        let (vendor, product, bus) = parse_hid_sysname("0003:054C:05C4.0002").unwrap();
        assert_eq!(vendor, 0x054c);
        assert_eq!(product, 0x05c4);
        assert_eq!(BusType::from_code(bus), BusType::Usb);
    }
}
