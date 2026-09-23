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

/// Whether a running process's full path exactly matches one of the configured executables.
pub fn is_configured(process_path: Option<&str>, executables: &[String]) -> bool {
    let Some(process) = process_path.and_then(normalize) else { return false };
    executables.iter().filter_map(|e| normalize(e)).any(|e| e == process)
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
}
