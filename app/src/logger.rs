//! Monitor log: rotated at 1 MB (three old copies kept); repeated messages are written at most
//! once every five minutes.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const MAX_BYTES: u64 = 1024 * 1024;
const REPEAT_WINDOW: Duration = Duration::from_secs(5 * 60);

pub struct Logger {
    path: PathBuf,
    echo: bool,
    recent: HashMap<String, Instant>,
}

impl Logger {
    pub fn new(path: PathBuf, echo: bool) -> Logger {
        Logger { path, echo, recent: HashMap::new() }
    }

    pub fn info(&mut self, message: &str) {
        self.write("INFO", message);
    }

    pub fn error(&mut self, message: &str) {
        self.write("ERROR", message);
    }

    pub fn write(&mut self, level: &str, message: &str) {
        let key = format!("{level}|{message}");
        let now = Instant::now();
        if self.recent.get(&key).is_some_and(|t| now.duration_since(*t) < REPEAT_WINDOW) {
            return;
        }
        self.recent.retain(|_, t| now.duration_since(*t) < REPEAT_WINDOW);
        self.recent.insert(key, now);

        if self.echo {
            println!("[{level}] {message}");
        }
        self.rotate();
        let line = format!("{} [{level}] {message}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn rotate(&self) {
        if fs::metadata(&self.path).map(|m| m.len() <= MAX_BYTES).unwrap_or(true) {
            return;
        }
        let numbered = |n: u32| PathBuf::from(format!("{}.{n}", self.path.display()));
        for n in (1..=2).rev() {
            let _ = fs::rename(numbered(n), numbered(n + 1));
        }
        let _ = fs::rename(&self.path, numbered(1));
    }
}
