//! Scans XDG desktop entries - the same set wofi's drun mode shows - so
//! the Bindings tab can offer "Launch Firefox" as an action. Each entry
//! becomes a ready-to-run shell command (field codes stripped, Terminal=
//! true entries wrapped in the user's terminal).

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct DesktopApp {
    /// Desktop file id, e.g. "firefox.desktop".
    pub id: String,
    pub name: String,
    pub comment: String,
    /// Shell command to launch it.
    pub command: String,
}

/// Directories in precedence order (first match for an id wins), the
/// way wofi/gio resolve them.
fn application_dirs() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    let data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(&home).join(".local/share"));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    let mut dirs = vec![data_home.join("applications")];
    for d in data_dirs.split(':').filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(d).join("applications"));
    }
    for extra in ["/var/lib/flatpak/exports/share/applications", "/var/lib/snapd/desktop/applications"] {
        dirs.push(PathBuf::from(extra));
    }
    dirs.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share/applications"));
    dirs
}

pub fn scan(terminal: &str) -> Vec<DesktopApp> {
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    for dir in application_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Some(id) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
            if !seen.insert(id.clone()) {
                continue; // shadowed by a higher-precedence dir
            }
            if let Some(app) = parse(&path, &id, terminal) {
                apps.push(app);
            }
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

fn parse(path: &Path, id: &str, terminal: &str) -> Option<DesktopApp> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_main = false;
    let mut name = None;
    let mut comment = String::new();
    let mut exec = None;
    let mut is_app = false;
    let mut hidden = false;
    let mut term = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "Type" => is_app = v == "Application",
            "Name" => name = Some(v.to_string()),
            "Comment" => comment = v.to_string(),
            "Exec" => exec = Some(v.to_string()),
            "NoDisplay" | "Hidden" => hidden |= v.eq_ignore_ascii_case("true"),
            "Terminal" => term = v.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    if !is_app || hidden {
        return None;
    }
    let exec = strip_field_codes(&exec?);
    if exec.is_empty() {
        return None;
    }
    let command = if term { format!("{terminal} -e {exec}") } else { exec };
    Some(DesktopApp { id: id.to_string(), name: name?, comment, command })
}

/// Removes %f %F %u %U %i %c %k and friends from an Exec line.
fn strip_field_codes(exec: &str) -> String {
    let mut out = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('%') => out.push('%'),
                Some(_) | None => {}
            }
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn strips_field_codes() {
        assert_eq!(super::strip_field_codes("firefox %u"), "firefox");
        assert_eq!(super::strip_field_codes("app --pct=100%% %F x"), "app --pct=100% x");
    }
}
