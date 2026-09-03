//! TOML config schema. Plain serde structs with no coupling to the
//! runtime engine or daemon, so a future GUI can depend on just this
//! module to read/write the same config file.

use serde::{Deserialize, Serialize};
use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("unknown button/stick name: {0}")]
    UnknownName(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub device: DeviceConfig,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub bindings: Vec<Binding>,
    #[serde(default)]
    pub macros: Vec<Macro>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&text)?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_str(text: &str) -> Result<Config, ConfigError> {
        let config: Config = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    /// Checks that every button/stick name referenced actually resolves,
    /// so bad config fails fast at load time rather than silently
    /// no-op-ing at runtime.
    fn validate(&self) -> Result<(), ConfigError> {
        for binding in &self.bindings {
            binding.input.validate()?;
        }
        for macro_def in &self.macros {
            for step in &macro_def.steps {
                step.validate()?;
            }
        }
        for layer in &self.layers {
            layer.activate.button_id()?;
            for binding in &layer.bindings {
                binding.input.validate()?;
            }
            if let Some(sm) = &layer.stick_mouse {
                padshell_core::StickId::from_str(&sm.stick)
                    .ok_or_else(|| ConfigError::UnknownName(sm.stick.clone()))?;
            }
            if let Some(sc) = &layer.scroll {
                padshell_core::StickId::from_str(&sc.stick)
                    .ok_or_else(|| ConfigError::UnknownName(sc.stick.clone()))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub poll_hz: u32,
    pub stick_deadzone: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            poll_hz: 250,
            stick_deadzone: 0.08,
        }
    }
}

/// Either a single button name or a chord of buttons held together.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputSpec {
    Single(String),
    Combo { combo: Vec<String> },
}

impl InputSpec {
    pub fn buttons(&self) -> Vec<&str> {
        match self {
            InputSpec::Single(s) => vec![s.as_str()],
            InputSpec::Combo { combo } => combo.iter().map(String::as_str).collect(),
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        for name in self.buttons() {
            padshell_core::ButtonId::from_str(name)
                .ok_or_else(|| ConfigError::UnknownName(name.to_string()))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub input: InputSpec,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Key { key: String },
    KeyCombo { keys: Vec<String> },
    Macro { name: String },
    MouseButton { button: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub steps: Vec<MacroStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroStep {
    Key {
        key: String,
        #[serde(default)]
        hold_ms: Option<u64>,
    },
    KeyCombo {
        keys: Vec<String>,
        #[serde(default)]
        hold_ms: Option<u64>,
    },
    Delay {
        ms: u64,
    },
}

impl MacroStep {
    fn validate(&self) -> Result<(), ConfigError> {
        // Key name validation lives in padshell-output's keycode table;
        // binding config only checks structural well-formedness here.
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub activate: LayerActivation,
    #[serde(default)]
    pub stick_mouse: Option<StickMouse>,
    #[serde(default)]
    pub scroll: Option<Scroll>,
    #[serde(default)]
    pub bindings: Vec<Binding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerActivation {
    pub button: String,
    pub mode: ActivationMode,
}

impl LayerActivation {
    pub fn button_id(&self) -> Result<padshell_core::ButtonId, ConfigError> {
        padshell_core::ButtonId::from_str(&self.button)
            .ok_or_else(|| ConfigError::UnknownName(self.button.clone()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationMode {
    Hold,
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickMouse {
    pub stick: String,
    pub sensitivity: f32,
    #[serde(default)]
    pub accel_curve: AccelCurve,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccelCurve {
    #[default]
    Linear,
    Quadratic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scroll {
    pub stick: String,
    #[serde(default = "default_scroll_axis")]
    pub axis: String,
    pub sensitivity: f32,
}

fn default_scroll_axis() -> String {
    "y".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config() {
        let toml = r#"
schema_version = 1

[[bindings]]
input = "cross"
action = { type = "key", key = "space" }
"#;
        let config = Config::from_str(toml).unwrap();
        assert_eq!(config.bindings.len(), 1);
    }

    #[test]
    fn parses_combo_and_layers() {
        let toml = r#"
schema_version = 1

[[bindings]]
input = { combo = ["l1", "r1"] }
action = { type = "key_combo", keys = ["super", "q"] }

[[layers]]
name = "mouse"
activate = { button = "l1", mode = "hold" }

  [layers.stick_mouse]
  stick = "right"
  sensitivity = 900.0

  [[layers.bindings]]
  input = "r2"
  action = { type = "mouse_button", button = "left" }

[[macros]]
name = "screenshot"
steps = [
  { type = "key_combo", keys = ["super", "shift", "s"] },
]
"#;
        let config = Config::from_str(toml).unwrap();
        assert_eq!(config.layers.len(), 1);
        assert_eq!(config.layers[0].bindings.len(), 1);
        assert_eq!(config.macros[0].steps.len(), 1);
    }

    #[test]
    fn rejects_unknown_button_name() {
        let toml = r#"
schema_version = 1

[[bindings]]
input = "not_a_real_button"
action = { type = "key", key = "space" }
"#;
        assert!(Config::from_str(toml).is_err());
    }
}
