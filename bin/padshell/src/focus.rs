//! Tracks Hyprland's focused window so workers can stand down while a
//! game that reads the controller itself is in front.
//!
//! Listens on Hyprland's event socket (`.socket2.sock`) for
//! `activewindow>>class,title` lines, seeding the initial value from the
//! request socket. Not running under Hyprland just means nothing is ever
//! focused, so padshell never pauses.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long to wait before reconnecting after Hyprland's socket goes
/// away (compositor restart, or not started yet at login).
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq)]
struct ActiveWindow {
    class: String,
    title: String,
}

#[derive(Clone, Default)]
pub struct FocusWatcher {
    active: Arc<Mutex<ActiveWindow>>,
}

impl FocusWatcher {
    /// Spawns the background listener and returns a handle workers can
    /// query cheaply on every report.
    pub fn spawn() -> Self {
        let watcher = FocusWatcher::default();
        let active = Arc::clone(&watcher.active);
        std::thread::Builder::new()
            .name("hypr-focus".into())
            .spawn(move || listen(active))
            .expect("spawn focus watcher thread");
        watcher
    }

    /// True if the focused window matches any entry: an exact window
    /// class or a substring of its title, both case-insensitive.
    pub fn matches(&self, patterns: &[String]) -> bool {
        if patterns.is_empty() {
            return false;
        }
        let active = self.active.lock().unwrap();
        let class = active.class.to_lowercase();
        let title = active.title.to_lowercase();
        patterns.iter().any(|p| {
            let p = p.trim().to_lowercase();
            !p.is_empty() && (class == p || title.contains(&p))
        })
    }
}

fn listen(active: Arc<Mutex<ActiveWindow>>) {
    loop {
        if let Some(dir) = socket_dir() {
            if let Some(initial) = query_active(&dir) {
                *active.lock().unwrap() = initial;
            }
            match UnixStream::connect(dir.join(".socket2.sock")) {
                Ok(stream) => {
                    tracing::debug!(?dir, "listening for Hyprland focus changes");
                    for line in BufReader::new(stream).lines() {
                        let Ok(line) = line else { break };
                        if let Some(window) = parse_event(&line) {
                            *active.lock().unwrap() = window;
                        }
                    }
                }
                Err(err) => tracing::debug!(error = %err, "Hyprland event socket unavailable"),
            }
            *active.lock().unwrap() = ActiveWindow::default();
        }
        std::thread::sleep(RECONNECT_DELAY);
    }
}

/// `$XDG_RUNTIME_DIR/hypr/<instance>`. The systemd user service may not
/// have `HYPRLAND_INSTANCE_SIGNATURE` in its environment, so fall back to
/// the most recently started instance directory.
fn socket_dir() -> Option<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    let hypr = runtime.join("hypr");
    if let Some(sig) = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE") {
        let dir = hypr.join(sig);
        if dir.join(".socket2.sock").exists() {
            return Some(dir);
        }
    }
    std::fs::read_dir(&hypr)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().join(".socket2.sock").exists())
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

/// Asks Hyprland for the currently focused window (plain-text
/// `activewindow` reply, which lists `class:` and `title:` fields).
fn query_active(dir: &std::path::Path) -> Option<ActiveWindow> {
    let mut stream = UnixStream::connect(dir.join(".socket.sock")).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    stream.write_all(b"activewindow").ok()?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply).ok()?;
    let field = |name: &str| {
        reply
            .lines()
            .find_map(|l| l.trim_start().strip_prefix(name))
            .map(|v| v.trim().to_string())
            .unwrap_or_default()
    };
    Some(ActiveWindow { class: field("class:"), title: field("title:") })
}

/// `activewindow>>class,title` - the class never contains a comma, the
/// title may. An empty payload (`activewindow>>,`) means nothing focused.
fn parse_event(line: &str) -> Option<ActiveWindow> {
    let payload = line.strip_prefix("activewindow>>")?;
    let (class, title) = payload.split_once(',').unwrap_or((payload, ""));
    Some(ActiveWindow { class: class.to_string(), title: title.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watcher_with(class: &str, title: &str) -> FocusWatcher {
        let w = FocusWatcher::default();
        *w.active.lock().unwrap() = ActiveWindow { class: class.into(), title: title.into() };
        w
    }

    #[test]
    fn parses_title_with_commas() {
        let w = parse_event("activewindow>>kitty,vim a, b").unwrap();
        assert_eq!(w.class, "kitty");
        assert_eq!(w.title, "vim a, b");
        assert!(parse_event("workspace>>2").is_none());
    }

    #[test]
    fn matches_class_exactly_or_title_substring() {
        let w = watcher_with("skate3rust", "Skate 3 Rust Engine");
        assert!(w.matches(&["SKATE3RUST".into()]));
        assert!(w.matches(&["skate 3".into()]));
        assert!(!w.matches(&["skate".into(), "retroarch".into()][1..]));
        assert!(!w.matches(&[]));
        assert!(!watcher_with("", "").matches(&["".into()]));
    }
}
