//! Reads a Hyprland config and turns its `bind*` lines into a list of
//! named actions the Bindings tab can offer, video-game style ("Close
//! window", "Go to workspace 3", ...). Each action carries the padshell
//! key names needed to reproduce its shortcut through the virtual
//! keyboard, so binding a controller button to an action is just a
//! `key_combo` binding with those keys.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct HyprAction {
    /// Stable id: "<submap>|<mods>|<key>|<dispatcher>|<args>".
    pub id: String,
    /// Friendly label, e.g. "Close window".
    pub label: String,
    /// Grouping: the section header comment above the bind in the
    /// config if there is one, otherwise derived from the dispatcher.
    pub category: String,
    /// Shortcut as the user would read it: "SUPER + SHIFT + Q".
    pub shortcut: String,
    /// padshell key names for a key_combo action, or None when the
    /// shortcut uses a key the virtual keyboard can't emit (XF86*,
    /// mouse binds, raw keycodes). `reason` explains why.
    pub keys: Option<Vec<String>>,
    pub reason: String,
    pub dispatcher: String,
    pub args: String,
    /// Submap the bind lives in ("" for the global map).
    pub submap: String,
    pub flags: String,
}

pub fn default_config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("hypr/hyprland.conf");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    PathBuf::from(home).join(".config/hypr/hyprland.conf")
}

pub struct HyprConfig {
    pub actions: Vec<HyprAction>,
    /// Value of `$terminal` in the config, if set (used to run
    /// Terminal=true desktop apps).
    pub terminal: Option<String>,
}

pub fn load(path: &Path) -> std::io::Result<HyprConfig> {
    let mut parser = Parser::default();
    parser.parse_file(path, 0)?;
    let terminal = parser.vars.get("terminal").cloned();
    Ok(HyprConfig { actions: parser.actions, terminal })
}

#[derive(Default)]
struct Parser {
    vars: HashMap<String, String>,
    submap: String,
    section: String,
    prev_line_was_rule: bool,
    /// A plain comment seen after a rule line, promoted to `section`
    /// only if another rule line follows it (rule / title / rule).
    pending_header: Option<String>,
    /// The most recent plain comment, consumed by the next bind as a
    /// label hint for exec commands ("# Power menu popup").
    hint: Option<String>,
    actions: Vec<HyprAction>,
}

impl Parser {
    fn parse_file(&mut self, path: &Path, depth: usize) -> std::io::Result<()> {
        if depth > 8 {
            return Ok(());
        }
        let text = std::fs::read_to_string(path)?;
        for raw in text.lines() {
            self.parse_line(raw, path, depth);
        }
        Ok(())
    }

    fn parse_line(&mut self, raw: &str, path: &Path, depth: usize) {
        let line = raw.trim();
        if line.is_empty() {
            self.hint = None;
            self.prev_line_was_rule = false;
            self.pending_header = None;
            return;
        }

        // Section headers: a comment sandwiched between decorative
        // "# =====" rules, e.g. "# Applications (Your i3 keybinds)".
        // Any other comment is a hint for the bind right below it.
        if let Some(comment) = line.strip_prefix('#') {
            let comment = comment.trim();
            let is_rule = !comment.is_empty()
                && comment.chars().all(|c| matches!(c, '=' | '-' | '#' | '*' | '~'));
            if is_rule {
                if let Some(header) = self.pending_header.take() {
                    self.section = header;
                    self.hint = None;
                }
                self.prev_line_was_rule = true;
            } else {
                if self.prev_line_was_rule && !comment.is_empty() {
                    self.pending_header = Some(comment.to_string());
                }
                if !comment.is_empty() {
                    self.hint = Some(comment.to_string());
                }
                self.prev_line_was_rule = false;
            }
            return;
        }
        self.prev_line_was_rule = false;
        self.pending_header = None;

        let (line, inline_comment) = split_trailing_comment(line);
        if let Some(c) = inline_comment {
            self.hint = Some(c.to_string());
        }
        let Some((lhs, rhs)) = line.split_once('=') else {
            return;
        };
        let lhs = lhs.trim();
        let rhs = self.expand_vars(rhs.trim());

        if let Some(name) = lhs.strip_prefix('$') {
            self.vars.insert(name.to_string(), rhs);
            return;
        }
        match lhs {
            "source" => {
                let target = expand_tilde(&rhs);
                let target = if Path::new(&target).is_absolute() {
                    PathBuf::from(target)
                } else {
                    path.parent().unwrap_or(Path::new(".")).join(target)
                };
                let _ = self.parse_file(&target, depth + 1);
            }
            "submap" => {
                self.submap = if rhs == "reset" { String::new() } else { rhs };
            }
            _ if lhs.starts_with("bind") => {
                // "bind", "binde", "bindl", "bindm", ... - the suffix
                // letters are flags.
                let flags = lhs[4..].to_string();
                self.push_bind(&flags, &rhs);
            }
            _ => {}
        }
    }

    fn expand_vars(&self, s: &str) -> String {
        if !s.contains('$') {
            return s.to_string();
        }
        // Longest names first so "$mainMod" isn't clobbered by "$main".
        let mut names: Vec<&String> = self.vars.keys().collect();
        names.sort_by_key(|n| std::cmp::Reverse(n.len()));
        let mut out = s.to_string();
        for name in names {
            let pat = format!("${name}");
            if out.contains(&pat) {
                out = out.replace(&pat, &self.vars[name]);
            }
        }
        out
    }

    fn push_bind(&mut self, flags: &str, rhs: &str) {
        let parts: Vec<&str> = rhs.splitn(4, ',').map(str::trim).collect();
        if parts.len() < 3 {
            return;
        }
        let mods_raw = parts[0];
        let key_raw = parts[1];
        let dispatcher = parts[2].to_string();
        let args = parts.get(3).map(|s| s.to_string()).unwrap_or_default();

        let mods: Vec<String> = mods_raw
            .split(|c: char| c.is_whitespace() || c == '+' || c == '_')
            .filter(|m| !m.is_empty())
            .map(|m| m.to_ascii_uppercase())
            .collect();

        let shortcut = {
            let mut parts = mods.clone();
            parts.push(pretty_key(key_raw));
            parts.join(" + ")
        };

        let (keys, reason) = translate_keys(&mods, key_raw, flags);
        let hint = self.hint.take();
        let mut label = describe(&dispatcher, &args, &self.submap);
        if matches!(dispatcher.as_str(), "exec" | "execr") {
            if let Some(media) = media_label(key_raw) {
                label = media.to_string();
            } else if let Some(h) = hint.filter(|h| !h.is_empty() && h.len() <= 60) {
                label = h;
            }
        }
        let label = capitalize(&label);
        let category = if !self.submap.is_empty() {
            format!("Mode: {}", self.submap)
        } else if !self.section.is_empty() {
            self.section.clone()
        } else {
            default_category(&dispatcher)
        };

        let id = format!(
            "{}|{}|{}|{}|{}",
            self.submap,
            mods.join("+"),
            key_raw.to_ascii_lowercase(),
            dispatcher,
            args
        );
        self.actions.push(HyprAction {
            id,
            label,
            category,
            shortcut,
            keys,
            reason,
            dispatcher,
            args,
            submap: self.submap.clone(),
            flags: flags.to_string(),
        });
    }
}

fn media_label(key: &str) -> Option<&'static str> {
    Some(match key.trim().to_ascii_lowercase().as_str() {
        "xf86audioraisevolume" => "Volume up",
        "xf86audiolowervolume" => "Volume down",
        "xf86audiomute" => "Mute",
        "xf86audiomicmute" => "Mute microphone",
        "xf86audionext" => "Next track",
        "xf86audioprev" => "Previous track",
        "xf86audioplay" | "xf86audiopause" => "Play / pause",
        "xf86monbrightnessup" => "Brightness up",
        "xf86monbrightnessdown" => "Brightness down",
        "xf86poweroff" => "Power menu",
        _ => return None,
    })
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn split_trailing_comment(line: &str) -> (&str, Option<&str>) {
    // Hyprland treats " #" (with leading whitespace) as a comment start;
    // a bare "#" inside exec args (e.g. "#ff0000") is kept.
    match line.find(" #") {
        Some(i) => (line[..i].trim_end(), Some(line[i + 2..].trim())),
        None => (line, None),
    }
}

fn expand_tilde(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        return format!("{home}/{rest}");
    }
    s.to_string()
}

fn pretty_key(key: &str) -> String {
    let k = key.trim();
    match k.to_ascii_lowercase().as_str() {
        "return" => "Return".into(),
        "space" => "Space".into(),
        "escape" => "Escape".into(),
        "page_up" | "prior" => "PgUp".into(),
        "page_down" | "next" => "PgDn".into(),
        "semicolon" => ";".into(),
        "equal" => "=".into(),
        "minus" => "-".into(),
        "comma" => ",".into(),
        "period" => ".".into(),
        "left" | "right" | "up" | "down" => {
            let mut c = k.chars();
            c.next().map(|f| f.to_ascii_uppercase()).into_iter().chain(c.map(|x| x.to_ascii_lowercase())).collect()
        }
        _ if k.len() == 1 => k.to_ascii_uppercase(),
        _ => k.to_string(),
    }
}

/// Maps Hyprland modifier + key names to padshell key names.
fn translate_keys(mods: &[String], key: &str, flags: &str) -> (Option<Vec<String>>, String) {
    if flags.contains('m') {
        return (None, "mouse bind (bindm) - drag actions can't be sent as keys".into());
    }
    let mut keys = Vec::new();
    for m in mods {
        let name = match m.as_str() {
            "SUPER" | "WIN" | "LOGO" | "MOD4" => "super",
            "SHIFT" => "shift",
            "CTRL" | "CONTROL" => "ctrl",
            "ALT" | "MOD1" => "alt",
            other => return (None, format!("unsupported modifier {other}")),
        };
        if !keys.iter().any(|k| k == name) {
            keys.push(name.to_string());
        }
    }
    let k = key.trim();
    let lower = k.to_ascii_lowercase();
    if let Some(xf) = lower.strip_prefix("xf86") {
        let name = match xf {
            "audioraisevolume" => "volumeup",
            "audiolowervolume" => "volumedown",
            "audiomute" => "mute",
            "audiomicmute" => "micmute",
            "audionext" => "nextsong",
            "audioprev" => "previoussong",
            "audioplay" | "audiopause" => "playpause",
            "monbrightnessup" => "brightnessup",
            "monbrightnessdown" => "brightnessdown",
            _ => return (None, format!("{k} isn't on the virtual keyboard")),
        };
        keys.push(name.to_string());
        return (Some(keys), String::new());
    }
    if lower.starts_with("code:") || lower.starts_with("mouse:") || lower.starts_with("mouse_") {
        return (None, "raw keycodes and mouse buttons aren't supported".into());
    }
    let padshell_key = match lower.as_str() {
        "return" | "kp_enter" => "enter".to_string(),
        "escape" => "escape".to_string(),
        "prior" | "page_up" | "kp_prior" => "pageup".to_string(),
        "next" | "page_down" | "kp_next" => "pagedown".to_string(),
        "bracketleft" => "leftbrace".to_string(),
        "bracketright" => "rightbrace".to_string(),
        other => other.to_string(),
    };
    if padshell_output::keymap::lookup(&padshell_key).is_none() {
        return (None, format!("key '{k}' isn't on the virtual keyboard"));
    }
    keys.push(padshell_key);
    (Some(keys), String::new())
}

fn default_category(dispatcher: &str) -> String {
    match dispatcher {
        "exec" | "execr" => "Applications",
        "workspace" | "movetoworkspace" | "movetoworkspacesilent" | "togglespecialworkspace" => {
            "Workspaces"
        }
        "movefocus" | "focuswindow" | "focusurgentorlast" | "cyclenext" => "Focus",
        "togglegroup" | "changegroupactive" | "moveoutofgroup" | "moveintogroup" => "Groups",
        "submap" => "Modes",
        "exit" | "forcerendererreload" => "System",
        _ => "Windows",
    }
    .to_string()
}

fn describe(dispatcher: &str, args: &str, submap: &str) -> String {
    fn dir(a: &str) -> &str {
        match a.trim() {
            "l" => "left",
            "r" => "right",
            "u" => "up",
            "d" => "down",
            other => other,
        }
    }
    match dispatcher {
        "exec" | "execr" => describe_exec(args, submap),
        "killactive" => "Close window".into(),
        "fullscreen" => "Toggle fullscreen".into(),
        "togglefloating" => "Toggle floating".into(),
        "pin" => "Pin window".into(),
        "focusurgentorlast" => "Focus urgent / last window".into(),
        "movefocus" => format!("Focus {}", dir(args)),
        "movewindow" => format!("Move window {}", dir(args)),
        "swapwindow" => format!("Swap window {}", dir(args)),
        "resizeactive" => format!("Resize window {}", resize_dir(args)),
        "workspace" => match args.trim() {
            "e+1" | "r+1" | "+1" => "Next workspace".into(),
            "e-1" | "r-1" | "-1" => "Previous workspace".into(),
            "previous" => "Last workspace".into(),
            "empty" => "New empty workspace".into(),
            n => format!("Go to workspace {n}"),
        },
        "movetoworkspacesilent" | "movetoworkspace" if args.trim().starts_with("special:") => {
            format!("Send window to scratchpad {}", &args.trim()[8..])
        }
        "movetoworkspace" => format!("Move window to workspace {}", args.trim()),
        "movetoworkspacesilent" => format!("Send window to workspace {}", args.trim()),
        "togglespecialworkspace" => format!("Toggle scratchpad {}", args.trim()).trim_end().to_string(),
        "layoutmsg" => match args.trim() {
            "togglesplit" => "Toggle split direction".into(),
            "orientationnext" => "Next layout orientation".into(),
            "swapwithmaster" => "Swap with master".into(),
            other => format!("Layout: {other}"),
        },
        "togglegroup" => "Toggle window group".into(),
        "changegroupactive" => match args.trim() {
            "b" => "Previous tab in group".into(),
            _ => "Next tab in group".into(),
        },
        "moveoutofgroup" => "Move window out of group".into(),
        "submap" => {
            if args.trim() == "reset" {
                if submap.is_empty() {
                    "Exit mode".into()
                } else {
                    format!("Exit {submap} mode")
                }
            } else {
                format!("Enter {} mode", args.trim())
            }
        }
        "exit" => "Exit Hyprland".into(),
        other => {
            if args.trim().is_empty() {
                other.to_string()
            } else {
                format!("{other} {}", args.trim())
            }
        }
    }
}

fn describe_exec(args: &str, submap: &str) -> String {
    let a = args.trim();
    if let Some(rest) = a.strip_prefix("hyprctl ") {
        let rest = rest.trim();
        if rest == "reload" {
            return "Reload Hyprland config".into();
        }
        if let Some(d) = rest.strip_prefix("dispatch ") {
            let mut it = d.trim().splitn(2, ' ');
            let disp = it.next().unwrap_or("");
            let dargs = it.next().unwrap_or("");
            return describe(disp, dargs, submap);
        }
        if let Some(k) = rest.strip_prefix("keyword ") {
            let opt = k.split_whitespace().next().unwrap_or("");
            let opt_name = opt.rsplit(':').next().unwrap_or(opt).replace('_', " ");
            let delta = if k.contains("- ") || k.contains("-5") || k.ends_with("- 5))") {
                "−"
            } else if k.contains("+ ") {
                "+"
            } else {
                ""
            };
            let amount = k
                .rsplit(|c: char| !c.is_ascii_digit())
                .find(|s| !s.is_empty())
                .unwrap_or("");
            return format!("{opt_name} {delta}{amount}").trim().to_string();
        }
    }
    format!("Run {}", short_command(a))
}

fn resize_dir(args: &str) -> String {
    let mut it = args.split_whitespace();
    let x: i32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let y: i32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    match (x.signum(), y.signum()) {
        (-1, 0) => "narrower".into(),
        (1, 0) => "wider".into(),
        (0, -1) => "shorter".into(),
        (0, 1) => "taller".into(),
        _ => args.trim().to_string(),
    }
}

/// "pkill wofi || wofi --show drun" -> "wofi", "kitty -e nvim ..." -> "kitty".
fn short_command(args: &str) -> String {
    let a = args.trim();
    if a.is_empty() {
        return "command".into();
    }
    // Prefer the last "||" alternative: "pkill x || x --show" launches x.
    let alt = a.rsplit("||").next().unwrap_or(a).trim();
    let alt = alt.trim_start_matches("bash -c ").trim_matches('\'').trim_matches('"');
    let mut words = alt.split_whitespace();
    let first = words.next().unwrap_or(alt);
    let first = first.rsplit('/').next().unwrap_or(first);
    // Interpreters: show the script name instead.
    let first = if matches!(first, "python3" | "python" | "sh" | "bash") {
        let script = words.next().unwrap_or(first);
        script.rsplit('/').next().unwrap_or(script)
    } else {
        first
    };
    if a.len() > 60 && !a.contains("||") && alt.contains("&&") {
        return format!("{first} (script)");
    }
    let rest: Vec<&str> = alt.split_whitespace().skip(1).take(3).collect();
    if first == "kitty" || first == "alacritty" || first == "foot" {
        if let Some(pos) = rest.iter().position(|w| *w == "-e") {
            if let Some(prog) = rest.get(pos + 1) {
                return format!("{first}: {prog}");
            }
        }
    }
    first.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_binds_and_vars() {
        let dir = std::env::temp_dir().join(format!("padshell-hypr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hyprland.conf");
        std::fs::write(
            &path,
            "$terminal = kitty\n$mainMod = SUPER\n\
             # ====\n# Apps\n# ====\n\
             bind = $mainMod, Return, exec, $terminal\n\
             bind = $mainMod SHIFT, Q, killactive,\n\
             bind = , XF86PowerOff, exec, poweroff\n\
             bind = $mainMod, R, submap, resize\nsubmap = resize\n\
             binde = , J, resizeactive, -10 0\nsubmap = reset\n",
        )
        .unwrap();
        let acts = load(&path).unwrap().actions;
        assert_eq!(acts[0].label, "Run kitty");
        assert_eq!(acts[0].category, "Apps");
        assert_eq!(acts[0].keys.as_deref(), Some(&["super".to_string(), "enter".to_string()][..]));
        assert_eq!(acts[1].label, "Close window");
        assert_eq!(acts[1].shortcut, "SUPER + SHIFT + Q");
        assert!(acts[2].keys.is_none());
        assert_eq!(acts[4].category, "Mode: resize");
        assert_eq!(acts[4].label, "Resize window narrower");
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod real_config {
    /// Dev aid: `cargo test -p padshell-ui dump_real -- --ignored --nocapture`
    /// prints how the user's actual Hyprland config parses.
    #[test]
    #[ignore]
    fn dump_real() {
        let acts = super::load(&super::default_config_path()).unwrap().actions;
        for a in acts {
            println!(
                "[{}] {:<40} {:<22} {:?} {}",
                a.category, a.label, a.shortcut, a.keys, a.reason
            );
        }
    }
}
