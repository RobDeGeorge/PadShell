//! Bridges padshell_binding::Config to QML: owns the config as the
//! source of truth in Rust, exposes it to QML as a JSON blob property
//! plus targeted mutating invokables that validate through the same
//! logic the daemon's config loader uses.

use cxx_qt_lib::{QString, QStringList};
use padshell_binding::config::{
    Action, BatteryAlert, HapticsMode, HapticsState, ActivationMode, Binding, Config, InputSpec, Layer, LayerActivation, Macro, MacroStep,
    Scroll, Settings, StickMouse,
};
use padshell_core::ButtonId;
use std::path::PathBuf;

fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config/padshell/config.toml")
}

fn empty_config() -> Config {
    Config {
        schema_version: 1,
        device: Default::default(),
        settings: Settings::default(),
        layers: Vec::new(),
        bindings: Vec::new(),
        macros: Vec::new(),
        haptics: Default::default(),
        battery_alerts: Vec::new(),
    }
}

pub struct ConfigBridgeRust {
    config: Config,
    config_path: PathBuf,
    config_json: QString,
    button_names: QStringList,
    key_names: QStringList,
    last_error: QString,
    hypr_actions_json: QString,
    hypr_config_path: QString,
    hypr_error: QString,
    /// JSON array of desktop apps (see apps.rs), the wofi drun list.
    apps_json: QString,
    /// Highest `below_percent` among battery alerts (so the UI gauge
    /// turns red at the same point the lightbar does); 20 if none.
    lowest_alert_percent: i32,
    /// `ds4` / `xbox` / `generic` for the profile's controller, so QML
    /// can pick the right button glyphs.
    controller_kind: QString,
    /// Kernel-reported name of the controller this profile belongs to.
    device_name: QString,
    /// JSON object: canonical button name -> label as printed on this
    /// kind of controller ("cross" -> "✕" on a DS4, "A" on an Xbox pad).
    button_labels_json: QString,
}

impl Default for ConfigBridgeRust {
    fn default() -> Self {
        ConfigBridgeRust {
            config: empty_config(),
            config_path: default_config_path(),
            config_json: QString::default(),
            button_names: QStringList::default(),
            key_names: QStringList::default(),
            last_error: QString::default(),
            hypr_actions_json: QString::from("[]"),
            hypr_config_path: QString::from(
                &crate::hypr::default_config_path().to_string_lossy().to_string(),
            ),
            hypr_error: QString::default(),
            apps_json: QString::from("[]"),
            lowest_alert_percent: 20,
            controller_kind: QString::from("generic"),
            device_name: QString::default(),
            button_labels_json: QString::from("{}"),
        }
    }
}

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, config_json)]
        #[qproperty(QStringList, button_names)]
        #[qproperty(QStringList, key_names)]
        #[qproperty(QString, last_error)]
        #[qproperty(QString, hypr_actions_json)]
        #[qproperty(QString, hypr_config_path)]
        #[qproperty(QString, hypr_error)]
        #[qproperty(QString, apps_json)]
        #[qproperty(i32, lowest_alert_percent)]
        #[qproperty(QString, controller_kind)]
        #[qproperty(QString, device_name)]
        #[qproperty(QString, button_labels_json)]
        type ConfigBridge = super::ConfigBridgeRust;

        /// Load the config from disk (default path: ~/.config/padshell/config.toml).
        /// Returns true on success; failure details land in last_error.
        #[qinvokable]
        fn load(self: Pin<&mut ConfigBridge>) -> bool;

        /// Serialize the current config as TOML and write it to disk -
        /// the running daemon's ConfigWatcher picks this up automatically.
        #[qinvokable]
        fn save(self: Pin<&mut ConfigBridge>) -> bool;

        /// Add a binding to the default layer. `input` is a button name
        /// or "a+b" for a combo. `action_json` is a JSON-encoded Action.
        #[qinvokable]
        fn add_binding(self: Pin<&mut ConfigBridge>, input: QString, action_json: QString) -> bool;

        #[qinvokable]
        fn remove_binding(self: Pin<&mut ConfigBridge>, index: i32);

        /// Bind `input` in a layer (-1 = default layer), replacing any
        /// existing binding for that same input so a button only ever
        /// does one thing per layer - the "assign a button" operation
        /// the game-style Bindings tab uses.
        #[qinvokable]
        fn set_binding(
            self: Pin<&mut ConfigBridge>,
            layer_index: i32,
            input: QString,
            action_json: QString,
        ) -> bool;

        /// Remove every binding for `input` in a layer (-1 = default).
        #[qinvokable]
        fn unbind(self: Pin<&mut ConfigBridge>, layer_index: i32, input: QString);

        /// Remove every single-button binding in a layer (-1 = default);
        /// chords are left alone.
        #[qinvokable]
        fn clear_bindings(self: Pin<&mut ConfigBridge>, layer_index: i32);

        /// Add or replace (by index; -1 appends) a battery alert.
        /// `mode` is off/solid/rainbow/fade/strobe.
        #[qinvokable]
        fn set_battery_alert(
            self: Pin<&mut ConfigBridge>,
            index: i32,
            below_percent: i32,
            mode: QString,
            r: i32,
            g: i32,
            b: i32,
            period_ms: i32,
            rumble: bool,
        ) -> bool;

        #[qinvokable]
        fn remove_battery_alert(self: Pin<&mut ConfigBridge>, index: i32);

        /// Parse the Hyprland config at hypr_config_path (empty = the
        /// default ~/.config/hypr/hyprland.conf) into hypr_actions_json.
        #[qinvokable]
        fn load_hypr(self: Pin<&mut ConfigBridge>, path: QString) -> bool;

        #[qinvokable]
        fn add_layer(self: Pin<&mut ConfigBridge>, name: QString, activate_button: QString, hold: bool) -> bool;

        #[qinvokable]
        fn remove_layer(self: Pin<&mut ConfigBridge>, index: i32);

        #[qinvokable]
        fn add_layer_binding(
            self: Pin<&mut ConfigBridge>,
            layer_index: i32,
            input: QString,
            action_json: QString,
        ) -> bool;

        #[qinvokable]
        fn remove_layer_binding(self: Pin<&mut ConfigBridge>, layer_index: i32, binding_index: i32);

        #[qinvokable]
        fn set_layer_stick_mouse(
            self: Pin<&mut ConfigBridge>,
            layer_index: i32,
            stick: QString,
            sensitivity: f32,
        ) -> bool;

        #[qinvokable]
        fn set_layer_scroll(
            self: Pin<&mut ConfigBridge>,
            layer_index: i32,
            stick: QString,
            axis: QString,
            sensitivity: f32,
        ) -> bool;

        #[qinvokable]
        fn add_macro(self: Pin<&mut ConfigBridge>, name: QString, steps_json: QString) -> bool;

        #[qinvokable]
        fn remove_macro(self: Pin<&mut ConfigBridge>, index: i32);
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use ffi::ConfigBridge;

impl ConfigBridge {
    fn sync_to_qml(mut self: Pin<&mut Self>) {
        let json = serde_json::to_string(&self.config)
            .unwrap_or_else(|_| "{}".to_string());
        self.as_mut().set_config_json(QString::from(&json));
        let threshold = self
            .config
            .battery_alerts
            .iter()
            .map(|a| a.below_percent as i32)
            .max()
            .unwrap_or(20);
        self.as_mut().set_lowest_alert_percent(threshold);

        let buttons: QStringList = ButtonId::ALL
            .iter()
            .map(|b| QString::from(b.as_str()))
            .collect();
        self.as_mut().set_button_names(buttons);

        let kind = self.config.device.kind.unwrap_or_default();
        self.as_mut().set_controller_kind(QString::from(kind.as_str()));
        let name = self.config.device.name.clone().unwrap_or_else(|| kind.display_name().to_string());
        self.as_mut().set_device_name(QString::from(&name));
        let labels: serde_json::Map<String, serde_json::Value> = ButtonId::ALL
            .iter()
            .map(|b| (b.as_str().to_string(), serde_json::Value::from(b.label(kind))))
            .collect();
        self.as_mut().set_button_labels_json(QString::from(&serde_json::Value::Object(labels).to_string()));

        let keys: QStringList = padshell_output::keymap::KEY_NAMES
            .iter()
            .map(|k| QString::from(*k))
            .collect();
        self.as_mut().set_key_names(keys);
    }

    fn set_error(mut self: Pin<&mut Self>, msg: impl std::fmt::Display) {
        self.as_mut().set_last_error(QString::from(&msg.to_string()));
    }

    /// Loads the currently-connected controller's own profile (creating
    /// it, seeded from default.toml, the first time this controller is
    /// seen) rather than a single global file - each controller keeps
    /// its own bindings, matching what the daemon reads on connect.
    pub fn load(mut self: Pin<&mut Self>) -> bool {
        match crate::device_profile::current_profile() {
            Ok((config, path, _info)) => {
                let mut rust = self.as_mut().rust_mut();
                rust.config = config;
                rust.config_path = path;
                self.as_mut().sync_to_qml();
                true
            }
            Err(err) => {
                self.as_mut().set_error(err);
                false
            }
        }
    }

    pub fn save(mut self: Pin<&mut Self>) -> bool {
        let toml_text = match toml::to_string_pretty(&self.config) {
            Ok(text) => text,
            Err(err) => {
                self.as_mut().set_error(err);
                return false;
            }
        };
        let path = self.config_path.clone();
        if let Some(parent) = path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                self.as_mut().set_error(err);
                return false;
            }
        }
        match std::fs::write(&path, toml_text) {
            Ok(()) => true,
            Err(err) => {
                self.as_mut().set_error(err);
                false
            }
        }
    }

    pub fn add_binding(mut self: Pin<&mut Self>, input: QString, action_json: QString) -> bool {
        let Some(input) = parse_input(&input.to_string()) else {
            self.as_mut().set_error("unknown button name in input");
            return false;
        };
        let action: Action = match serde_json::from_str(&action_json.to_string()) {
            Ok(a) => a,
            Err(err) => {
                self.as_mut().set_error(err);
                return false;
            }
        };
        self.as_mut()
            .rust_mut()
            .config
            .bindings
            .push(Binding { input, action });
        self.as_mut().sync_to_qml();
        true
    }

    pub fn remove_binding(mut self: Pin<&mut Self>, index: i32) {
        let bindings = &mut self.as_mut().rust_mut().config.bindings;
        if let Some(i) = usize::try_from(index).ok().filter(|&i| i < bindings.len()) {
            bindings.remove(i);
        }
        self.as_mut().sync_to_qml();
    }

    pub fn set_binding(
        mut self: Pin<&mut Self>,
        layer_index: i32,
        input: QString,
        action_json: QString,
    ) -> bool {
        let Some(input) = parse_input(&input.to_string()) else {
            self.as_mut().set_error("unknown button name in input");
            return false;
        };
        let action: Action = match serde_json::from_str(&action_json.to_string()) {
            Ok(a) => a,
            Err(err) => {
                self.as_mut().set_error(err);
                return false;
            }
        };
        let mut rust = self.as_mut().rust_mut();
        let Some(bindings) = bindings_of(&mut rust.config, layer_index) else {
            drop(rust);
            self.as_mut().set_error("invalid layer index");
            return false;
        };
        let key = input_key(&input);
        bindings.retain(|b| input_key(&b.input) != key);
        bindings.push(Binding { input, action });
        drop(rust);
        self.as_mut().set_last_error(QString::default());
        self.as_mut().sync_to_qml();
        true
    }

    pub fn unbind(mut self: Pin<&mut Self>, layer_index: i32, input: QString) {
        let key = normalize_input_string(&input.to_string());
        let mut rust = self.as_mut().rust_mut();
        if let Some(bindings) = bindings_of(&mut rust.config, layer_index) {
            bindings.retain(|b| input_key(&b.input) != key);
        }
        drop(rust);
        self.as_mut().sync_to_qml();
    }

    pub fn clear_bindings(mut self: Pin<&mut Self>, layer_index: i32) {
        let mut rust = self.as_mut().rust_mut();
        if let Some(bindings) = bindings_of(&mut rust.config, layer_index) {
            bindings.retain(|b| matches!(b.input, InputSpec::Combo { .. }));
        }
        drop(rust);
        self.as_mut().sync_to_qml();
    }

    pub fn set_battery_alert(
        mut self: Pin<&mut Self>,
        index: i32,
        below_percent: i32,
        mode: QString,
        r: i32,
        g: i32,
        b: i32,
        period_ms: i32,
        rumble: bool,
    ) -> bool {
        let mode = match mode.to_string().as_str() {
            "off" => HapticsMode::Off,
            "solid" => HapticsMode::Solid,
            "rainbow" => HapticsMode::Rainbow,
            "fade" => HapticsMode::Fade,
            "strobe" => HapticsMode::Strobe,
            other => {
                self.as_mut().set_error(format!("unknown lightbar mode {other}"));
                return false;
            }
        };
        let alert = BatteryAlert {
            below_percent: below_percent.clamp(0, 100) as u8,
            lightbar: HapticsState {
                mode,
                r: r.clamp(0, 255) as u8,
                g: g.clamp(0, 255) as u8,
                b: b.clamp(0, 255) as u8,
                period_ms: period_ms.max(100) as u32,
            },
            rumble,
        };
        let mut rust = self.as_mut().rust_mut();
        match usize::try_from(index).ok().filter(|&i| i < rust.config.battery_alerts.len()) {
            Some(i) => rust.config.battery_alerts[i] = alert,
            None => rust.config.battery_alerts.push(alert),
        }
        rust.config.battery_alerts.sort_by_key(|a| std::cmp::Reverse(a.below_percent));
        drop(rust);
        self.as_mut().set_last_error(QString::default());
        self.as_mut().sync_to_qml();
        true
    }

    pub fn remove_battery_alert(mut self: Pin<&mut Self>, index: i32) {
        let alerts = &mut self.as_mut().rust_mut().config.battery_alerts;
        if let Some(i) = usize::try_from(index).ok().filter(|&i| i < alerts.len()) {
            alerts.remove(i);
        }
        self.as_mut().sync_to_qml();
    }

    pub fn load_hypr(mut self: Pin<&mut Self>, path: QString) -> bool {
        let path_str = path.to_string();
        let path = if path_str.trim().is_empty() {
            crate::hypr::default_config_path()
        } else {
            PathBuf::from(path_str.trim())
        };
        self.as_mut()
            .set_hypr_config_path(QString::from(&path.to_string_lossy().to_string()));
        match crate::hypr::load(&path) {
            Ok(hypr) => {
                let json = serde_json::to_string(&hypr.actions).unwrap_or_else(|_| "[]".into());
                self.as_mut().set_hypr_actions_json(QString::from(&json));
                self.as_mut().set_hypr_error(QString::default());
                let terminal = hypr.terminal.unwrap_or_else(|| "kitty".to_string());
                let apps = crate::apps::scan(&terminal);
                let apps_json = serde_json::to_string(&apps).unwrap_or_else(|_| "[]".into());
                self.as_mut().set_apps_json(QString::from(&apps_json));
                true
            }
            Err(err) => {
                self.as_mut().set_hypr_actions_json(QString::from("[]"));
                self.as_mut()
                    .set_hypr_error(QString::from(&format!("{}: {err}", path.display())));
                false
            }
        }
    }

    pub fn add_layer(mut self: Pin<&mut Self>, name: QString, activate_button: QString, hold: bool) -> bool {
        let button = activate_button.to_string();
        if ButtonId::from_str(&button).is_none() {
            self.as_mut().set_error("unknown button name for layer activation");
            return false;
        }
        let layer = Layer {
            name: name.to_string(),
            activate: LayerActivation {
                button,
                mode: if hold { ActivationMode::Hold } else { ActivationMode::Toggle },
            },
            stick_mouse: None,
            scroll: None,
            bindings: Vec::new(),
        };
        self.as_mut().rust_mut().config.layers.push(layer);
        self.as_mut().sync_to_qml();
        true
    }

    pub fn remove_layer(mut self: Pin<&mut Self>, index: i32) {
        let layers = &mut self.as_mut().rust_mut().config.layers;
        if let Some(i) = usize::try_from(index).ok().filter(|&i| i < layers.len()) {
            layers.remove(i);
        }
        self.as_mut().sync_to_qml();
    }

    pub fn add_layer_binding(
        mut self: Pin<&mut Self>,
        layer_index: i32,
        input: QString,
        action_json: QString,
    ) -> bool {
        let Some(input) = parse_input(&input.to_string()) else {
            self.as_mut().set_error("unknown button name in input");
            return false;
        };
        let action: Action = match serde_json::from_str(&action_json.to_string()) {
            Ok(a) => a,
            Err(err) => {
                self.as_mut().set_error(err);
                return false;
            }
        };
        let Some(idx) = usize::try_from(layer_index).ok() else {
            self.as_mut().set_error("invalid layer index");
            return false;
        };
        let mut binding = self.as_mut().rust_mut();
        let Some(layer) = binding.config.layers.get_mut(idx) else {
            self.as_mut().set_error("invalid layer index");
            return false;
        };
        layer.bindings.push(Binding { input, action });
        self.as_mut().sync_to_qml();
        true
    }

    pub fn remove_layer_binding(mut self: Pin<&mut Self>, layer_index: i32, binding_index: i32) {
        if let Some(idx) = usize::try_from(layer_index).ok() {
            if let Some(layer) = self.as_mut().rust_mut().config.layers.get_mut(idx) {
                if let Some(bi) = usize::try_from(binding_index)
                    .ok()
                    .filter(|&bi| bi < layer.bindings.len())
                {
                    layer.bindings.remove(bi);
                }
            }
        }
        self.as_mut().sync_to_qml();
    }

    pub fn set_layer_stick_mouse(
        mut self: Pin<&mut Self>,
        layer_index: i32,
        stick: QString,
        sensitivity: f32,
    ) -> bool {
        let stick_name = stick.to_string();
        if padshell_core::StickId::from_str(&stick_name).is_none() {
            self.as_mut().set_error("unknown stick name");
            return false;
        }
        let Some(idx) = usize::try_from(layer_index).ok() else {
            self.as_mut().set_error("invalid layer index");
            return false;
        };
        let mut binding = self.as_mut().rust_mut();
        let Some(layer) = binding.config.layers.get_mut(idx) else {
            self.as_mut().set_error("invalid layer index");
            return false;
        };
        layer.stick_mouse = Some(StickMouse {
            stick: stick_name,
            sensitivity,
            accel_curve: Default::default(),
        });
        self.as_mut().sync_to_qml();
        true
    }

    pub fn set_layer_scroll(
        mut self: Pin<&mut Self>,
        layer_index: i32,
        stick: QString,
        axis: QString,
        sensitivity: f32,
    ) -> bool {
        let stick_name = stick.to_string();
        if padshell_core::StickId::from_str(&stick_name).is_none() {
            self.as_mut().set_error("unknown stick name");
            return false;
        }
        let Some(idx) = usize::try_from(layer_index).ok() else {
            self.as_mut().set_error("invalid layer index");
            return false;
        };
        let mut binding = self.as_mut().rust_mut();
        let Some(layer) = binding.config.layers.get_mut(idx) else {
            self.as_mut().set_error("invalid layer index");
            return false;
        };
        layer.scroll = Some(Scroll {
            stick: stick_name,
            axis: axis.to_string(),
            sensitivity,
        });
        self.as_mut().sync_to_qml();
        true
    }

    pub fn add_macro(mut self: Pin<&mut Self>, name: QString, steps_json: QString) -> bool {
        let steps: Vec<MacroStep> = match serde_json::from_str(&steps_json.to_string()) {
            Ok(s) => s,
            Err(err) => {
                self.as_mut().set_error(err);
                return false;
            }
        };
        self.as_mut().rust_mut().config.macros.push(Macro {
            name: name.to_string(),
            steps,
        });
        self.as_mut().sync_to_qml();
        true
    }

    pub fn remove_macro(mut self: Pin<&mut Self>, index: i32) {
        let macros = &mut self.as_mut().rust_mut().config.macros;
        if let Some(i) = usize::try_from(index).ok().filter(|&i| i < macros.len()) {
            macros.remove(i);
        }
        self.as_mut().sync_to_qml();
    }
}

/// Order-insensitive identity for an input, so "l1+r1" and "r1+l1"
/// are the same binding slot.
fn input_key(input: &InputSpec) -> String {
    match input {
        InputSpec::Single(s) => s.clone(),
        InputSpec::Combo { combo } => {
            let mut c = combo.clone();
            c.sort();
            c.join("+")
        }
    }
}

fn normalize_input_string(s: &str) -> String {
    let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
    if parts.len() > 1 {
        parts.sort();
    }
    parts.join("+")
}

fn bindings_of(config: &mut Config, layer_index: i32) -> Option<&mut Vec<Binding>> {
    if layer_index < 0 {
        return Some(&mut config.bindings);
    }
    let idx = usize::try_from(layer_index).ok()?;
    config.layers.get_mut(idx).map(|l| &mut l.bindings)
}

fn parse_input(s: &str) -> Option<InputSpec> {
    if let Some((a, b)) = s.split_once('+') {
        let combo: Vec<String> = s.split('+').map(str::to_string).collect();
        if ButtonId::from_str(a).is_some() && ButtonId::from_str(b).is_some() {
            return Some(InputSpec::Combo { combo });
        }
        return None;
    }
    if ButtonId::from_str(s).is_some() {
        Some(InputSpec::Single(s.to_string()))
    } else {
        None
    }
}
