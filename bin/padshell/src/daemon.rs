use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use padshell_core::ControllerState;
use padshell_binding::{BindingEngine, Config, EngineAction, HapticsMode, HapticsState};
use padshell_hid::controller::{self, ControllerInfo};
use padshell_hid::{AnimationHandle, HapticsController, LightbarAnimation};
use padshell_output::OutputSink;

use crate::config_watch::ConfigWatcher;
use crate::focus::FocusWatcher;

/// How often the supervisor re-scans hidraw devices for controllers that
/// have newly appeared or gone away. A controller unplug/replug (or a
/// Bluetooth drop/reconnect) is picked up within one tick of this.
const SCAN_INTERVAL: Duration = Duration::from_millis(750);

/// How often a worker re-reads its controller's battery level.
const BATTERY_POLL: Duration = Duration::from_secs(15);

/// Longest a worker blocks waiting for input before checking for config
/// reloads / battery. Only matters for pads that go quiet when idle
/// (evdev/Xbox) - a DS4 streams reports far faster than this.
const READ_TIMEOUT: Duration = Duration::from_millis(200);

/// Longest gap a single tick integrates stick motion over. A pad that
/// was idle (or a stalled read) would otherwise turn the whole gap into
/// one cursor jump on the next report.
const MAX_TICK_DT: Duration = Duration::from_millis(50);

pub struct RunOptions {
    /// Root config directory (default `~/.config/padshell`). Each
    /// controller gets its own profile under `<config_dir>/devices/`.
    pub config_dir: PathBuf,
    /// If true, decoded controller state is logged and no output events
    /// are injected - useful for verifying HID decoding without an
    /// active virtual device. Bypasses the multi-device supervisor and
    /// just dumps the first controller found, since this is a debug
    /// tool rather than the real run path.
    pub dump_only: bool,
    /// With dump_only, print only when the button set changes instead of
    /// every report - much easier to read while pressing buttons live.
    pub changes_only: bool,
}

pub fn run(opts: RunOptions) -> anyhow::Result<()> {
    if opts.dump_only {
        return run_dump_only(&opts);
    }

    migrate_legacy_config(&opts.config_dir)?;

    let sink = Arc::new(OutputSink::create()?);
    let focus = FocusWatcher::spawn();
    let mut workers: HashMap<String, JoinHandle<()>> = HashMap::new();

    loop {
        let connected = controller::enumerate();

        // Reap workers for controllers that disconnected (or whose read
        // loop errored out) since the last scan.
        workers.retain(|device_id, handle| {
            if handle.is_finished() {
                tracing::info!(device_id, "controller disconnected");
                false
            } else {
                let _ = device_id;
                true
            }
        });

        for info in connected {
            let device_id = info.device_id();
            if workers.contains_key(&device_id) {
                continue;
            }
            tracing::info!(
                device_id,
                kind = %info.kind.as_str(),
                name = %info.name,
                path = ?info.input_path(),
                bus = ?info.bus,
                "controller connected"
            );

            let config_dir = opts.config_dir.clone();
            let sink = Arc::clone(&sink);
            let focus = focus.clone();
            let id_for_thread = device_id.clone();
            let handle = std::thread::spawn(move || {
                if let Err(err) = run_worker(&info, &id_for_thread, &config_dir, sink, &focus) {
                    tracing::warn!(device_id = %id_for_thread, error = %err, "controller worker exited");
                }
            });
            workers.insert(device_id, handle);
        }

        std::thread::sleep(SCAN_INTERVAL);
    }
}

/// Runs one controller's whole lifecycle: load/create its profile, watch
/// it for hot-reload, reapply its saved lightbar state, then read and
/// dispatch reports until the device errors out (typically because it
/// was unplugged), at which point this returns and the supervisor reaps
/// it - reconnecting later spawns a fresh worker that repeats all of
/// this, which is what makes reconnect naturally reapply saved state.
fn run_worker(
    info: &ControllerInfo,
    device_id: &str,
    config_dir: &Path,
    sink: Arc<OutputSink>,
    focus: &FocusWatcher,
) -> anyhow::Result<()> {
    let (mut config, profile_path) = Config::load_or_init_profile(config_dir, device_id)
        .map_err(|e| anyhow::anyhow!("failed to load profile for {device_id}: {e}"))?;
    if let Err(err) = config.ensure_device_identity(&profile_path, info.vendor_id, info.product_id, info.kind, &info.name) {
        tracing::warn!(device_id, error = %err, "could not record controller kind in profile");
    }

    let mut lightbar = Lightbar::new(info);
    let mut battery_state = BatteryMonitor::new(info);
    battery_state.refresh(&config, &mut lightbar, device_id);

    let mut engine = BindingEngine::new(config);
    let mut watcher = ConfigWatcher::new(&profile_path)?;

    // Opened without grabbing: RetroArch/SDL/Steam read the same node.
    let mut source = info.open()?;
    let mut last_tick = Instant::now();
    let mut last_state = ControllerState::default();
    let mut paused = false;

    loop {
        if let Some(new_config) = watcher.poll_reload() {
            tracing::info!(device_id, "config changed, reloading");
            let releases = engine.reload(new_config);
            dispatch(releases, &sink, device_id);
            battery_state.force_refresh(engine.config(), &mut lightbar, device_id);
        }

        let now = Instant::now();
        if battery_state.due(now) {
            battery_state.refresh(engine.config(), &mut lightbar, device_id);
        }

        // A focused game reads the pad itself, so stand down: one neutral
        // tick releases any held mouse button, then nothing is emitted
        // until focus moves elsewhere. Buttons still held at that point
        // are not treated as fresh presses.
        let should_pause = focus.matches(&engine.config().settings.pause_when_focused);
        if should_pause != paused {
            paused = should_pause;
            tracing::info!(device_id, paused, "focused window changed padshell output");
            if paused {
                let releases = engine.tick(&ControllerState::default(), Duration::ZERO);
                dispatch(releases, &sink, device_id);
            } else {
                engine.sync_buttons(last_state.buttons);
            }
        }

        // Stick-driven mouse motion is integrated over time, so while a
        // stick is producing it, tick at the configured rate even if the
        // pad sends nothing new (evdev pads only report changes).
        let timed = !paused && engine.wants_timed_ticks(&last_state);
        let timeout = if timed {
            Duration::from_secs(1) / engine.config().settings.poll_hz.clamp(1, 1000)
        } else {
            READ_TIMEOUT
        };
        let fresh = source.next_state(timeout)?;

        let now = Instant::now();
        let dt = now.duration_since(last_tick).min(MAX_TICK_DT);
        last_tick = now;

        match fresh {
            Some(state) => last_state = state,
            None if timed => {}
            None => continue,
        }
        if paused {
            continue;
        }
        dispatch(engine.tick(&last_state, dt), &sink, device_id);
    }
}

/// Carries out one tick's worth of engine output.
fn dispatch(actions: Vec<EngineAction>, sink: &Arc<OutputSink>, device_id: &str) {
    for action in actions {
        match action {
            EngineAction::Emit(event) => {
                if let Err(err) = sink.apply(&event) {
                    tracing::warn!(device_id, error = %err, "failed to emit output event");
                }
            }
            EngineAction::PlayMacro(items) => {
                let sink = Arc::clone(sink);
                std::thread::spawn(move || play_macro(sink, items));
            }
            EngineAction::Launch(command) => launch(device_id, &command),
        }
    }
}

/// Spawns `sh -c command` in its own process group with stdio detached,
/// so the app outlives the daemon and never blocks the read loop.
fn launch(device_id: &str, command: &str) {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    tracing::info!(device_id, command, "launching");
    let result = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    match result {
        Ok(mut child) => {
            // Reap in the background so the child doesn't linger as a zombie.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(err) => tracing::warn!(device_id, command, error = %err, "launch failed"),
    }
}

/// Owns one controller's lightbar so that switching state (config
/// reload, battery alert starting/ending) stops the previous animation
/// thread instead of stacking a second one on top of it.
struct Lightbar {
    controller: HapticsController,
    animation: Option<AnimationHandle>,
    applied: Option<HapticsState>,
}

impl Lightbar {
    fn new(info: &ControllerInfo) -> Self {
        Lightbar {
            controller: info.haptics(),
            animation: None,
            applied: None,
        }
    }

    /// Pushes a lightbar/animation state to the hardware if it differs
    /// from what's currently applied.
    fn apply(&mut self, haptics: &HapticsState) {
        if self.applied.as_ref() == Some(haptics) {
            return;
        }
        if let Some(handle) = self.animation.take() {
            handle.stop();
        }
        self.applied = Some(haptics.clone());
        if !self.controller.has_lightbar() {
            return;
        }
        match haptics.mode {
            HapticsMode::Off => {}
            HapticsMode::Solid => {
                let _ = self.controller.set_lightbar(haptics.r, haptics.g, haptics.b);
            }
            HapticsMode::Rainbow => {
                self.animation =
                    Some(self.controller.start_animation(LightbarAnimation::Rainbow { period_ms: haptics.period_ms }));
            }
            HapticsMode::Fade => {
                self.animation = Some(self.controller.start_animation(LightbarAnimation::Fade {
                    r: haptics.r,
                    g: haptics.g,
                    b: haptics.b,
                    period_ms: haptics.period_ms,
                }));
            }
            HapticsMode::Strobe => {
                self.animation = Some(self.controller.start_animation(LightbarAnimation::Strobe {
                    r: haptics.r,
                    g: haptics.g,
                    b: haptics.b,
                    period_ms: haptics.period_ms,
                }));
            }
        }
    }

    fn rumble(&self) {
        if self.controller.has_rumble() {
            let _ = self.controller.rumble(40000, 40000, 400);
        }
    }
}

impl Drop for Lightbar {
    fn drop(&mut self) {
        if let Some(handle) = self.animation.take() {
            handle.stop();
        }
    }
}

/// Polls the battery and decides which lightbar state should be showing:
/// the lowest matching battery alert, or the profile's normal state.
struct BatteryMonitor {
    syspath: PathBuf,
    next_poll: Instant,
    /// Index of the alert currently active, if any.
    active_alert: Option<usize>,
}

impl BatteryMonitor {
    fn new(info: &ControllerInfo) -> Self {
        BatteryMonitor {
            syspath: info.syspath.clone(),
            next_poll: Instant::now(),
            active_alert: None,
        }
    }

    fn due(&self, now: Instant) -> bool {
        now >= self.next_poll
    }

    /// Config changed: forget which alert was active so a changed alert
    /// re-applies, then re-evaluate immediately.
    fn force_refresh(&mut self, config: &Config, lightbar: &mut Lightbar, device_id: &str) {
        self.active_alert = None;
        self.refresh(config, lightbar, device_id);
    }

    fn refresh(&mut self, config: &Config, lightbar: &mut Lightbar, device_id: &str) {
        self.next_poll = Instant::now() + BATTERY_POLL;
        let status = padshell_hid::battery::read(&self.syspath);
        let matched = status.filter(|s| !s.charging).and_then(|s| {
            config
                .battery_alerts
                .iter()
                .enumerate()
                .filter(|(_, a)| s.percent <= a.below_percent)
                .min_by_key(|(_, a)| a.below_percent)
                .map(|(i, _)| i)
        });

        match matched {
            Some(i) => {
                let alert = &config.battery_alerts[i];
                if self.active_alert != Some(i) {
                    tracing::info!(
                        device_id,
                        percent = status.map(|s| s.percent),
                        threshold = alert.below_percent,
                        "battery alert active"
                    );
                    if alert.rumble {
                        lightbar.rumble();
                    }
                }
                self.active_alert = Some(i);
                lightbar.apply(&alert.lightbar);
            }
            None => {
                if self.active_alert.is_some() {
                    tracing::info!(device_id, "battery alert cleared");
                }
                self.active_alert = None;
                lightbar.apply(&config.haptics);
            }
        }
    }
}

fn play_macro(sink: Arc<OutputSink>, items: Vec<padshell_binding::macro_::PlaybackItem>) {
    for item in items {
        match item {
            padshell_binding::macro_::PlaybackItem::Emit(event) => {
                if let Err(err) = sink.apply(&event) {
                    tracing::warn!(error = %err, "macro step failed");
                }
            }
            padshell_binding::macro_::PlaybackItem::Wait(duration) => {
                std::thread::sleep(duration);
            }
        }
    }
}

/// One-time migration for existing users: a legacy single
/// `~/.config/padshell/config.toml` becomes the new `default.toml`
/// template that seeds every newly-seen controller's profile.
fn migrate_legacy_config(config_dir: &Path) -> anyhow::Result<()> {
    let legacy = config_dir.join("config.toml");
    let default_path = config_dir.join("default.toml");
    if legacy.exists() && !default_path.exists() {
        std::fs::create_dir_all(config_dir)?;
        std::fs::copy(&legacy, &default_path)?;
        tracing::info!("migrated legacy config.toml to default.toml");
    }
    Ok(())
}

fn run_dump_only(opts: &RunOptions) -> anyhow::Result<()> {
    let info = controller::first()
        .ok_or_else(|| anyhow::anyhow!("no controller found. Run `padshell devices` to list what was detected."))?;
    tracing::info!(kind = %info.kind.as_str(), name = %info.name, path = ?info.input_path(), bus = ?info.bus, "opening controller");
    let mut source = info.open()?;
    let mut last_buttons = padshell_core::ButtonSet::empty();

    loop {
        let Some(state) = source.next_state(Duration::from_secs(1))? else {
            continue;
        };

        if opts.changes_only {
            use std::io::Write;
            print!(
                "\r\x1b[K L({:+.2},{:+.2}) R({:+.2},{:+.2}) L2:{:.2} R2:{:.2}  {:?}",
                state.left_stick.x,
                state.left_stick.y,
                state.right_stick.x,
                state.right_stick.y,
                state.l2_analog,
                state.r2_analog,
                state.buttons,
            );
            std::io::stdout().flush().ok();
            if state.buttons != last_buttons {
                println!();
                last_buttons = state.buttons;
            }
        } else {
            tracing::info!(?state, "controller state");
        }
    }
}
