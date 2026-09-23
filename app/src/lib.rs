//! Windows App Blocker: closes selected apps during scheduled hours.
//!
//! The same executable runs the control panel (no arguments) and the background
//! monitor (`--monitor`), which Task Scheduler starts as SYSTEM.

pub mod config;
pub mod detect;
pub mod install;
pub mod logger;
pub mod monitor;
pub mod notify;
pub mod paths;
pub mod policy;
pub mod schedule;
pub mod task;
pub mod time;
pub mod ui;
pub mod win;
