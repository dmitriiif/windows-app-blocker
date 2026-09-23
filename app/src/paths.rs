//! Install locations and executable-path matching.

use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "Windows App Blocker";
pub const EXE_NAME: &str = "Windows App Blocker.exe";
pub const TASK_NAME: &str = "WindowsAppBlocker";

fn env_dir(name: &str, fallback: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

/// `C:\Program Files\Windows App Blocker`: the program. Removed on uninstall.
pub fn program_dir() -> PathBuf {
    env_dir("ProgramW6432", r"C:\Program Files").join(APP_NAME)
}

pub fn installed_exe() -> PathBuf {
    program_dir().join(EXE_NAME)
}

/// `C:\ProgramData\WindowsAppBlocker`: settings and logs. Kept on uninstall.
pub fn data_dir() -> PathBuf {
    env_dir("ProgramData", r"C:\ProgramData").join("WindowsAppBlocker")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn log_path() -> PathBuf {
    data_dir().join("WindowsAppBlocker.log")
}

/// Normalises a path for comparison: absolute, backslashes, no trailing separator, lower case.
pub fn normalize(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let absolute = std::path::absolute(Path::new(trimmed)).ok()?;
    let text = absolute.to_string_lossy().replace('/', "\\");
    Some(text.trim_end_matches('\\').to_lowercase())
}

pub fn same_path(a: &str, b: &str) -> bool {
    matches!((normalize(a), normalize(b)), (Some(x), Some(y)) if x == y)
}

/// Whether a running process's full path matches one of the configured executables. A `*` in a
/// folder name matches any text within that one folder name, e.g. `Discord\app-*\Discord.exe`.
pub fn is_configured(process_path: Option<&str>, executables: &[String]) -> bool {
    let Some(process) = process_path.and_then(normalize) else { return false };
    executables.iter().filter_map(|e| normalize(e)).any(|e| matches_pattern(&e, &process))
}

fn matches_pattern(pattern: &str, path: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == path;
    }
    let pattern: Vec<&str> = pattern.split('\\').collect();
    let path: Vec<&str> = path.split('\\').collect();
    pattern.len() == path.len() && pattern.iter().zip(&path).all(|(p, t)| glob(p, t))
}

/// Matches one path segment against a pattern where `*` stands for any run of characters.
fn glob(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match p.split_first() {
            None => t.is_empty(),
            Some(('*', rest)) => (0..=t.len()).any(|i| go(rest, &t[i..])),
            Some((c, rest)) => t.first() == Some(c) && go(rest, &t[1..]),
        }
    }
    go(&pattern.chars().collect::<Vec<_>>(), &text.chars().collect::<Vec<_>>())
}

/// Files that currently exist for a configured path, expanding `*` in folder names.
pub fn expand(pattern: &str) -> Vec<PathBuf> {
    if !pattern.contains('*') {
        let path = PathBuf::from(pattern);
        return if path.is_file() { vec![path] } else { Vec::new() };
    }
    let text = pattern.replace('/', "\\");
    let mut parts = text.split('\\').filter(|p| !p.is_empty());
    let Some(root) = parts.next() else { return Vec::new() };
    let mut current = vec![PathBuf::from(format!("{root}\\"))];
    for part in parts {
        let mut next = Vec::new();
        for dir in &current {
            if !part.contains('*') {
                next.push(dir.join(part));
                continue;
            }
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            let wanted = part.to_lowercase();
            next.extend(entries.flatten().filter(|e| glob(&wanted, &e.file_name().to_string_lossy().to_lowercase())).map(|e| e.path()));
        }
        current = next;
    }
    current.retain(|p| p.is_file());
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_matching() {
        let targets = vec![r"C:\Program Files\Example\Example.exe".to_string(), r"D:\Tools\Editor.exe".to_string()];
        assert!(is_configured(Some(r"c:\program files\example\EXAMPLE.EXE"), &targets), "case insensitive");
        assert!(is_configured(Some(r"D:\Tools\Editor.exe"), &targets), "second executable matches");
        assert!(is_configured(Some("D:/Tools/Editor.exe"), &targets), "forward slashes");
        assert!(!is_configured(Some(r"C:\Program Files\Example\Helper.exe"), &targets), "different exe, same folder");
        assert!(!is_configured(Some(r"C:\Program Files\ExampleOther\Example.exe"), &targets), "similar folder");
        assert!(!is_configured(None, &targets), "missing process path");
    }

    #[test]
    fn wildcard_matching() {
        let targets = vec![r"C:\Users\Me\AppData\Local\Discord\app-*\Discord.exe".to_string()];
        assert!(is_configured(Some(r"C:\Users\Me\AppData\Local\Discord\app-1.0.9163\Discord.exe"), &targets), "any version");
        assert!(is_configured(Some(r"c:\users\me\appdata\local\discord\APP-2\discord.exe"), &targets), "case insensitive");
        assert!(!is_configured(Some(r"C:\Users\Me\AppData\Local\Discord\Discord.exe"), &targets), "folder missing");
        assert!(!is_configured(Some(r"C:\Users\Me\AppData\Local\Discord\app-1\x\Discord.exe"), &targets), "star stays in one folder");
        assert!(!is_configured(Some(r"C:\Users\Me\AppData\Local\Discord\other-1\Discord.exe"), &targets), "prefix must match");
        assert!(!is_configured(Some(r"C:\Users\Me\AppData\Local\Discord\app-1\Update.exe"), &targets), "other exe");
    }

    #[test]
    fn expands_wildcards_on_disk() {
        let dir = std::env::temp_dir().join(format!("app-blocker-expand-{}", std::process::id()));
        for version in ["app-1.0", "app-2.0", "packages"] {
            std::fs::create_dir_all(dir.join(version)).unwrap();
            std::fs::write(dir.join(version).join("Chat.exe"), b"").unwrap();
        }
        let pattern = dir.join("app-*").join("Chat.exe").to_string_lossy().into_owned();
        let mut found = expand(&pattern);
        found.sort();
        assert_eq!(found, vec![dir.join("app-1.0").join("Chat.exe"), dir.join("app-2.0").join("Chat.exe")]);
        assert_eq!(expand(&dir.join("app-1.0").join("Chat.exe").to_string_lossy()).len(), 1, "plain path");
        assert!(expand(&dir.join("app-*").join("Missing.exe").to_string_lossy()).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
