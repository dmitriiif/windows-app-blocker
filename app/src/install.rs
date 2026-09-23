//! Installing, upgrading and uninstalling.
//!
//! The program lives in `C:\Program Files\Windows App Blocker` and is removed on uninstall.
//! Settings live in `C:\ProgramData\WindowsAppBlocker` and are kept, so a reinstall starts from
//! the same apps, schedule and lock choices.

use crate::config::{self, Config};
use crate::paths::{self, APP_NAME};
use crate::task;
use crate::win;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

/// Files left in the data folder by the PowerShell-based version.
const LEGACY_FILES: [&str; 5] = [
    "AppBlocker.Common.ps1",
    "AppBlockerMonitor.ps1",
    "Install-AppBlocker.ps1",
    "Uninstall-AppBlocker.ps1",
    "Windows App Blocker.exe",
];

pub fn is_installed() -> bool {
    paths::installed_exe().is_file() && task::query().is_some_and(|t| !t.is_legacy())
}

fn shortcut_paths() -> (Option<PathBuf>, Option<PathBuf>) {
    let file = format!("{APP_NAME}.lnk");
    (win::common_programs_dir().map(|d| d.join(&file)), win::desktop_dir().map(|d| d.join(&file)))
}

/// SYSTEM and administrators keep full control; other users may only read.
fn secure_directory(dir: &Path) -> Result<(), String> {
    let dir_arg = dir.to_string_lossy().into_owned();
    let output = win::run_hidden(
        &win::system32("icacls.exe"),
        &[&dir_arg, "/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F", "*S-1-5-32-545:(OI)(CI)RX"],
    )
    .map_err(|e| format!("Could not run icacls.exe: {e}"))?;
    if !output.status.success() {
        return Err("Could not secure the settings folder.".into());
    }
    Ok(())
}

fn copy_with_retry(from: &Path, to: &Path) -> Result<(), String> {
    let mut last_error = None;
    for _ in 0..20 {
        match fs::copy(from, to) {
            Ok(_) => return Ok(()),
            Err(e) => last_error = Some(e),
        }
        sleep(Duration::from_millis(250));
    }
    Err(format!("Could not copy the program to {}: {}", to.display(), last_error.map(|e| e.to_string()).unwrap_or_default()))
}

/// Removes files from the PowerShell version and backs up its configuration.
fn remove_legacy_install(data_dir: &Path) {
    let config_path = data_dir.join("config.json");
    if let Ok(text) = fs::read_to_string(&config_path) {
        if config::is_legacy_json(&text) {
            let _ = fs::write(data_dir.join("config.v2.bak.json"), text);
        }
    }
    for file in LEGACY_FILES {
        let _ = fs::remove_file(data_dir.join(file));
    }
}

/// Saves unfinished setup choices, so closing the window before installing loses nothing.
pub fn save_setup_progress(config: &Config) -> Result<(), String> {
    let data_dir = paths::data_dir();
    let new_folder = !data_dir.exists();
    config::save(config, &paths::config_path())?;
    if new_folder {
        secure_directory(&data_dir)?;
    }
    Ok(())
}

/// Installs or upgrades the app with `config`, which must already have its setup choices.
pub fn install(config: &Config) -> Result<(), String> {
    let restore_protection = task::query().is_some_and(|t| t.enabled && !t.is_legacy());
    if task::query().is_some() {
        task::stop();
    }

    let data_dir = paths::data_dir();
    fs::create_dir_all(&data_dir).map_err(|e| format!("Could not create {}: {e}", data_dir.display()))?;
    remove_legacy_install(&data_dir);

    let program_dir = paths::program_dir();
    fs::create_dir_all(&program_dir).map_err(|e| format!("Could not create {}: {e}", program_dir.display()))?;
    let current = std::env::current_exe().map_err(|e| format!("Could not locate this program: {e}"))?;
    let installed = paths::installed_exe();
    if !paths::same_path(&current.to_string_lossy(), &installed.to_string_lossy()) {
        copy_with_retry(&current, &installed)?;
    }

    let mut config = config.clone();
    config.setup_completed = true;
    config::save(&config, &paths::config_path())?;
    secure_directory(&data_dir)?;

    task::register(&installed, &config.target_user_sid)?;

    let (start_menu, desktop) = shortcut_paths();
    for (link, wanted) in [(start_menu, config.shortcuts.start_menu), (desktop, config.shortcuts.desktop)] {
        let Some(link) = link else { continue };
        if wanted {
            win::create_shortcut(&link, &installed, &program_dir)?;
        } else {
            let _ = fs::remove_file(&link);
        }
    }

    if restore_protection {
        task::enable_and_run()?;
    }
    Ok(())
}

/// Removes the task, shortcuts and program folder. Settings stay in ProgramData.
///
/// The program folder is deleted by a helper after this process exits, because Windows does not
/// allow a running program to delete itself. The caller should exit promptly afterwards.
pub fn uninstall() -> Result<(), String> {
    task::delete()?;
    let (start_menu, desktop) = shortcut_paths();
    for link in [start_menu, desktop].into_iter().flatten() {
        let _ = fs::remove_file(link);
    }

    let program_dir = paths::program_dir();
    if !program_dir.exists() {
        return Ok(());
    }
    let expected = paths::normalize(&paths::program_dir().to_string_lossy());
    if expected.as_deref().is_none_or(|p| !p.ends_with(r"\windows app blocker")) {
        return Err(format!("Refusing to remove unexpected directory: {}", program_dir.display()));
    }
    let dir = program_dir.to_string_lossy();
    // Retries for about 20 seconds while this window closes.
    let command = format!(
        "for /l %i in (1,1,20) do (if exist \"{dir}\" (ping -n 2 127.0.0.1 >nul & rmdir /s /q \"{dir}\"))"
    );
    win::spawn_hidden_cmd(&command).map_err(|e| format!("Could not start the cleanup helper: {e}"))
}
