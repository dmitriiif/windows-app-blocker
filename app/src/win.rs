//! Thin wrappers over the Win32 APIs the app needs.

use std::ffi::c_void;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use windows::core::{Interface, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, LocalFree, ERROR_ALREADY_EXISTS, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows::Win32::System::RemoteDesktop::{
    WTSActive, WTSEnumerateSessionsW, WTSFreeMemory, WTSQueryUserToken, WTSSendMessageW, WTS_CURRENT_SERVER_HANDLE, WTS_SESSION_INFOW,
};
use windows::Win32::System::Threading::{
    CreateMutexW, CreateProcessAsUserW, GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    TerminateProcess, CREATE_NO_WINDOW as CREATE_NO_WINDOW_FLAG, CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, STARTF_USESHOWWINDOW, STARTUPINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MESSAGEBOX_RESULT, SW_HIDE};
use windows::Win32::UI::Shell::{FOLDERID_CommonPrograms, FOLDERID_Desktop, IShellLinkW, SHGetKnownFolderPath, ShellLink, KF_FLAG_DEFAULT};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A handle closed on drop.
pub struct OwnedHandle(pub HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Runs a console program without showing a window and captures its output.
pub fn run_hidden(program: &Path, args: &[&str]) -> std::io::Result<Output> {
    Command::new(program).args(args).creation_flags(CREATE_NO_WINDOW).output()
}

pub fn system32(file: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join(file)
}

/// Starts a detached, windowless `cmd.exe` with a raw command line.
pub fn spawn_hidden_cmd(command_line: &str) -> std::io::Result<()> {
    Command::new(system32("cmd.exe"))
        .raw_arg(format!("/d /c \"{command_line}\""))
        .creation_flags(CREATE_NO_WINDOW)
        .current_dir(std::env::temp_dir())
        .spawn()
        .map(|_| ())
}

/// Takes a named, machine-wide mutex. Returns `None` if another process already holds it.
pub fn single_instance(name: &str) -> Option<OwnedHandle> {
    unsafe {
        let handle = CreateMutexW(None, true, &HSTRING::from(name)).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }
        Some(OwnedHandle(handle))
    }
}

fn token_sid(token: HANDLE) -> Option<String> {
    unsafe {
        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        if needed == 0 {
            return None;
        }
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        GetTokenInformation(token, TokenUser, Some(buffer.as_mut_ptr() as *mut c_void), needed, &mut needed).ok()?;
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).ok()?;
        let sid = text.to_string().ok();
        let _ = LocalFree(HLOCAL(text.0 as *mut c_void));
        sid
    }
}

fn process_sid(process: HANDLE) -> Option<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let token = OwnedHandle(token);
        token_sid(token.0)
    }
}

/// SID of the account running this process.
pub fn current_user_sid() -> Option<String> {
    process_sid(unsafe { GetCurrentProcess() })
}

pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
}

pub fn list_processes() -> windows::core::Result<Vec<ProcessInfo>> {
    unsafe {
        let snapshot = OwnedHandle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?);
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut list = Vec::new();
        if Process32FirstW(snapshot.0, &mut entry).is_ok() {
            loop {
                let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
                list.push(ProcessInfo { pid: entry.th32ProcessID, name: String::from_utf16_lossy(&entry.szExeFile[..len]) });
                if Process32NextW(snapshot.0, &mut entry).is_err() {
                    break;
                }
            }
        }
        Ok(list)
    }
}

/// An open process. Checks and termination go through the same handle, so a reused PID can
/// never cause a different program to be closed.
pub struct OpenedProcess {
    handle: OwnedHandle,
    pub can_terminate: bool,
}

impl OpenedProcess {
    pub fn open(pid: u32) -> Option<OpenedProcess> {
        unsafe {
            if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE, false, pid) {
                return Some(OpenedProcess { handle: OwnedHandle(h), can_terminate: true });
            }
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            Some(OpenedProcess { handle: OwnedHandle(h), can_terminate: false })
        }
    }

    pub fn image_path(&self) -> Option<String> {
        unsafe {
            let mut buffer = vec![0u16; 32768];
            let mut size = buffer.len() as u32;
            QueryFullProcessImageNameW(self.handle.0, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut size).ok()?;
            Some(String::from_utf16_lossy(&buffer[..size as usize]))
        }
    }

    pub fn owner_sid(&self) -> Option<String> {
        process_sid(self.handle.0)
    }

    pub fn terminate(&self) -> windows::core::Result<()> {
        unsafe { TerminateProcess(self.handle.0, 1) }
    }
}

/// Tokens of the active sessions where `sid` is logged on. Only works when running as SYSTEM.
fn user_sessions(sid: &str) -> Vec<(u32, OwnedHandle)> {
    let mut found = Vec::new();
    unsafe {
        let mut info: *mut WTS_SESSION_INFOW = std::ptr::null_mut();
        let mut count = 0u32;
        if WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut info, &mut count).is_err() {
            return found;
        }
        for session in std::slice::from_raw_parts(info, count as usize) {
            if session.State != WTSActive {
                continue;
            }
            let mut token = HANDLE::default();
            if WTSQueryUserToken(session.SessionId, &mut token).is_err() {
                continue;
            }
            let token = OwnedHandle(token);
            if token_sid(token.0).is_some_and(|s| s.eq_ignore_ascii_case(sid)) {
                found.push((session.SessionId, token));
            }
        }
        WTSFreeMemory(info as *mut c_void);
    }
    found
}

/// Starts a hidden program on the desktop of every active session of `sid`, as that user.
pub fn spawn_as_user(sid: &str, program: &Path, args: &str) -> Result<(), String> {
    let sessions = user_sessions(sid);
    if sessions.is_empty() {
        return Err("The user is not signed in.".into());
    }
    for (_, token) in sessions {
        unsafe {
            let mut env: *mut c_void = std::ptr::null_mut();
            let has_env = CreateEnvironmentBlock(&mut env, token.0, false).is_ok();
            let mut command: Vec<u16> = format!("\"{}\" {args}", program.display()).encode_utf16().chain([0]).collect();
            let mut desktop: Vec<u16> = r"winsta0\default".encode_utf16().chain([0]).collect();
            let startup = STARTUPINFOW {
                cb: std::mem::size_of::<STARTUPINFOW>() as u32,
                lpDesktop: PWSTR(desktop.as_mut_ptr()),
                dwFlags: STARTF_USESHOWWINDOW,
                wShowWindow: SW_HIDE.0 as u16,
                ..Default::default()
            };
            let mut process = PROCESS_INFORMATION::default();
            let result = CreateProcessAsUserW(
                token.0,
                PCWSTR::null(),
                PWSTR(command.as_mut_ptr()),
                None,
                None,
                false,
                CREATE_NO_WINDOW_FLAG | CREATE_UNICODE_ENVIRONMENT,
                has_env.then_some(env as *const c_void),
                PCWSTR::null(),
                &startup,
                &mut process,
            );
            if has_env {
                let _ = DestroyEnvironmentBlock(env);
            }
            result.map_err(|e| format!("Could not start a program for the user: {}", e.message()))?;
            drop(OwnedHandle(process.hProcess));
            drop(OwnedHandle(process.hThread));
        }
    }
    Ok(())
}

/// Shows a message box on every active session of `sid`, without waiting for it to be closed.
pub fn message_box_for_user(sid: &str, title: &str, message: &str, timeout_seconds: u32) -> Result<(), String> {
    let sessions = user_sessions(sid);
    if sessions.is_empty() {
        return Err("The user is not signed in.".into());
    }
    let title_w = HSTRING::from(title);
    let message_w = HSTRING::from(message);
    for (session, _) in sessions {
        let mut response = MESSAGEBOX_RESULT::default();
        unsafe {
            WTSSendMessageW(
                WTS_CURRENT_SERVER_HANDLE,
                session,
                &title_w,
                (title_w.len() * 2) as u32,
                &message_w,
                (message_w.len() * 2) as u32,
                MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND,
                timeout_seconds,
                &mut response,
                false,
            )
            .map_err(|e| format!("Could not show a message: {}", e.message()))?;
        }
    }
    Ok(())
}

fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let raw = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let path = raw.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(raw.0 as *const c_void));
        path
    }
}

pub fn desktop_dir() -> Option<PathBuf> {
    known_folder(&FOLDERID_Desktop)
}

pub fn common_programs_dir() -> Option<PathBuf> {
    known_folder(&FOLDERID_CommonPrograms)
}

/// Creates (or replaces) a `.lnk` shortcut.
pub fn create_shortcut(link: &Path, target: &Path, working_dir: &Path) -> Result<(), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| -> windows::core::Result<()> {
            let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            shell.SetPath(&HSTRING::from(target.as_os_str()))?;
            shell.SetWorkingDirectory(&HSTRING::from(working_dir.as_os_str()))?;
            shell.SetIconLocation(&HSTRING::from(target.as_os_str()), 0)?;
            shell.SetDescription(&HSTRING::from("Block selected apps on a schedule"))?;
            let file: IPersistFile = shell.cast()?;
            file.Save(PCWSTR(HSTRING::from(link.as_os_str()).as_ptr()), true)
        })();
        result.map_err(|e| format!("Could not create the shortcut {}: {}", link.display(), e.message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_this_process() {
        let pid = std::process::id();
        assert!(list_processes().unwrap().iter().any(|p| p.pid == pid));
        let me = OpenedProcess::open(pid).expect("open own process");
        let exe = std::env::current_exe().unwrap();
        assert!(crate::paths::same_path(&me.image_path().unwrap(), &exe.to_string_lossy()));
        let sid = current_user_sid().expect("current SID");
        assert!(crate::config::is_valid_sid(&sid), "{sid}");
        assert_eq!(me.owner_sid(), Some(sid));
    }

    #[test]
    fn mutex_is_exclusive() {
        let name = format!(r"Local\WindowsAppBlockerTest{}", std::process::id());
        let first = single_instance(&name);
        assert!(first.is_some());
        assert!(single_instance(&name).is_none());
        drop(first);
        assert!(single_instance(&name).is_some());
    }

    #[test]
    fn finds_known_folders() {
        assert!(desktop_dir().is_some_and(|d| d.is_dir()));
        assert!(common_programs_dir().is_some_and(|d| d.is_dir()));
    }
}
