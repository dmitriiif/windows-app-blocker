//! Heads-up notifications shown shortly before a block starts.
//!
//! Toasts are raised by a hidden Windows PowerShell running as the signed-in user, under this
//! app's own name (registered in the user's `HKCU\Software\Classes\AppUserModelId`). When that
//! cannot be started, a plain message box is shown instead.

use crate::config::Config;
use crate::paths::APP_NAME;
use crate::schedule::next_change;
use crate::win;
use chrono::NaiveDateTime;
use std::path::PathBuf;

const APP_ID: &str = "WindowsAppBlocker";

/// The notification due at `now`, if one is: `(block start, title, message)`.
pub fn due(config: &Config, now: NaiveDateTime, already_sent_for: Option<NaiveDateTime>) -> Option<(NaiveDateTime, String, String)> {
    let prefs = &config.preferences;
    if !prefs.notify_before_block || config.executables.is_empty() {
        return None;
    }
    let start = next_change(config, now)?;
    let seconds = (start - now).num_seconds();
    if seconds <= 0 || seconds > prefs.notify_minutes as i64 * 60 || already_sent_for == Some(start) {
        return None;
    }
    let minutes = (seconds + 59) / 60;
    let title = format!("Blocking starts in {minutes} minute{}", if minutes == 1 { "" } else { "s" });
    let message = format!("Your blocked apps will be closed at {}. Save your work.", start.format("%H:%M"));
    Some((start, title, message))
}

fn powershell() -> PathBuf {
    win::system32(r"WindowsPowerShell\v1.0\powershell.exe")
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn toast_script(title: &str, message: &str) -> String {
    let xml = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual><audio src="ms-winsoundevent:Notification.Reminder"/></toast>"#,
        xml_escape(title),
        xml_escape(message)
    );
    format!(
        r#"$ErrorActionPreference = 'Stop'
$key = 'HKCU:\Software\Classes\AppUserModelId\{APP_ID}'
New-Item -Path $key -Force | Out-Null
Set-ItemProperty -Path $key -Name DisplayName -Value '{APP_NAME}'
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml('{}')
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{APP_ID}').Show([Windows.UI.Notifications.ToastNotification]::new($xml))
"#,
        xml.replace('\'', "''")
    )
}

/// PowerShell arguments that run `script` without quoting problems.
fn encoded_args(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    format!("-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -EncodedCommand {}", base64(&bytes))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { TABLE[(n >> (18 - 6 * i)) as usize & 63] as char } else { '=' });
        }
    }
    out
}

/// From the SYSTEM monitor: notifies the configured user in their own session.
pub fn send_to_user(config: &Config, title: &str, message: &str) -> Result<(), String> {
    let args = encoded_args(&toast_script(title, message));
    win::spawn_as_user(&config.target_user_sid, &powershell(), &args)
        .or_else(|toast_error| {
            win::message_box_for_user(&config.target_user_sid, title, message, 60)
                .map_err(|box_error| format!("{toast_error} {box_error}"))
        })
}

/// From the control panel: shows a sample notification to whoever is using it.
pub fn send_test() -> Result<(), String> {
    let script = toast_script("Blocking starts in 10 minutes", "This is how the heads-up will look before a block.");
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64(&bytes);
    win::run_hidden(
        &powershell(),
        &["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded],
    )
    .map_err(|e| format!("Could not start PowerShell: {e}"))
    .and_then(|output| {
        if output.status.success() {
            Ok(())
        } else {
            Err(format!("The notification could not be shown. {}", String::from_utf8_lossy(&output.stderr).trim()))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Block;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn notifies_once_inside_the_window() {
        let mut c = Config::default();
        c.days.every_day.blocks = vec![Block::new(23 * 60, 7 * 60).unwrap()];
        c.executables = vec![r"C:\Games\game.exe".into()];
        c.preferences.notify_minutes = 10;
        assert!(due(&c, at("2026-01-01 22:49:00"), None).is_none(), "off by default");
        c.preferences.notify_before_block = true;
        assert!(due(&c, at("2026-01-01 22:49:00"), None).is_none(), "too early");
        let (start, title, _) = due(&c, at("2026-01-01 22:50:30"), None).expect("inside the window");
        assert_eq!(start, at("2026-01-01 23:00:00"));
        assert_eq!(title, "Blocking starts in 10 minutes");
        assert!(due(&c, at("2026-01-01 22:55:00"), Some(start)).is_none(), "only once per block");
        assert!(due(&c, at("2026-01-01 23:30:00"), None).is_none(), "already blocking");
    }
}
