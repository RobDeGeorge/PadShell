//! Gamepads read through the kernel's evdev interface (`/dev/input/eventN`).
//!
//! This is how Xbox controllers reach userspace on Linux: over USB the
//! `xpad` driver owns the device and exposes *no* hidraw node at all, and
//! over Bluetooth `hid-microsoft` / `xpadneo` do expose hidraw but their
//! evdev mapping is the well-trodden path every game uses. Reading evdev
//! without `EVIOCGRAB` leaves the node fully shared, so RetroArch (udev
//! joypad driver), SDL games and Steam keep seeing the controller exactly
//! as before - padshell just listens alongside them.
//!
//! Also serves as the fallback for any other gamepad the kernel knows
//! how to drive (8BitDo, Logitech, Stadia, ...).

use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

use evdev::{AbsoluteAxisCode, Device, EventSummary, KeyCode};
use padshell_core::{ButtonId, ControllerKind, ControllerState};

use crate::device::{BusType, HidError};
use crate::source::{wait_readable, ControllerSource};

/// Vendor IDs whose pads we identify as Xbox-layout regardless of the
/// kernel-reported name.
const MICROSOFT_VENDOR_ID: u16 = 0x045e;
/// padshell's own uinput keyboard/mouse - never a controller.
const PADSHELL_VIRTUAL_VENDOR_ID: u16 = 0x1209;
/// Steam Input's virtual "X-Box 360 pad": a uinput mirror of whatever
/// physical pad Steam has open. Reading it as well would fire every
/// binding a second time. (Valve's real hardware uses other product IDs.)
const STEAM_VIRTUAL_PAD: (u16, u16) = (0x28de, 0x11ff);
/// ABS_MAX + 1 (linux/input-event-codes.h) - the size of the absinfo table.
const ABS_COUNT: usize = 0x40;

#[derive(Debug, Clone)]
pub struct EvdevPadInfo {
    pub path: PathBuf,
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus: BusType,
    /// Sysfs path of the device that owns this input node (the HID
    /// device for Bluetooth pads, the USB interface for `xpad`) - where
    /// a `power_supply/` dir lives when the driver reports battery.
    pub parent_syspath: PathBuf,
    pub uniq: Option<String>,
    pub port_path: Option<String>,
}

impl EvdevPadInfo {
    pub fn kind(&self) -> ControllerKind {
        let name = self.name.to_ascii_lowercase();
        if self.vendor_id == MICROSOFT_VENDOR_ID || name.contains("xbox") || name.contains("x-box") {
            ControllerKind::Xbox
        } else {
            ControllerKind::Generic
        }
    }
}

/// Every evdev node that looks like a gamepad (has a south face button
/// and an X axis), excluding padshell's own virtual devices, Steam's
/// virtual pad, and anything
/// in `skip_vendor_products` (used to leave DS4s to the hidraw path so
/// one controller isn't opened twice).
pub fn enumerate(skip_vendor_products: &[(u16, u16)]) -> Result<Vec<EvdevPadInfo>, HidError> {
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_subsystem("input")?;
    enumerator.match_property("ID_INPUT_JOYSTICK", "1")?;

    let mut out = Vec::new();
    for udev_dev in enumerator.scan_devices()? {
        let Some(devnode) = udev_dev.devnode() else { continue };
        if !devnode.to_string_lossy().contains("/event") {
            continue;
        }
        let Ok(device) = Device::open(devnode) else { continue };
        if !looks_like_gamepad(&device) {
            continue;
        }
        let id = device.input_id();
        if id.vendor() == PADSHELL_VIRTUAL_VENDOR_ID || (id.vendor(), id.product()) == STEAM_VIRTUAL_PAD {
            continue;
        }
        if skip_vendor_products.contains(&(id.vendor(), id.product())) {
            continue;
        }

        // udev's parent is the inputN class device; its parent is the
        // driver-level device (HID or USB interface) we key battery off.
        let parent_syspath = udev_dev
            .parent()
            .and_then(|input_n| input_n.parent())
            .map(|p| p.syspath().to_path_buf())
            .unwrap_or_else(|| udev_dev.syspath().to_path_buf());
        let port_path = udev_dev
            .parent_with_subsystem("usb")
            .ok()
            .flatten()
            .and_then(|usb| usb.sysname().to_str().map(str::to_string));

        out.push(EvdevPadInfo {
            path: devnode.to_path_buf(),
            name: device.name().unwrap_or("gamepad").to_string(),
            vendor_id: id.vendor(),
            product_id: id.product(),
            bus: bus_from_code(id.bus_type().0),
            parent_syspath,
            uniq: device.unique_name().map(str::trim).filter(|u| !u.is_empty()).map(str::to_string),
            port_path,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn bus_from_code(code: u16) -> BusType {
    match code {
        0x03 => BusType::Usb,
        0x05 => BusType::Bluetooth,
        other => BusType::Other(other),
    }
}

fn looks_like_gamepad(device: &Device) -> bool {
    let has_south = device.supported_keys().is_some_and(|k| k.contains(KeyCode::BTN_SOUTH));
    let has_x = device.supported_absolute_axes().is_some_and(|a| a.contains(AbsoluteAxisCode::ABS_X));
    has_south && has_x
}

/// Per-axis calibration pulled from the kernel's absinfo, so one
/// decoder serves pads reporting -32768..32767 (xpad), 0..65535
/// (Bluetooth HID) or 0..255 (triggers) alike.
#[derive(Debug, Clone, Copy, Default)]
struct AxisRange {
    min: i32,
    max: i32,
}

impl AxisRange {
    /// Centered axis -> [-1, 1].
    fn centered(&self, value: i32) -> f32 {
        let span = (self.max - self.min) as f32;
        if span <= 0.0 {
            return 0.0;
        }
        let mid = (self.max as f32 + self.min as f32) / 2.0;
        ((value as f32 - mid) / (span / 2.0)).clamp(-1.0, 1.0)
    }

    /// One-sided axis (trigger) -> [0, 1].
    fn unipolar(&self, value: i32) -> f32 {
        let span = (self.max - self.min) as f32;
        if span <= 0.0 {
            return 0.0;
        }
        ((value - self.min) as f32 / span).clamp(0.0, 1.0)
    }
}

pub struct EvdevSource {
    device: Device,
    kind: ControllerKind,
    state: ControllerState,
    ranges: [AxisRange; ABS_COUNT],
    /// True once at least one event arrived since the last SYN_REPORT,
    /// so idle sync packets don't re-emit an unchanged state.
    dirty: bool,
}

impl EvdevSource {
    pub fn open(path: &Path, kind: ControllerKind) -> Result<Self, HidError> {
        let device = Device::open(path)?;
        let mut ranges = [AxisRange::default(); ABS_COUNT];
        if let Ok(abs) = device.get_abs_state() {
            for (i, info) in abs.iter().enumerate().take(ABS_COUNT) {
                ranges[i] = AxisRange { min: info.minimum, max: info.maximum };
            }
        }
        let mut src = EvdevSource { device, kind, state: ControllerState::default(), ranges, dirty: false };
        src.seed_from_current();
        Ok(src)
    }

    /// Initialize sticks/triggers from where the axes currently sit, so
    /// a trigger already half-pulled when the daemon starts isn't read
    /// as released until it moves.
    fn seed_from_current(&mut self) {
        if let Ok(abs) = self.device.get_abs_state() {
            for (i, info) in abs.iter().enumerate().take(ABS_COUNT) {
                self.apply_axis(AbsoluteAxisCode(i as u16), info.value);
            }
        }
        if let Ok(keys) = self.device.get_key_state() {
            for key in keys.iter() {
                self.apply_key(key, true);
            }
        }
    }

    fn apply_axis(&mut self, axis: AbsoluteAxisCode, value: i32) {
        let range = self.ranges.get(axis.0 as usize).copied().unwrap_or_default();
        match axis {
            AbsoluteAxisCode::ABS_X => self.state.left_stick.x = range.centered(value),
            AbsoluteAxisCode::ABS_Y => self.state.left_stick.y = range.centered(value),
            AbsoluteAxisCode::ABS_RX => self.state.right_stick.x = range.centered(value),
            AbsoluteAxisCode::ABS_RY => self.state.right_stick.y = range.centered(value),
            // xpad / hid-playstation put triggers on Z/RZ; the Bluetooth
            // HID gamepad descriptor (hid-microsoft) uses BRAKE/GAS.
            AbsoluteAxisCode::ABS_Z | AbsoluteAxisCode::ABS_BRAKE => {
                self.state.l2_analog = range.unipolar(value);
                self.state.buttons.set(ButtonId::L2, self.state.l2_analog > 0.5);
            }
            AbsoluteAxisCode::ABS_RZ | AbsoluteAxisCode::ABS_GAS => {
                self.state.r2_analog = range.unipolar(value);
                self.state.buttons.set(ButtonId::R2, self.state.r2_analog > 0.5);
            }
            AbsoluteAxisCode::ABS_HAT0X => {
                self.state.buttons.set(ButtonId::DpadLeft, value < 0);
                self.state.buttons.set(ButtonId::DpadRight, value > 0);
            }
            AbsoluteAxisCode::ABS_HAT0Y => {
                self.state.buttons.set(ButtonId::DpadUp, value < 0);
                self.state.buttons.set(ButtonId::DpadDown, value > 0);
            }
            _ => {}
        }
    }

    fn apply_key(&mut self, key: KeyCode, pressed: bool) {
        if let Some(id) = button_for_key(key, self.kind) {
            self.state.buttons.set(id, pressed);
        }
    }
}

/// Maps an evdev button to the shared vocabulary.
///
/// The face-button quirk: `BTN_NORTH == BTN_X` and `BTN_WEST == BTN_Y`
/// in the kernel's codes. `xpad`/`xpadneo`/`hid-microsoft` report the
/// Xbox **X** button (left) as BTN_X and **Y** (top) as BTN_Y, i.e. by
/// letter, while the gamepad spec (and `hid-playstation`) uses them
/// spatially (NORTH = top). So for Xbox pads NORTH is the *left* button.
fn button_for_key(key: KeyCode, kind: ControllerKind) -> Option<ButtonId> {
    let (north, west) = match kind {
        ControllerKind::Xbox => (ButtonId::Square, ButtonId::Triangle),
        _ => (ButtonId::Triangle, ButtonId::Square),
    };
    Some(match key {
        KeyCode::BTN_SOUTH => ButtonId::Cross,
        KeyCode::BTN_EAST => ButtonId::Circle,
        KeyCode::BTN_NORTH => north,
        KeyCode::BTN_WEST => west,
        KeyCode::BTN_TL => ButtonId::L1,
        KeyCode::BTN_TR => ButtonId::R1,
        KeyCode::BTN_TL2 => ButtonId::L2,
        KeyCode::BTN_TR2 => ButtonId::R2,
        KeyCode::BTN_THUMBL => ButtonId::L3,
        KeyCode::BTN_THUMBR => ButtonId::R3,
        KeyCode::BTN_SELECT => ButtonId::Share,
        KeyCode::BTN_START => ButtonId::Options,
        KeyCode::BTN_MODE => ButtonId::Ps,
        KeyCode::BTN_DPAD_UP => ButtonId::DpadUp,
        KeyCode::BTN_DPAD_DOWN => ButtonId::DpadDown,
        KeyCode::BTN_DPAD_LEFT => ButtonId::DpadLeft,
        KeyCode::BTN_DPAD_RIGHT => ButtonId::DpadRight,
        // Xbox Series Share button (xpad/xpadneo report it as KEY_RECORD);
        // the DS4's touchpad click is the closest spare slot.
        KeyCode::KEY_RECORD => ButtonId::TouchpadClick,
        _ => return None,
    })
}

impl ControllerSource for EvdevSource {
    fn next_state(&mut self, timeout: Duration) -> Result<Option<ControllerState>, HidError> {
        loop {
            if !wait_readable(self.device.as_fd(), timeout)? {
                return Ok(None);
            }
            let events: Vec<_> = self.device.fetch_events()?.collect();
            let mut synced = false;
            for event in events {
                match event.destructure() {
                    EventSummary::Key(_, key, value) => {
                        self.apply_key(key, value != 0);
                        self.dirty = true;
                    }
                    EventSummary::AbsoluteAxis(_, axis, value) => {
                        self.apply_axis(axis, value);
                        self.dirty = true;
                    }
                    EventSummary::Synchronization(..) => synced = true,
                    _ => {}
                }
            }
            if synced && self.dirty {
                self.dirty = false;
                return Ok(Some(self.state.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_ranges_normalize_regardless_of_scale() {
        let signed = AxisRange { min: -32768, max: 32767 };
        assert!((signed.centered(32767) - 1.0).abs() < 0.001);
        assert!((signed.centered(-32768) + 1.0).abs() < 0.001);
        assert!(signed.centered(0).abs() < 0.001);

        let unsigned = AxisRange { min: 0, max: 65535 };
        assert!((unsigned.centered(65535) - 1.0).abs() < 0.001);
        assert!(unsigned.centered(32767).abs() < 0.001);

        let trigger = AxisRange { min: 0, max: 1023 };
        assert_eq!(trigger.unipolar(0), 0.0);
        assert_eq!(trigger.unipolar(1023), 1.0);
    }

    #[test]
    fn xbox_face_buttons_follow_letters_not_position() {
        assert_eq!(button_for_key(KeyCode::BTN_NORTH, ControllerKind::Xbox), Some(ButtonId::Square));
        assert_eq!(button_for_key(KeyCode::BTN_WEST, ControllerKind::Xbox), Some(ButtonId::Triangle));
        assert_eq!(button_for_key(KeyCode::BTN_NORTH, ControllerKind::Generic), Some(ButtonId::Triangle));
        assert_eq!(button_for_key(KeyCode::BTN_MODE, ControllerKind::Xbox), Some(ButtonId::Ps));
    }
}
