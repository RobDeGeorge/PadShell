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

#[derive(Debug, Clone)]
pub struct HidrawInfo {
    pub path: PathBuf,
    pub vendor_id: u16,
    pub product_id: u16,
}

/// List every /dev/hidraw* device along with its USB vendor/product ID,
/// for `padshell devices` and for matching against known controllers.
pub fn enumerate() -> Result<Vec<HidrawInfo>, HidError> {
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_subsystem("hidraw")?;

    let mut out = Vec::new();
    for device in enumerator.scan_devices()? {
        let Some(devnode) = device.devnode() else {
            continue;
        };

        // Vendor/product live on the ancestor "usb_device" node, not the
        // hidraw node itself.
        let Some(usb_device) = device.parent_with_subsystem_devtype("usb", "usb_device")?
        else {
            continue;
        };

        let vendor_id = usb_device
            .attribute_value("idVendor")
            .and_then(|v| v.to_str())
            .and_then(|v| u16::from_str_radix(v, 16).ok());
        let product_id = usb_device
            .attribute_value("idProduct")
            .and_then(|v| v.to_str())
            .and_then(|v| u16::from_str_radix(v, 16).ok());

        if let (Some(vendor_id), Some(product_id)) = (vendor_id, product_id) {
            out.push(HidrawInfo {
                path: devnode.to_path_buf(),
                vendor_id,
                product_id,
            });
        }
    }

    Ok(out)
}

/// Find the first hidraw device matching a given vendor/product ID.
pub fn find(vendor_id: u16, product_id: u16) -> Result<HidrawInfo, HidError> {
    enumerate()?
        .into_iter()
        .find(|d| d.vendor_id == vendor_id && d.product_id == product_id)
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
