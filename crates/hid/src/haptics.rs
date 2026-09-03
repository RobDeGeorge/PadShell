//! Lightbar color and rumble control via the kernel's own subsystems
//! (LED class + evdev force-feedback) rather than raw HID output
//! reports. On this system `hid-playstation` already claims the DS4's
//! output reports for its own LED/rumble bookkeeping, so writing raw
//! HID output reports directly (report ID 0x05/0x11) is silently
//! ineffective - verified live: it did not change the lightbar, while
//! writing to `/sys/class/leds/inputNN:{red,green,blue}/brightness`
//! did. This module goes through those kernel-native paths instead.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HapticsError {
    #[error("no LED lightbar found for this device")]
    NoLightbar,
    #[error("no force-feedback capable input device found for this device")]
    NoRumble,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct HapticsController {
    led_dir: Option<PathBuf>,
    led_prefix: Option<String>,
    evdev_path: Option<PathBuf>,
}

impl HapticsController {
    /// Discovers the lightbar LEDs and force-feedback evdev node
    /// associated with a HID device, given that device's sysfs path
    /// (e.g. `/sys/bus/hid/devices/0005:054C:09CC.0016`).
    pub fn discover(hid_syspath: &Path) -> Self {
        let led_dir = hid_syspath.join("leds");
        let led_prefix = fs::read_dir(&led_dir)
            .ok()
            .and_then(|entries| {
                entries.filter_map(Result::ok).find_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    name.split_once(':').map(|(prefix, _)| prefix.to_string())
                })
            });

        let evdev_path = led_prefix.as_deref().and_then(find_evdev_node);

        HapticsController {
            led_dir: led_prefix.as_ref().map(|_| led_dir),
            led_prefix,
            evdev_path,
        }
    }

    pub fn has_lightbar(&self) -> bool {
        self.led_dir.is_some()
    }

    pub fn has_rumble(&self) -> bool {
        self.evdev_path.is_some()
    }

    pub fn set_lightbar(&self, r: u8, g: u8, b: u8) -> Result<(), HapticsError> {
        let (Some(dir), Some(prefix)) = (&self.led_dir, &self.led_prefix) else {
            return Err(HapticsError::NoLightbar);
        };
        for (color, value) in [("red", r), ("green", g), ("blue", b)] {
            let path = dir.join(format!("{prefix}:{color}/brightness"));
            fs::write(&path, value.to_string())?;
        }
        Ok(())
    }

    /// Plays a rumble effect for `duration_ms`, blocking for that long
    /// then stopping it. `weak`/`strong` are motor magnitudes (0-65535).
    pub fn rumble(&self, weak: u16, strong: u16, duration_ms: u16) -> Result<(), HapticsError> {
        let path = self.evdev_path.as_ref().ok_or(HapticsError::NoRumble)?;
        let mut device = evdev::Device::open(path)?;

        let data = evdev::FFEffectData {
            direction: 0,
            trigger: evdev::FFTrigger {
                button: 0,
                interval: 0,
            },
            replay: evdev::FFReplay {
                length: duration_ms,
                delay: 0,
            },
            kind: evdev::FFEffectKind::Rumble {
                strong_magnitude: strong,
                weak_magnitude: weak,
            },
        };
        let mut effect = device.upload_ff_effect(data)?;
        effect.play(1)?;
        std::thread::sleep(std::time::Duration::from_millis(duration_ms as u64));
        effect.stop()?;
        Ok(())
    }
}

fn find_evdev_node(input_name: &str) -> Option<PathBuf> {
    let dir = PathBuf::from("/sys/class/input").join(input_name);
    fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("event") {
                Some(PathBuf::from("/dev/input").join(name))
            } else {
                None
            }
        })
}
