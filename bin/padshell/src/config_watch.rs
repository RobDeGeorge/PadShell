//! Watches the config file's parent directory for changes and re-parses
//! on write/rename (editors often save via atomic rename, which doesn't
//! fire a plain "modify" event on the file itself).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};

pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
    rx: Receiver<()>,
    config_path: PathBuf,
    /// When the most recent change to the file was seen, if it hasn't
    /// been reloaded yet.
    pending_since: Option<Instant>,
}

/// How long the file must stay quiet before it's re-read, so a save made
/// of several events (truncate + write, or write + rename) is parsed
/// once, after the last of them.
const SETTLE: Duration = Duration::from_millis(200);

impl ConfigWatcher {
    pub fn new(config_path: &Path) -> anyhow::Result<Self> {
        let (tx, rx) = channel();
        let watch_target = config_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        // The directory holds every controller's profile; only events
        // naming this one matter.
        let file_name = config_path.file_name().map(|n| n.to_os_string());
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if event.paths.iter().any(|p| p.file_name() == file_name.as_deref()) {
                    let _ = tx.send(());
                }
            }
        })?;
        watcher.watch(watch_target, RecursiveMode::NonRecursive)?;

        Ok(ConfigWatcher {
            _watcher: watcher,
            rx,
            config_path: config_path.to_path_buf(),
            pending_since: None,
        })
    }

    /// Non-blocking check for a pending reload. Must be called
    /// regularly: a change is picked up on the first call after the
    /// file has been quiet for `SETTLE`.
    pub fn poll_reload(&mut self) -> Option<padshell_binding::Config> {
        let mut saw_event = false;
        while self.rx.try_recv().is_ok() {
            saw_event = true;
        }
        if saw_event {
            self.pending_since = Some(Instant::now());
        }
        if self.pending_since?.elapsed() < SETTLE {
            return None;
        }
        self.pending_since = None;

        match padshell_binding::Config::load(&self.config_path) {
            Ok(config) => Some(config),
            Err(err) => {
                tracing::warn!(error = %err, "config reload failed, keeping previous config");
                None
            }
        }
    }
}
