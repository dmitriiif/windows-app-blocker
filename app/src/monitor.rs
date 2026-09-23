//! Background monitor started by Task Scheduler as SYSTEM (`--monitor`).
//!
//! Options for manual testing: `--once` runs a single check, `--dry-run` only reports what it
//! would close (printed and logged), `--config <path>` reads another configuration file.

use crate::config::{self, Config};
use crate::logger::Logger;
use crate::notify;
use crate::paths;
use crate::schedule::is_blocked;
use crate::win::{self, OpenedProcess};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

pub fn run(args: &[String]) -> i32 {
    let once = args.iter().any(|a| a == "--once");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let config_path = args
        .iter()
        .position(|a| a == "--config")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(paths::config_path);
    let log_path = config_path.parent().map(|p| p.join("WindowsAppBlocker.log")).unwrap_or_else(paths::log_path);
    let mut log = Logger::new(log_path, dry_run);

    let Some(_mutex) = win::single_instance(r"Global\WindowsAppBlockerMonitor") else {
        return 0;
    };

    // Keep enforcing the last valid configuration if the file becomes unreadable.
    let mut last_good: Option<Config> = None;
    let mut notified_for = None;
    loop {
        match config::load(&config_path) {
            Ok(config) => last_good = Some(config),
            Err(message) => {
                log.error(&message);
                if once && last_good.is_none() {
                    return 1;
                }
            }
        }
        let interval = match &last_good {
            Some(config) => {
                let now = chrono::Local::now().naive_local();
                if is_blocked(config, now) {
                    enforce(config, dry_run, &mut log);
                } else if let Some((start, title, message)) = notify::due(config, now, notified_for) {
                    notified_for = Some(start);
                    if dry_run {
                        log.info(&format!("Would notify: {title}"));
                    } else if let Err(e) = notify::send_to_user(config, &title, &message) {
                        log.error(&format!("Could not show the notification: {e}"));
                    } else {
                        log.info(&format!("Notified: {title}"));
                    }
                }
                config.check_interval_seconds.clamp(1, 60)
            }
            None => 5,
        };
        if once {
            return 0;
        }
        sleep(Duration::from_secs(interval as u64));
    }
}

fn enforce(config: &Config, dry_run: bool, log: &mut Logger) {
    let processes = match win::list_processes() {
        Ok(list) => list,
        Err(e) => {
            log.error(&format!("Could not list running processes: {}", e.message()));
            return;
        }
    };
    let own_pid = std::process::id();
    for info in processes.into_iter().filter(|p| p.pid > 4 && p.pid != own_pid) {
        let Some(process) = OpenedProcess::open(info.pid) else { continue };
        if !paths::is_configured(process.image_path().as_deref(), &config.executables) {
            continue;
        }
        if !process.owner_sid().is_some_and(|sid| sid.eq_ignore_ascii_case(&config.target_user_sid)) {
            continue;
        }
        if dry_run {
            log.info(&format!("Would stop {} (PID {})", info.name, info.pid));
        } else if !process.can_terminate {
            log.error(&format!("Could not stop {} (PID {}): access denied", info.name, info.pid));
        } else {
            match process.terminate() {
                Ok(()) => log.info(&format!("Stopped {} (PID {})", info.name, info.pid)),
                Err(e) => log.error(&format!("Could not stop {} (PID {}): {}", info.name, info.pid, e.message())),
            }
        }
    }
}
