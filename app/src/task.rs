//! The `WindowsAppBlocker` scheduled task. Protection is on when the task is enabled.

use crate::paths::TASK_NAME;
use crate::win::{run_hidden, system32};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskInfo {
    pub enabled: bool,
    pub command: String,
}

impl TaskInfo {
    /// Whether the task was registered by the previous PowerShell-based version.
    pub fn is_legacy(&self) -> bool {
        self.command.to_ascii_lowercase().contains("powershell")
    }
}

fn schtasks(args: &[&str]) -> Result<String, String> {
    let output = run_hidden(&system32("schtasks.exe"), args).map_err(|e| format!("Could not run schtasks.exe: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if stderr.trim().is_empty() { stdout.trim().to_owned() } else { stderr.trim().to_owned() };
        Err(message)
    }
}

fn element<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))? + start;
    Some(xml[start..end].trim())
}

/// Returns `None` when the task does not exist.
pub fn query() -> Option<TaskInfo> {
    let xml = schtasks(&["/Query", "/TN", TASK_NAME, "/XML"]).ok()?;
    let settings = element(&xml, "Settings").unwrap_or("");
    Some(TaskInfo {
        enabled: element(settings, "Enabled").map(|v| v != "false").unwrap_or(true),
        command: element(&xml, "Command").unwrap_or("").trim_matches('"').to_owned(),
    })
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn task_xml(exe: &Path, target_user_sid: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Blocks selected Windows applications during configured hours.</Description>
  </RegistrationInfo>
  <Triggers>
    <BootTrigger><Enabled>true</Enabled></BootTrigger>
    <LogonTrigger><Enabled>true</Enabled><UserId>{sid}</UserId></LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>S-1-5-18</UserId>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>false</Enabled>
    <Hidden>true</Hidden>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{exe}"</Command>
      <Arguments>--monitor</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        sid = escape(target_user_sid),
        exe = escape(&exe.to_string_lossy()),
    )
}

/// Registers (or replaces) the task, disabled. Protection is turned on separately.
pub fn register(exe: &Path, target_user_sid: &str) -> Result<(), String> {
    let xml_path = std::env::temp_dir().join(format!("windows-app-blocker-task-{}.xml", std::process::id()));
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(task_xml(exe, target_user_sid).encode_utf16().flat_map(u16::to_le_bytes));
    std::fs::write(&xml_path, bytes).map_err(|e| format!("Could not write the task definition: {e}"))?;
    let xml_arg = xml_path.to_string_lossy().into_owned();
    let result = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &xml_arg, "/F"]);
    let _ = std::fs::remove_file(&xml_path);
    result.map(|_| ()).map_err(|e| format!("Could not register the background task: {e}"))
}

pub fn stop() {
    let _ = schtasks(&["/End", "/TN", TASK_NAME]);
}

pub fn enable_and_run() -> Result<(), String> {
    schtasks(&["/Change", "/TN", TASK_NAME, "/ENABLE"]).map_err(|e| format!("Could not turn protection on: {e}"))?;
    schtasks(&["/Run", "/TN", TASK_NAME]).map_err(|e| format!("Could not start the background task: {e}"))?;
    Ok(())
}

pub fn stop_and_disable() -> Result<(), String> {
    stop();
    schtasks(&["/Change", "/TN", TASK_NAME, "/DISABLE"]).map_err(|e| format!("Could not turn protection off: {e}"))?;
    Ok(())
}

pub fn delete() -> Result<(), String> {
    if query().is_none() {
        return Ok(());
    }
    stop();
    schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]).map(|_| ()).map_err(|e| format!("Could not remove the background task: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_definition_is_well_formed() {
        let xml = task_xml(Path::new(r"C:\Program Files\Windows App Blocker\Windows App Blocker.exe"), "S-1-5-21-1-2-3-1001");
        assert_eq!(element(element(&xml, "Settings").unwrap(), "Enabled"), Some("false"));
        assert_eq!(element(&xml, "Arguments"), Some("--monitor"));
        assert!(xml.contains("<UserId>S-1-5-21-1-2-3-1001</UserId>"));
        assert!(TaskInfo { enabled: true, command: r"C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe".into() }.is_legacy());
    }
}
