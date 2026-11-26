//! The binding engine: turns a stream of ControllerState snapshots into
//! OutputEvents according to the loaded config's bindings, layers, and
//! macros.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use padshell_core::{ButtonId, ButtonSet, ControllerState, MouseButton, OutputEvent, StickId};

use crate::config::{Action, Binding, Config, InputSpec};
use crate::curve::{apply_deadzone, axis_to_velocity};
use crate::layer::LayerSwitch;
use crate::macro_::{self, PlaybackItem};

/// Output of one engine tick: either an immediate event to inject, or a
/// macro's expanded playback timeline for the caller to schedule (macros
/// have delays, so they aren't emitted synchronously).
pub enum EngineAction {
    Emit(OutputEvent),
    PlayMacro(Vec<PlaybackItem>),
    /// Spawn a shell command, detached.
    Launch(String),
}

struct LayerRuntime {
    switch: LayerSwitch,
    definition: crate::config::Layer,
}

/// Accumulates sub-pixel motion across ticks so slow stick deflection
/// (or a low scroll sensitivity) still eventually emits whole-unit
/// deltas instead of always rounding to zero.
#[derive(Default)]
struct MotionAccumulator {
    x: f32,
    y: f32,
}

impl MotionAccumulator {
    /// Adds a fractional delta and returns the whole-unit part to emit
    /// now, keeping the remainder for next time.
    fn step(&mut self, dx: f32, dy: f32) -> (f32, f32) {
        self.x += dx;
        self.y += dy;
        let out_x = self.x.trunc();
        let out_y = self.y.trunc();
        self.x -= out_x;
        self.y -= out_y;
        (out_x, out_y)
    }
}

pub struct BindingEngine {
    config: Config,
    layers: Vec<LayerRuntime>,
    prev_buttons: ButtonSet,
    /// Mouse buttons currently held down, keyed by the physical
    /// controller button that pressed them, so a release fires even if
    /// the layer that bound it has since deactivated (avoids a stuck
    /// virtual mouse button when the user releases the layer-hold key
    /// before the mouse button).
    held_mouse_buttons: HashMap<ButtonId, MouseButton>,
    mouse_move_accum: MotionAccumulator,
    scroll_accum: MotionAccumulator,
}

impl BindingEngine {
    pub fn new(config: Config) -> Self {
        let layers = config
            .layers
            .iter()
            .map(|layer| LayerRuntime {
                switch: LayerSwitch::new(layer.activate.mode),
                definition: layer.clone(),
            })
            .collect();
        BindingEngine {
            config,
            layers,
            prev_buttons: ButtonSet::empty(),
            held_mouse_buttons: HashMap::new(),
            mouse_move_accum: MotionAccumulator::default(),
            scroll_accum: MotionAccumulator::default(),
        }
    }

    /// Replace the config in place (used for hot reload), returning the
    /// releases for any mouse buttons the old config was holding down so
    /// a reload mid-drag can't leave one stuck. Layer switch state
    /// resets, which is fine - a config edit is a rare, deliberate event
    /// and stale "held" state across a reload would be surprising. The
    /// last-seen buttons carry over, so a button that's still held
    /// doesn't count as a fresh press under the new config.
    pub fn reload(&mut self, config: Config) -> Vec<EngineAction> {
        let releases = self
            .held_mouse_buttons
            .values()
            .map(|&mb| EngineAction::Emit(OutputEvent::MouseButtonUp(mb)))
            .collect();
        let prev_buttons = self.prev_buttons;
        *self = BindingEngine::new(config);
        self.prev_buttons = prev_buttons;
        releases
    }

    /// Marks `buttons` as already seen, so ones still held don't count
    /// as fresh presses on the next tick (used when output resumes after
    /// a pause, during which ticks saw a neutral controller).
    pub fn sync_buttons(&mut self, buttons: ButtonSet) {
        self.prev_buttons = buttons;
    }

    /// True if `state` holds a stick deflected in a way the active layer
    /// turns into mouse motion or scrolling. Motion is integrated over
    /// time, so the caller must keep ticking at a steady rate while this
    /// holds even if the controller sends nothing new (evdev pads only
    /// report changes, and a stick held still doesn't change).
    pub fn wants_timed_ticks(&self, state: &ControllerState) -> bool {
        let Some(layer) = self.layers.iter().rev().find(|l| l.switch.is_active()) else {
            return false;
        };
        let deadzone = self.config.settings.stick_deadzone;
        let live = |value: f32| apply_deadzone(value, deadzone) != 0.0;
        let pos = |stick: &str| {
            StickId::from_str(stick).map(|id| match id {
                StickId::Left => state.left_stick,
                StickId::Right => state.right_stick,
            })
        };
        let def = &layer.definition;
        let mouse = def.stick_mouse.as_ref().and_then(|sm| pos(&sm.stick)).is_some_and(|p| live(p.x) || live(p.y));
        let scroll = def
            .scroll
            .as_ref()
            .and_then(|sc| pos(&sc.stick).map(|p| if sc.axis == "x" { p.x } else { p.y }))
            .is_some_and(live);
        mouse || scroll
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Process one controller state snapshot. `dt` is the time elapsed
    /// since the previous tick, used for mouse-mode velocity integration.
    pub fn tick(&mut self, state: &ControllerState, dt: Duration) -> Vec<EngineAction> {
        let mut actions = Vec::new();
        let prev_buttons = self.prev_buttons;
        let cur_buttons = state.buttons;

        // Release any mouse button whose triggering controller button is
        // no longer physically pressed, regardless of layer state - this
        // is what makes press-and-hold-to-drag work, and avoids a stuck
        // virtual button if the user releases the layer-hold key first.
        self.held_mouse_buttons.retain(|&trigger, &mut mb| {
            if cur_buttons.is_pressed(trigger) {
                true
            } else {
                actions.push(EngineAction::Emit(OutputEvent::MouseButtonUp(mb)));
                false
            }
        });

        // Update layer activation state first, so this tick's bindings
        // already reflect any layer switch triggered by this tick's edge.
        let deadzone = self.config.settings.stick_deadzone;
        for layer_rt in &mut self.layers {
            let Ok(button) = layer_rt.definition.activate.button_id() else {
                continue;
            };
            let is_pressed = cur_buttons.is_pressed(button);
            let is_press_edge = is_pressed && !prev_buttons.is_pressed(button);
            layer_rt.switch.on_tick(is_pressed, is_press_edge);
        }

        // Active layer bindings take priority over the default layer;
        // last-activated wins if somehow multiple are active at once.
        let mut handled: HashSet<String> = HashSet::new();
        for layer_rt in self.layers.iter().rev() {
            if !layer_rt.switch.is_active() {
                continue;
            }
            dispatch_bindings(
                &self.config,
                &mut self.held_mouse_buttons,
                &layer_rt.definition.bindings,
                &prev_buttons,
                &cur_buttons,
                &mut handled,
                &mut actions,
            );
            if let Some(sm) = &layer_rt.definition.stick_mouse {
                emit_stick_mouse(sm, state, deadzone, dt, &mut self.mouse_move_accum, &mut actions);
            }
            if let Some(sc) = &layer_rt.definition.scroll {
                emit_scroll(sc, state, deadzone, dt, &mut self.scroll_accum, &mut actions);
            }
            // Only the highest-priority active layer's bindings apply.
            break;
        }

        dispatch_bindings(
            &self.config,
            &mut self.held_mouse_buttons,
            &self.config.bindings,
            &prev_buttons,
            &cur_buttons,
            &mut handled,
            &mut actions,
        );

        self.prev_buttons = cur_buttons;
        actions
    }
}

/// Dispatches a binding list's press-edge actions. A free function (not
/// a method) because it needs to mutably borrow `held_mouse_buttons`
/// while `self.config` is only borrowed immutably, which a method
/// taking `&mut self` can't express.
fn dispatch_bindings(
    config: &Config,
    held_mouse_buttons: &mut HashMap<ButtonId, MouseButton>,
    bindings: &[Binding],
    prev: &ButtonSet,
    cur: &ButtonSet,
    handled: &mut HashSet<String>,
    actions: &mut Vec<EngineAction>,
) {
    // Combos first, so their member buttons don't also fire their
    // single-button bindings on the same edge.
    for binding in bindings.iter().filter(|b| is_combo(&b.input)) {
        if combo_press_edge(&binding.input, prev, cur) {
            for name in binding.input.buttons() {
                handled.insert(name.to_string());
            }
            // Combos always tap (down+up together) - hold-to-drag via a
            // combo isn't supported in v1, only single-button bindings.
            fire_action_tap(config, &binding.action, actions);
        }
    }

    for binding in bindings.iter().filter(|b| !is_combo(&b.input)) {
        let InputSpec::Single(name) = &binding.input else {
            continue;
        };
        let Some(button) = ButtonId::from_str(name) else {
            continue;
        };
        if handled.contains(name) {
            continue;
        }
        if cur.is_pressed(button) && !prev.is_pressed(button) {
            if let Action::MouseButton { button: mb_name } = &binding.action {
                if let Some(mb) = MouseButton::from_str(mb_name) {
                    actions.push(EngineAction::Emit(OutputEvent::MouseButtonDown(mb)));
                    held_mouse_buttons.insert(button, mb);
                } else {
                    tracing::warn!(button = %mb_name, "unknown mouse button name in config");
                }
            } else {
                fire_action_tap(config, &binding.action, actions);
            }
        }
    }
}

fn fire_action_tap(config: &Config, action: &Action, actions: &mut Vec<EngineAction>) {
    match action {
        Action::Key { key } => {
            actions.push(EngineAction::Emit(OutputEvent::KeyTap(vec![key.clone()])));
        }
        Action::KeyCombo { keys } => {
            actions.push(EngineAction::Emit(OutputEvent::KeyTap(keys.clone())));
        }
        Action::Macro { name } => {
            if let Some(macro_def) = config.macros.iter().find(|m| &m.name == name) {
                actions.push(EngineAction::PlayMacro(macro_::expand(macro_def)));
            } else {
                tracing::warn!(macro_name = %name, "binding references unknown macro");
            }
        }
        Action::Launch { command } => {
            actions.push(EngineAction::Launch(command.clone()));
        }
        Action::MouseButton { button } => {
            if let Some(mb) = MouseButton::from_str(button) {
                actions.push(EngineAction::Emit(OutputEvent::MouseButtonDown(mb)));
                actions.push(EngineAction::Emit(OutputEvent::MouseButtonUp(mb)));
            } else {
                tracing::warn!(button = %button, "unknown mouse button name in config");
            }
        }
    }
}

fn is_combo(input: &InputSpec) -> bool {
    matches!(input, InputSpec::Combo { .. })
}

fn combo_press_edge(input: &InputSpec, prev: &ButtonSet, cur: &ButtonSet) -> bool {
    let buttons: Vec<ButtonId> = input.buttons().iter().filter_map(|s| ButtonId::from_str(s)).collect();
    if buttons.is_empty() {
        return false;
    }
    let all_pressed_now = buttons.iter().all(|&b| cur.is_pressed(b));
    let all_pressed_before = buttons.iter().all(|&b| prev.is_pressed(b));
    all_pressed_now && !all_pressed_before
}

fn emit_stick_mouse(
    sm: &crate::config::StickMouse,
    state: &ControllerState,
    deadzone: f32,
    dt: Duration,
    accum: &mut MotionAccumulator,
    actions: &mut Vec<EngineAction>,
) {
    let Some(stick) = StickId::from_str(&sm.stick) else {
        return;
    };
    let pos = match stick {
        StickId::Left => state.left_stick,
        StickId::Right => state.right_stick,
    };
    let x = apply_deadzone(pos.x, deadzone);
    let y = apply_deadzone(pos.y, deadzone);
    if x == 0.0 && y == 0.0 {
        return;
    }
    let dt_secs = dt.as_secs_f32();
    let dx = axis_to_velocity(x, sm.sensitivity, &sm.accel_curve) * dt_secs;
    let dy = axis_to_velocity(y, sm.sensitivity, &sm.accel_curve) * dt_secs;
    // Accumulate sub-pixel motion across ticks so slow deflection still
    // eventually moves the cursor instead of always rounding to zero.
    let (dx, dy) = accum.step(dx, dy);
    if dx != 0.0 || dy != 0.0 {
        actions.push(EngineAction::Emit(OutputEvent::MouseMove { dx, dy }));
    }
}

fn emit_scroll(
    sc: &crate::config::Scroll,
    state: &ControllerState,
    deadzone: f32,
    dt: Duration,
    accum: &mut MotionAccumulator,
    actions: &mut Vec<EngineAction>,
) {
    let Some(stick) = StickId::from_str(&sc.stick) else {
        return;
    };
    let pos = match stick {
        StickId::Left => state.left_stick,
        StickId::Right => state.right_stick,
    };
    let axis_value = if sc.axis == "x" { pos.x } else { pos.y };
    let value = apply_deadzone(axis_value, deadzone);
    if value == 0.0 {
        return;
    }
    let dt_secs = dt.as_secs_f32();
    let delta = value * sc.sensitivity * dt_secs;
    let (dx, dy) = if sc.axis == "x" {
        accum.step(delta, 0.0)
    } else {
        accum.step(0.0, delta)
    };
    if dx != 0.0 || dy != 0.0 {
        actions.push(EngineAction::Emit(OutputEvent::Scroll { dx, dy }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn state_with_buttons(buttons: &[ButtonId]) -> ControllerState {
        let mut bs = ButtonSet::empty();
        for &b in buttons {
            bs.set(b, true);
        }
        ControllerState {
            buttons: bs,
            ..Default::default()
        }
    }

    #[test]
    fn single_binding_fires_on_press_edge_only() {
        let config = Config::from_str(
            r#"
schema_version = 1
[[bindings]]
input = "cross"
action = { type = "key", key = "space" }
"#,
        )
        .unwrap();
        let mut engine = BindingEngine::new(config);

        let pressed = state_with_buttons(&[ButtonId::Cross]);
        let actions = engine.tick(&pressed, Duration::from_millis(4));
        assert_eq!(actions.len(), 1);

        // holding, no new edge -> no repeat
        let actions = engine.tick(&pressed, Duration::from_millis(4));
        assert_eq!(actions.len(), 0);
    }

    #[test]
    fn launch_binding_yields_launch_action_once() {
        let config = Config::from_str(
            r#"
schema_version = 1
[[bindings]]
input = "options"
action = { type = "launch", command = "firefox" }
"#,
        )
        .unwrap();
        let mut engine = BindingEngine::new(config);
        let pressed = state_with_buttons(&[ButtonId::Options]);
        let actions = engine.tick(&pressed, Duration::from_millis(4));
        assert!(matches!(actions.as_slice(), [EngineAction::Launch(c)] if c == "firefox"));
        assert!(engine.tick(&pressed, Duration::from_millis(4)).is_empty());
    }

    #[test]
    fn combo_fires_instead_of_member_single_bindings() {
        let config = Config::from_str(
            r#"
schema_version = 1
[[bindings]]
input = { combo = ["l1", "r1"] }
action = { type = "key_combo", keys = ["super", "q"] }

[[bindings]]
input = "l1"
action = { type = "key", key = "a" }
"#,
        )
        .unwrap();
        let mut engine = BindingEngine::new(config);

        let both = state_with_buttons(&[ButtonId::L1, ButtonId::R1]);
        let actions = engine.tick(&both, Duration::from_millis(4));
        assert_eq!(actions.len(), 1); // only the combo fires, not the l1 single binding
    }

    #[test]
    fn hold_layer_activates_and_deactivates() {
        let config = Config::from_str(
            r#"
schema_version = 1
[[layers]]
name = "mouse"
activate = { button = "l1", mode = "hold" }

  [[layers.bindings]]
  input = "r2"
  action = { type = "mouse_button", button = "left" }
"#,
        )
        .unwrap();
        let mut engine = BindingEngine::new(config);

        let l1_and_r2 = state_with_buttons(&[ButtonId::L1, ButtonId::R2]);
        let actions = engine.tick(&l1_and_r2, Duration::from_millis(4));
        assert_eq!(actions.len(), 1); // mouse button down only (held, not tapped)

        // still held - no repeat
        let actions = engine.tick(&l1_and_r2, Duration::from_millis(4));
        assert_eq!(actions.len(), 0);

        // r2 released (l1 still held) -> button up fires
        let l1_only = state_with_buttons(&[ButtonId::L1]);
        let actions = engine.tick(&l1_only, Duration::from_millis(4));
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn mouse_button_releases_even_if_layer_deactivates_first() {
        let config = Config::from_str(
            r#"
schema_version = 1
[[layers]]
name = "mouse"
activate = { button = "l1", mode = "hold" }

  [[layers.bindings]]
  input = "r2"
  action = { type = "mouse_button", button = "left" }
"#,
        )
        .unwrap();
        let mut engine = BindingEngine::new(config);

        let l1_and_r2 = state_with_buttons(&[ButtonId::L1, ButtonId::R2]);
        engine.tick(&l1_and_r2, Duration::from_millis(4));

        // user releases l1 first, dropping out of the mouse layer, while
        // still physically holding r2 - the drag should keep going
        // (button stays down) rather than releasing just because the
        // layer that originally bound it deactivated.
        let r2_only = state_with_buttons(&[ButtonId::R2]);
        let actions = engine.tick(&r2_only, Duration::from_millis(4));
        assert_eq!(actions.len(), 0);

        // now r2 itself releases -> the tracked mouse button releases,
        // regardless of which layer (if any) is currently active.
        let none = state_with_buttons(&[]);
        let actions = engine.tick(&none, Duration::from_millis(4));
        assert_eq!(actions.len(), 1);
    }

    const MOUSE_LAYER: &str = r#"
schema_version = 1
[[layers]]
name = "mouse"
activate = { button = "l1", mode = "hold" }
stick_mouse = { stick = "right", sensitivity = 900.0 }

  [[layers.bindings]]
  input = "r2"
  action = { type = "mouse_button", button = "left" }

[[bindings]]
input = "cross"
action = { type = "key", key = "space" }
"#;

    #[test]
    fn reload_releases_held_mouse_button_and_does_not_refire_held_buttons() {
        let mut engine = BindingEngine::new(Config::from_str(MOUSE_LAYER).unwrap());
        let held = state_with_buttons(&[ButtonId::L1, ButtonId::R2, ButtonId::Cross]);
        engine.tick(&held, Duration::from_millis(4));

        let releases = engine.reload(Config::from_str(MOUSE_LAYER).unwrap());
        assert!(matches!(
            releases.as_slice(),
            [EngineAction::Emit(OutputEvent::MouseButtonUp(MouseButton::Left))]
        ));

        // Same buttons still held after the reload: nothing is a new press.
        assert!(engine.tick(&held, Duration::from_millis(4)).is_empty());
    }

    #[test]
    fn sync_buttons_suppresses_press_edges_for_held_buttons() {
        let mut engine = BindingEngine::new(Config::from_str(MOUSE_LAYER).unwrap());
        let cross = state_with_buttons(&[ButtonId::Cross]);
        engine.sync_buttons(cross.buttons);
        assert!(engine.tick(&cross, Duration::from_millis(4)).is_empty());

        // Released and pressed again -> fires normally.
        engine.tick(&state_with_buttons(&[]), Duration::from_millis(4));
        assert_eq!(engine.tick(&cross, Duration::from_millis(4)).len(), 1);
    }

    #[test]
    fn timed_ticks_wanted_only_for_live_stick_on_active_layer() {
        let mut engine = BindingEngine::new(Config::from_str(MOUSE_LAYER).unwrap());
        let mut state = state_with_buttons(&[]);
        state.right_stick.x = 0.9;

        // Stick deflected but the mouse layer isn't active.
        engine.tick(&state, Duration::from_millis(4));
        assert!(!engine.wants_timed_ticks(&state));

        state.buttons.set(ButtonId::L1, true);
        engine.tick(&state, Duration::from_millis(4));
        assert!(engine.wants_timed_ticks(&state));

        // Inside the deadzone, and the stick the layer doesn't use.
        state.right_stick.x = 0.01;
        state.left_stick.y = 0.9;
        assert!(!engine.wants_timed_ticks(&state));
    }
}
