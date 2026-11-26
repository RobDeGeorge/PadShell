//! Resolves the currently-connected controller to its saved profile, so
//! the GUI (Bindings + Haptics tabs) reads and writes the same
//! per-controller file the daemon loads on connect and reapplies on
//! reconnect - without this, GUI changes would live only in the GUI's
//! own memory or hit hardware directly with nothing persisted.

use std::path::PathBuf;

use padshell_hid::controller::{self, ControllerInfo};

pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config/padshell")
}

/// The first connected controller of any kind (DS4 via hidraw, Xbox or
/// other gamepads via evdev), if any. The GUI doesn't have a controller
/// picker yet, so "first found" matches the pre-existing behavior -
/// picking a specific controller when several are connected is a
/// follow-up.
pub fn current_device() -> Option<ControllerInfo> {
    controller::first()
}

/// Loads (creating from `default.toml` if this is the first time this
/// controller has been seen) the currently-connected controller's
/// profile, alongside its path and device info - the same profile file
/// the daemon's `run_worker` uses, so GUI edits and daemon behavior stay
/// in sync. Records the controller's kind/name in the profile the same
/// way the daemon does, so labels are right even before the daemon has
/// ever seen this pad.
pub fn current_profile() -> Result<(padshell_binding::Config, PathBuf, ControllerInfo), String> {
    let info = current_device().ok_or_else(|| "no controller found".to_string())?;
    let device_id = info.device_id();
    let (mut config, path) =
        padshell_binding::Config::load_or_init_profile(&config_dir(), &device_id).map_err(|e| e.to_string())?;
    config
        .ensure_device_identity(&path, info.vendor_id, info.product_id, info.kind, &info.name)
        .map_err(|e| e.to_string())?;
    Ok((config, path, info))
}
