// No console window for the control panel or the background monitor.
#![windows_subsystem = "windows"]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--monitor") {
        std::process::exit(app_blocker::monitor::run(&args[1..]));
    }
    if let Err(message) = app_blocker::ui::run() {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title(app_blocker::paths::APP_NAME)
            .set_description(format!("The window could not be opened.\n\n{message}"))
            .show();
        std::process::exit(1);
    }
}
