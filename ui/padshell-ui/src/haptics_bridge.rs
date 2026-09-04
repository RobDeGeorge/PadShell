//! Bridges DS4 lightbar/rumble control to QML, plus a small persisted
//! library of named "effects" (a saved color + rumble combo) so users
//! can build and replay custom lighting/rumble presets.

use cxx_qt_lib::QString;
use padshell_binding::{HapticsMode, HapticsState};
use padshell_hid::{AnimationHandle, HapticsController, LightbarAnimation};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Discovers the haptics controller for whichever DS4 is currently
/// connected, scoped to that specific device (not "first DS4 anywhere")
/// so this matches what `persist_haptics` writes to.
fn discover() -> Option<HapticsController> {
    let info = crate::device_profile::current_device()?;
    Some(HapticsController::discover(&info.hid_syspath))
}

/// Saves the given lightbar/animation state into the currently-connected
/// controller's own profile file, so it's what the daemon reapplies the
/// next time this controller reconnects. The hardware write already
/// happened by the time this is called, so a failure here doesn't undo
/// the lightbar change on screen - it just means it won't survive a
/// reconnect, which callers surface via last_error rather than swallow.
fn persist_haptics(mode: HapticsMode, r: u8, g: u8, b: u8, period_ms: u32) -> Result<(), String> {
    let (mut config, path, _info) = crate::device_profile::current_profile()?;
    config.haptics = HapticsState { mode, r, g, b, period_ms };
    config.save(&path).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    pub name: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub weak: u16,
    pub strong: u16,
    pub duration_ms: u16,
}

fn effects_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config/padshell/effects.json")
}

pub struct HapticsBridgeRust {
    effects: Vec<Effect>,
    effects_json: QString,
    last_error: QString,
    has_lightbar: bool,
    has_rumble: bool,
    is_animating: bool,
    animation: Option<AnimationHandle>,
}

impl Default for HapticsBridgeRust {
    fn default() -> Self {
        HapticsBridgeRust {
            effects: Vec::new(),
            effects_json: QString::from("[]"),
            last_error: QString::default(),
            has_lightbar: false,
            has_rumble: false,
            is_animating: false,
            animation: None,
        }
    }
}

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, effects_json)]
        #[qproperty(QString, last_error)]
        #[qproperty(bool, has_lightbar)]
        #[qproperty(bool, has_rumble)]
        #[qproperty(bool, is_animating)]
        type HapticsBridge = super::HapticsBridgeRust;

        /// Checks whether a DS4 with lightbar/rumble is currently
        /// connected. Call again after (re)connecting the controller.
        #[qinvokable]
        fn refresh_capabilities(self: Pin<&mut HapticsBridge>);

        #[qinvokable]
        fn set_lightbar(self: Pin<&mut HapticsBridge>, r: i32, g: i32, b: i32) -> bool;

        /// Non-blocking: plays the rumble on a background thread for
        /// duration_ms, then stops it.
        #[qinvokable]
        fn rumble(self: Pin<&mut HapticsBridge>, weak: i32, strong: i32, duration_ms: i32) -> bool;

        #[qinvokable]
        fn load_effects(self: Pin<&mut HapticsBridge>);

        #[qinvokable]
        fn save_effect(
            self: Pin<&mut HapticsBridge>,
            name: QString,
            r: i32,
            g: i32,
            b: i32,
            weak: i32,
            strong: i32,
            duration_ms: i32,
        ) -> bool;

        #[qinvokable]
        fn remove_effect(self: Pin<&mut HapticsBridge>, index: i32);

        /// Applies both the lightbar color and rumble of a saved effect.
        #[qinvokable]
        fn play_effect(self: Pin<&mut HapticsBridge>, index: i32) -> bool;

        /// Starts a continuously-running lightbar animation. `kind` is
        /// "rainbow", "fade", or "strobe"; r/g/b are the base color for
        /// fade/strobe (ignored for rainbow); period_ms is one full cycle.
        #[qinvokable]
        fn start_animation(
            self: Pin<&mut HapticsBridge>,
            kind: QString,
            r: i32,
            g: i32,
            b: i32,
            period_ms: i32,
        ) -> bool;

        #[qinvokable]
        fn stop_animation(self: Pin<&mut HapticsBridge>);
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use ffi::HapticsBridge;

impl HapticsBridge {
    fn sync_effects_json(mut self: Pin<&mut Self>) {
        let json = serde_json::to_string(&self.effects).unwrap_or_else(|_| "[]".to_string());
        self.as_mut().set_effects_json(QString::from(&json));
    }

    fn set_error(mut self: Pin<&mut Self>, msg: impl std::fmt::Display) {
        self.as_mut().set_last_error(QString::from(&msg.to_string()));
    }

    pub fn refresh_capabilities(mut self: Pin<&mut Self>) {
        match discover() {
            Some(h) => {
                self.as_mut().set_has_lightbar(h.has_lightbar());
                self.as_mut().set_has_rumble(h.has_rumble());
            }
            None => {
                self.as_mut().set_has_lightbar(false);
                self.as_mut().set_has_rumble(false);
                self.as_mut().set_error("no DS4 controller found");
            }
        }
    }

    pub fn set_lightbar(mut self: Pin<&mut Self>, r: i32, g: i32, b: i32) -> bool {
        let Some(haptics) = discover() else {
            self.as_mut().set_error("no DS4 controller found");
            return false;
        };
        let (r, g, b) = (clamp_u8(r), clamp_u8(g), clamp_u8(b));
        match haptics.set_lightbar(r, g, b) {
            Ok(()) => {
                if let Err(err) = persist_haptics(HapticsMode::Solid, r, g, b, 0) {
                    self.as_mut().set_error(format!("lightbar set, but won't survive a reconnect: {err}"));
                }
                true
            }
            Err(err) => {
                self.as_mut().set_error(err);
                false
            }
        }
    }

    pub fn rumble(mut self: Pin<&mut Self>, weak: i32, strong: i32, duration_ms: i32) -> bool {
        let Some(haptics) = discover() else {
            self.as_mut().set_error("no DS4 controller found");
            return false;
        };
        if !haptics.has_rumble() {
            self.as_mut().set_error("controller has no force-feedback support");
            return false;
        }
        let weak = clamp_u16(weak);
        let strong = clamp_u16(strong);
        let duration_ms = clamp_u16(duration_ms);
        std::thread::spawn(move || {
            let _ = haptics.rumble(weak, strong, duration_ms);
        });
        true
    }

    pub fn load_effects(mut self: Pin<&mut Self>) {
        let path = effects_path();
        let effects = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<Effect>>(&text).ok())
            .unwrap_or_default();
        self.as_mut().rust_mut().effects = effects;
        self.as_mut().sync_effects_json();
    }

    fn persist(&self) -> std::io::Result<()> {
        let path = effects_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.effects).unwrap_or_else(|_| "[]".to_string());
        std::fs::write(&path, json)
    }

    pub fn save_effect(
        mut self: Pin<&mut Self>,
        name: QString,
        r: i32,
        g: i32,
        b: i32,
        weak: i32,
        strong: i32,
        duration_ms: i32,
    ) -> bool {
        let name = name.to_string();
        let effect = Effect {
            name: name.clone(),
            r: clamp_u8(r),
            g: clamp_u8(g),
            b: clamp_u8(b),
            weak: clamp_u16(weak),
            strong: clamp_u16(strong),
            duration_ms: clamp_u16(duration_ms),
        };

        let mut rust = self.as_mut().rust_mut();
        if let Some(existing) = rust.effects.iter_mut().find(|e| e.name == name) {
            *existing = effect;
        } else {
            rust.effects.push(effect);
        }

        if let Err(err) = self.persist() {
            self.as_mut().set_error(err);
            return false;
        }
        self.as_mut().sync_effects_json();
        true
    }

    pub fn remove_effect(mut self: Pin<&mut Self>, index: i32) {
        let mut rust = self.as_mut().rust_mut();
        if let Some(i) = usize::try_from(index).ok().filter(|&i| i < rust.effects.len()) {
            rust.effects.remove(i);
        }
        if let Err(err) = self.persist() {
            self.as_mut().set_error(err);
        }
        self.as_mut().sync_effects_json();
    }

    pub fn play_effect(mut self: Pin<&mut Self>, index: i32) -> bool {
        let Some(effect) = usize::try_from(index)
            .ok()
            .and_then(|i| self.effects.get(i))
            .cloned()
        else {
            self.as_mut().set_error("invalid effect index");
            return false;
        };
        let lightbar_ok = self
            .as_mut()
            .set_lightbar(effect.r as i32, effect.g as i32, effect.b as i32);
        let rumble_ok = self.as_mut().rumble(
            effect.weak as i32,
            effect.strong as i32,
            effect.duration_ms as i32,
        );
        lightbar_ok && rumble_ok
    }
}

impl HapticsBridge {
    pub fn start_animation(
        mut self: Pin<&mut Self>,
        kind: QString,
        r: i32,
        g: i32,
        b: i32,
        period_ms: i32,
    ) -> bool {
        let Some(haptics) = discover() else {
            self.as_mut().set_error("no DS4 controller found");
            return false;
        };
        if !haptics.has_lightbar() {
            self.as_mut().set_error("controller has no lightbar");
            return false;
        }

        let period_ms = period_ms.clamp(50, 60_000) as u32;
        let (r, g, b) = (clamp_u8(r), clamp_u8(g), clamp_u8(b));
        let (mode, animation) = match kind.to_string().as_str() {
            "rainbow" => (HapticsMode::Rainbow, LightbarAnimation::Rainbow { period_ms }),
            "fade" => (HapticsMode::Fade, LightbarAnimation::Fade { r, g, b, period_ms }),
            "strobe" => (HapticsMode::Strobe, LightbarAnimation::Strobe { r, g, b, period_ms }),
            other => {
                self.as_mut().set_error(format!("unknown animation kind: {other}"));
                return false;
            }
        };

        // Stop whatever was previously running before starting the new one.
        if let Some(old) = self.animation.as_ref() {
            old.stop();
        }
        let handle = haptics.start_animation(animation);
        let mut rust = self.as_mut().rust_mut();
        rust.animation = Some(handle);
        self.as_mut().set_is_animating(true);
        if let Err(err) = persist_haptics(mode, r, g, b, period_ms) {
            self.as_mut().set_error(format!("animation started, but won't survive a reconnect: {err}"));
        }
        true
    }

    pub fn stop_animation(mut self: Pin<&mut Self>) {
        if let Some(handle) = self.animation.as_ref() {
            handle.stop();
        }
        self.as_mut().rust_mut().animation = None;
        self.as_mut().set_is_animating(false);
        if let Err(err) = persist_haptics(HapticsMode::Off, 0, 0, 0, 0) {
            self.as_mut().set_error(err);
        }
    }
}

fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

fn clamp_u16(v: i32) -> u16 {
    v.clamp(0, 65535) as u16
}
