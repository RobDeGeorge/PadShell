//! Layer activation (hold/toggle) state machine. Time-based hold
//! detection is driven by a caller-supplied "now" rather than reading
//! the clock directly, so it's testable without real time passing.

use crate::config::ActivationMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerState {
    #[default]
    Inactive,
    Active,
}

/// Tracks whether one non-default layer is currently active, given the
/// press/release state of its activation button each tick.
#[derive(Debug, Default)]
pub struct LayerSwitch {
    mode: Option<ActivationMode>,
    state: LayerState,
}

impl LayerSwitch {
    pub fn new(mode: ActivationMode) -> Self {
        LayerSwitch {
            mode: Some(mode),
            state: LayerState::Inactive,
        }
    }

    pub fn is_active(&self) -> bool {
        self.state == LayerState::Active
    }

    /// Call once per tick with whether the activation button is currently
    /// pressed and whether this tick is a rising edge (press) for it.
    pub fn on_tick(&mut self, is_pressed: bool, is_press_edge: bool) {
        match self.mode {
            Some(ActivationMode::Hold) => {
                self.state = if is_pressed {
                    LayerState::Active
                } else {
                    LayerState::Inactive
                };
            }
            Some(ActivationMode::Toggle) => {
                if is_press_edge {
                    self.state = match self.state {
                        LayerState::Active => LayerState::Inactive,
                        LayerState::Inactive => LayerState::Active,
                    };
                }
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_mode_tracks_button_state() {
        let mut sw = LayerSwitch::new(ActivationMode::Hold);
        assert!(!sw.is_active());
        sw.on_tick(true, true);
        assert!(sw.is_active());
        sw.on_tick(true, false);
        assert!(sw.is_active());
        sw.on_tick(false, false);
        assert!(!sw.is_active());
    }

    #[test]
    fn toggle_mode_flips_on_press_edge() {
        let mut sw = LayerSwitch::new(ActivationMode::Toggle);
        sw.on_tick(true, true);
        assert!(sw.is_active());
        // holding, no new edge - stays active
        sw.on_tick(true, false);
        assert!(sw.is_active());
        sw.on_tick(false, false);
        assert!(sw.is_active());
        sw.on_tick(true, true);
        assert!(!sw.is_active());
    }
}
