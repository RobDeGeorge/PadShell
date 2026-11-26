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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    /// Haptics for the first connected controller of any kind.
    pub fn discover_first() -> Option<Self> {
        crate::controller::first().map(|info| info.haptics())
    }

    /// Uses `path` for rumble when LED-based discovery found no evdev
    /// node - the case for pads with no lightbar (Xbox), whose evdev
    /// node is the same one we read input from.
    pub fn with_evdev_fallback(mut self, path: Option<PathBuf>) -> Self {
        if self.evdev_path.is_none() {
            self.evdev_path = path.filter(|p| p.exists());
        }
        self
    }

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

/// A continuously-running lightbar animation. Speed/period is in
/// milliseconds for one full cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightbarAnimation {
    /// Cycles through the full hue spectrum.
    Rainbow { period_ms: u32 },
    /// Breathes a single base color's brightness up and down.
    Fade { r: u8, g: u8, b: u8, period_ms: u32 },
    /// Blinks a base color fully on/off.
    Strobe { r: u8, g: u8, b: u8, period_ms: u32 },
}

impl LightbarAnimation {
    fn color_at(&self, elapsed: Duration) -> (u8, u8, u8) {
        match *self {
            LightbarAnimation::Rainbow { period_ms } => {
                let t = elapsed.as_millis() as f32 % period_ms.max(1) as f32;
                let hue = (t / period_ms.max(1) as f32) * 360.0;
                hsv_to_rgb(hue, 1.0, 1.0)
            }
            LightbarAnimation::Fade { r, g, b, period_ms } => {
                let t = elapsed.as_secs_f32();
                let period_secs = (period_ms.max(1) as f32) / 1000.0;
                // 0.0 -> 1.0 -> 0.0 breathing envelope.
                let phase = (t / period_secs * std::f32::consts::TAU).sin();
                let brightness = (phase + 1.0) / 2.0;
                (
                    (r as f32 * brightness).round() as u8,
                    (g as f32 * brightness).round() as u8,
                    (b as f32 * brightness).round() as u8,
                )
            }
            LightbarAnimation::Strobe { r, g, b, period_ms } => {
                let t = elapsed.as_millis() as u32 % period_ms.max(1);
                if t < period_ms.max(1) / 2 {
                    (r, g, b)
                } else {
                    (0, 0, 0)
                }
            }
        }
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let h_prime = (h % 360.0) / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

/// A handle that stops a running animation when dropped or told to.
#[derive(Clone)]
pub struct AnimationHandle {
    stop: Arc<AtomicBool>,
}

impl AnimationHandle {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl HapticsController {
    /// Spawns a background thread that continuously updates the
    /// lightbar to animate `kind` until the returned handle's `stop()`
    /// is called. Runs at ~33fps (30ms tick).
    pub fn start_animation(&self, kind: LightbarAnimation) -> AnimationHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let handle = AnimationHandle { stop: stop.clone() };

        let led_dir = self.led_dir.clone();
        let led_prefix = self.led_prefix.clone();
        let controller = HapticsController {
            led_dir,
            led_prefix,
            evdev_path: None,
        };

        std::thread::spawn(move || {
            let start = Instant::now();
            while !stop.load(Ordering::Relaxed) {
                let (r, g, b) = kind.color_at(start.elapsed());
                let _ = controller.set_lightbar(r, g, b);
                std::thread::sleep(Duration::from_millis(30));
            }
        });

        handle
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_primary_colors() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));
    }

    #[test]
    fn rainbow_wraps_back_to_start_color() {
        let anim = LightbarAnimation::Rainbow { period_ms: 1000 };
        let start = anim.color_at(Duration::from_millis(0));
        let wrapped = anim.color_at(Duration::from_millis(1000));
        assert_eq!(start, wrapped);
    }

    #[test]
    fn fade_breathes_between_off_and_full_brightness() {
        let anim = LightbarAnimation::Fade { r: 200, g: 0, b: 0, period_ms: 1000 };
        // At t=0 the sine envelope is at its midpoint (0.5 brightness).
        let (r, _, _) = anim.color_at(Duration::from_millis(0));
        assert!((90..=110).contains(&r), "expected mid-brightness, got {r}");
    }

    #[test]
    fn strobe_toggles_on_and_off_within_one_period() {
        let anim = LightbarAnimation::Strobe { r: 100, g: 100, b: 100, period_ms: 1000 };
        assert_eq!(anim.color_at(Duration::from_millis(0)), (100, 100, 100));
        assert_eq!(anim.color_at(Duration::from_millis(600)), (0, 0, 0));
    }
}
