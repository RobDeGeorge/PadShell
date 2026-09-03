//! Watches the config file's parent directory for changes and re-parses
//! on write/rename (editors often save via atomic rename, which doesn't
//! fire a plain "modify" event on the file itself).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
    rx: Receiver<()>,
    config_path: PathBuf,
    last_reload: std::time::Instant,
}

impl ConfigWatcher {
    pub fn new(config_path: &Path) -> anyhow::Result<Self> {
        let (tx, rx) = channel();
        let watch_target = config_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = tx.send(());
            }
        })?;
        watcher.watch(watch_target, RecursiveMode::NonRecursive)?;

        Ok(ConfigWatcher {
            _watcher: watcher,
            rx,
            config_path: config_path.to_path_buf(),
            last_reload: std::time::Instant::now(),
        })
    }

    /// Non-blocking check for a pending reload, debounced to avoid
    /// re-parsing on every individual event of a multi-event save.
    pub fn poll_reload(&mut self) -> Option<padshell_binding::Config> {
        let mut saw_event = false;
        while self.rx.try_recv().is_ok() {
            saw_event = true;
        }
        if !saw_event {
            return None;
        }
        if self.last_reload.elapsed() < Duration::from_millis(200) {
            return None;
        }
        self.last_reload = std::time::Instant::now();

        match padshell_binding::Config::load(&self.config_path) {
            Ok(config) => Some(config),
            Err(err) => {
                tracing::warn!(error = %err, "config reload failed, keeping previous config");
                None
            }
        }
    }
}
