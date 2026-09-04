//! Resolves the currently-connected controller to its saved profile, so
//! the GUI (Bindings + Haptics tabs) reads and writes the same
//! per-controller file the daemon loads on connect and reapplies on
//! reconnect - without this, GUI changes would live only in the GUI's
//! own memory or hit hardware directly with nothing persisted.

use std::path::PathBuf;

use padshell_hid::device::{self, HidrawInfo};

pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config/padshell")
}

/// The first connected DS4's hidraw info, if any. The GUI doesn't have a
/// controller picker yet, so "first found" matches the pre-existing
/// haptics behavior - picking a specific controller when several are
/// connected is a follow-up, not needed for a single controller.
pub fn current_device() -> Option<HidrawInfo> {
    let mut candidates =
        device::find_all(padshell_hid::ds4::DS4_VENDOR_ID, padshell_hid::ds4::DS4_PRODUCT_ID_V1).ok()?;
    candidates.extend(
        device::find_all(padshell_hid::ds4::DS4_VENDOR_ID, padshell_hid::ds4::DS4_PRODUCT_ID_V2).unwrap_or_default(),
    );
    candidates.into_iter().next()
}

/// Loads (creating from `default.toml` if this is the first time this
/// controller has been seen) the currently-connected controller's
/// profile, alongside its path and hidraw info - the same profile file
/// the daemon's `run_worker` uses, so GUI edits and daemon behavior stay
/// in sync.
pub fn current_profile() -> Result<(padshell_binding::Config, PathBuf, HidrawInfo), String> {
    let info = current_device().ok_or_else(|| "no DS4 controller found".to_string())?;
    let device_id = info.device_id();
    let (config, path) =
        padshell_binding::Config::load_or_init_profile(&config_dir(), &device_id).map_err(|e| e.to_string())?;
    Ok((config, path, info))
}
