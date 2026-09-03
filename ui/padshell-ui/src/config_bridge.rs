//! Bridges padshell_binding::Config to QML: owns the config as the
//! source of truth in Rust, exposes it to QML as a JSON blob property
//! plus targeted mutating invokables that validate through the same
//! logic the daemon's config loader uses.

use cxx_qt_lib::{QString, QStringList};
use padshell_binding::config::{
    Action, ActivationMode, Binding, Config, InputSpec, Layer, LayerActivation, Macro, MacroStep,
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
    }
}

pub struct ConfigBridgeRust {
    config: Config,
    config_path: PathBuf,
    config_json: QString,
    button_names: QStringList,
    key_names: QStringList,
    last_error: QString,
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

        let buttons: QStringList = ButtonId::ALL
            .iter()
            .map(|b| QString::from(b.as_str()))
            .collect();
        self.as_mut().set_button_names(buttons);

        let keys: QStringList = padshell_output::keymap::KEY_NAMES
            .iter()
            .map(|k| QString::from(*k))
            .collect();
        self.as_mut().set_key_names(keys);
    }

    fn set_error(mut self: Pin<&mut Self>, msg: impl std::fmt::Display) {
        self.as_mut().set_last_error(QString::from(&msg.to_string()));
    }

    pub fn load(mut self: Pin<&mut Self>) -> bool {
        let path = self.config_path.clone();
        match Config::load(&path) {
            Ok(config) => {
                self.as_mut().rust_mut().config = config;
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
