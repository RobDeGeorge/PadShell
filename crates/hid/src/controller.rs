//! One enumeration over every controller padshell can drive, whatever
//! transport it's read through. The daemon and GUI go through this
//! rather than the DS4-specific hidraw lookup.

use std::path::{Path, PathBuf};

use padshell_core::ControllerKind;

use crate::device::{self, BusType, HidError, HidrawDevice, HidrawInfo};
use crate::ds4::{Ds4BtParser, Ds4UsbParser, DS4_PRODUCT_ID_V1, DS4_PRODUCT_ID_V2, DS4_VENDOR_ID};
use crate::evdev_pad::{self, EvdevPadInfo, EvdevSource};
use crate::source::{ControllerSource, HidrawSource};
use crate::HapticsController;

const DS4_IDS: &[(u16, u16)] = &[(DS4_VENDOR_ID, DS4_PRODUCT_ID_V1), (DS4_VENDOR_ID, DS4_PRODUCT_ID_V2)];

/// How the controller's input reaches us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputPath {
    /// Raw HID reports (DS4): parsed by a `ControllerReportParser`.
    Hidraw(PathBuf),
    /// Kernel input events (Xbox and anything else with a gamepad driver).
    Evdev(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ControllerInfo {
    pub kind: ControllerKind,
    /// Human-readable device name (kernel-reported for evdev pads).
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus: BusType,
    pub input: InputPath,
    /// Sysfs directory to look under for `leds/` and `power_supply/`.
    pub syspath: PathBuf,
    /// Force-feedback node, when known up front (evdev pads: the same
    /// node we read from). DS4s discover theirs via their LEDs.
    pub ff_evdev_path: Option<PathBuf>,
    pub uniq: Option<String>,
    pub port_path: Option<String>,
}

impl ControllerInfo {
    fn from_hidraw(info: HidrawInfo) -> Self {
        ControllerInfo {
            kind: ControllerKind::Ds4,
            name: "DualShock 4".to_string(),
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            bus: info.bus,
            input: InputPath::Hidraw(info.path),
            syspath: info.hid_syspath,
            ff_evdev_path: None,
            uniq: info.uniq,
            port_path: info.port_path,
        }
    }

    fn from_evdev(info: EvdevPadInfo) -> Self {
        ControllerInfo {
            kind: info.kind(),
            name: info.name,
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            bus: info.bus,
            input: InputPath::Evdev(info.path.clone()),
            syspath: info.parent_syspath,
            ff_evdev_path: Some(info.path),
            uniq: info.uniq,
            port_path: info.port_path,
        }
    }

    /// Stable identity keying this controller's profile; same scheme as
    /// `HidrawInfo::device_id` (hardware serial, else USB port, else the
    /// bare vendor/product). An Xbox pad over Bluetooth has its MAC as
    /// serial; over USB `xpad` exposes none, so it's keyed by port.
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

    pub fn input_path(&self) -> &Path {
        match &self.input {
            InputPath::Hidraw(p) | InputPath::Evdev(p) => p,
        }
    }

    /// Opens the controller for reading. Never grabs the device, so
    /// other readers (RetroArch, Steam, SDL games) keep working.
    pub fn open(&self) -> Result<Box<dyn ControllerSource>, HidError> {
        match &self.input {
            InputPath::Hidraw(path) => {
                let parser: Box<dyn crate::ControllerReportParser + Send> = match self.bus {
                    BusType::Bluetooth => Box::new(Ds4BtParser::new()),
                    _ => Box::new(Ds4UsbParser::new()),
                };
                Ok(Box::new(HidrawSource::new(HidrawDevice::open(path)?, parser)))
            }
            InputPath::Evdev(path) => Ok(Box::new(EvdevSource::open(path, self.kind)?)),
        }
    }

    /// Lightbar (if any) and rumble for this controller.
    pub fn haptics(&self) -> HapticsController {
        HapticsController::discover(&self.syspath).with_evdev_fallback(self.ff_evdev_path.clone())
    }

    pub fn battery(&self) -> Option<crate::BatteryStatus> {
        crate::battery::read(&self.syspath)
    }
}

/// Every connected controller: DS4s via hidraw (the parser that also
/// gets us touchpad/gyro later), everything else via evdev.
pub fn enumerate() -> Vec<ControllerInfo> {
    let mut out: Vec<ControllerInfo> = DS4_IDS
        .iter()
        .flat_map(|&(v, p)| device::find_all(v, p).unwrap_or_default())
        .map(ControllerInfo::from_hidraw)
        .collect();
    match evdev_pad::enumerate(DS4_IDS) {
        Ok(pads) => out.extend(pads.into_iter().map(ControllerInfo::from_evdev)),
        Err(err) => tracing::debug!(error = %err, "evdev gamepad enumeration failed"),
    }
    out
}

pub fn first() -> Option<ControllerInfo> {
    enumerate().into_iter().next()
}
