//! Macro playback: turns a config-defined step sequence into a flat list
//! of OutputEvents with delays resolved to durations, for the engine (or
//! a dedicated worker thread) to play back.

use crate::config::{Macro, MacroStep};
use padshell_core::OutputEvent;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum PlaybackItem {
    Emit(OutputEvent),
    Wait(Duration),
}

const DEFAULT_HOLD_MS: u64 = 20;

/// Expands a macro definition into a flat playback timeline.
pub fn expand(macro_def: &Macro) -> Vec<PlaybackItem> {
    let mut items = Vec::new();
    for step in &macro_def.steps {
        match step {
            MacroStep::Key { key, hold_ms } => {
                items.push(PlaybackItem::Emit(OutputEvent::KeyTap(vec![key.clone()])));
                items.push(PlaybackItem::Wait(Duration::from_millis(
                    hold_ms.unwrap_or(DEFAULT_HOLD_MS),
                )));
            }
            MacroStep::KeyCombo { keys, hold_ms } => {
                items.push(PlaybackItem::Emit(OutputEvent::KeyTap(keys.clone())));
                items.push(PlaybackItem::Wait(Duration::from_millis(
                    hold_ms.unwrap_or(DEFAULT_HOLD_MS),
                )));
            }
            MacroStep::Delay { ms } => {
                items.push(PlaybackItem::Wait(Duration::from_millis(*ms)));
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_key_and_delay_steps() {
        let macro_def = Macro {
            name: "greeting".into(),
            steps: vec![
                MacroStep::Key {
                    key: "h".into(),
                    hold_ms: None,
                },
                MacroStep::Delay { ms: 200 },
            ],
        };
        let items = expand(&macro_def);
        assert_eq!(items.len(), 3); // emit + default wait + explicit delay
    }
}
