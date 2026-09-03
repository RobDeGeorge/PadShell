use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use padshell_binding::{BindingEngine, Config, EngineAction};
use padshell_hid::device::{self, HidrawDevice};
use padshell_hid::ds4::{Ds4UsbParser, DS4_PRODUCT_ID_V1, DS4_PRODUCT_ID_V2, DS4_VENDOR_ID};
use padshell_hid::ControllerReportParser;
use padshell_output::OutputSink;

use crate::config_watch::ConfigWatcher;

pub struct RunOptions {
    pub config_path: PathBuf,
    /// If true, decoded controller state is logged and no output events
    /// are injected - useful for verifying HID decoding without an
    /// active virtual device.
    pub dump_only: bool,
}

pub fn run(opts: RunOptions) -> anyhow::Result<()> {
    let config = Config::load(&opts.config_path)
        .map_err(|e| anyhow::anyhow!("failed to load config {:?}: {e}", opts.config_path))?;
    let mut engine = BindingEngine::new(config);
    let mut watcher = ConfigWatcher::new(&opts.config_path)?;

    let sink = if opts.dump_only {
        None
    } else {
        Some(Arc::new(OutputSink::create()?))
    };

    let hidraw_info = find_ds4()?;
    tracing::info!(path = ?hidraw_info.path, "opening DS4");
    let mut hidraw = HidrawDevice::open(&hidraw_info.path)?;
    let parser = Ds4UsbParser::new();

    let mut buf = vec![0u8; parser.report_len()];
    let mut last_tick = Instant::now();

    loop {
        if let Some(new_config) = watcher.poll_reload() {
            tracing::info!("config changed, reloading");
            engine.reload(new_config);
        }

        let n = hidraw.read_report(&mut buf)?;
        let Some(state) = parser.parse(&buf[..n]) else {
            tracing::trace!(len = n, "discarded unparseable report");
            continue;
        };

        let now = Instant::now();
        let dt = now.duration_since(last_tick);
        last_tick = now;

        if opts.dump_only {
            tracing::info!(?state, "controller state");
            continue;
        }

        let actions = engine.tick(&state, dt);
        let Some(sink) = &sink else { continue };
        for action in actions {
            match action {
                EngineAction::Emit(event) => {
                    if let Err(err) = sink.apply(&event) {
                        tracing::warn!(error = %err, "failed to emit output event");
                    }
                }
                EngineAction::PlayMacro(items) => {
                    let sink = Arc::clone(sink);
                    std::thread::spawn(move || play_macro(sink, items));
                }
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

fn find_ds4() -> anyhow::Result<device::HidrawInfo> {
    device::find(DS4_VENDOR_ID, DS4_PRODUCT_ID_V1)
        .or_else(|_| device::find(DS4_VENDOR_ID, DS4_PRODUCT_ID_V2))
        .map_err(|e| {
            anyhow::anyhow!(
                "no DS4 controller found ({e}). Is it connected via USB? \
                 Run `padshell devices` to list available hidraw devices."
            )
        })
}
