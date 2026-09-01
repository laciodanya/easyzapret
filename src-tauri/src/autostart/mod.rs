//! Login autostart and ordered boot sequence on app launch.
//!
//! EasyZapret requires administrator rights, so the HKCU Run key cannot start
//! it at logon — Explorer launches those entries unelevated and Windows skips
//! `requireAdministrator` binaries. A logon scheduled task with
//! `HighestAvailable` is what actually starts the app.
//! Zapret always starts before WARP when WARP autostart is enabled.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::settings::{self};
use crate::{logs, paths, AppState};

#[cfg(windows)]
use crate::util::run_capture;
#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
const TASK_NAME: &str = "EasyZapret";
#[cfg(windows)]
const RUN_VALUE: &str = "EasyZapret";
#[cfg(windows)]
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const STARTUP_APPROVED_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartState {
    pub launch_at_login: bool,
    pub login_entry_present: bool,
}

pub fn query_state() -> AutostartState {
    let cfg = settings::load().autostart;
    AutostartState {
        launch_at_login: cfg.launch_at_login,
        login_entry_present: login_entry_exists(),
    }
}

fn login_entry_exists() -> bool {
    #[cfg(windows)]
    {
        let (ok, _) = run_capture("schtasks", &["/Query", "/TN", TASK_NAME]);
        ok
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Strip the `\\?\` prefix that `current_exe` may add — it breaks Task Scheduler.
#[cfg_attr(not(windows), allow(dead_code))]
fn exe_path_for_task(path: &std::path::Path) -> String {
    let mut s = path.to_string_lossy().into_owned();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    s
}

#[cfg_attr(not(windows), allow(dead_code))]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg_attr(not(windows), allow(dead_code))]
fn logon_task_xml(exe: &str, workdir: &str) -> String {
    let exe = xml_escape(exe);
    let workdir = xml_escape(workdir);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Start EasyZapret at user logon</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <Delay>PT15S</Delay>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <WorkingDirectory>{workdir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
    )
}

#[cfg(windows)]
fn utf16_le_bom(text: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

#[cfg(windows)]
fn current_exe_path() -> Result<(String, String), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe_path_for_task(&exe);
    let workdir = std::path::Path::new(&exe)
        .parent()
        .ok_or_else(|| "cannot resolve install directory".to_string())?
        .to_string_lossy()
        .into_owned();
    Ok((exe, workdir))
}

#[cfg(windows)]
fn remove_legacy_run_key() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run) = hkcu.open_subkey_with_flags(RUN_SUBKEY, KEY_SET_VALUE) {
        let _ = run.delete_value(RUN_VALUE);
    }
    if let Ok(approved) = hkcu.open_subkey_with_flags(STARTUP_APPROVED_SUBKEY, KEY_SET_VALUE) {
        let _ = approved.delete_value(RUN_VALUE);
    }
}

#[cfg(windows)]
fn set_login_entry(enable: bool) -> Result<(), String> {
    // 0.5.3 wrote a Run key; Explorer cannot start this elevated exe from it.
    remove_legacy_run_key();
    if enable {
        let (exe, workdir) = current_exe_path()?;
        let xml_path = paths::tmp_dir().join("easyzapret-logon-task.xml");
        std::fs::create_dir_all(paths::tmp_dir()).map_err(|e| e.to_string())?;
        std::fs::write(&xml_path, utf16_le_bom(&logon_task_xml(&exe, &workdir)))
            .map_err(|e| e.to_string())?;
        let xml_arg = xml_path.to_string_lossy().into_owned();
        let (ok, out) = run_capture(
            "schtasks",
            &["/Create", "/TN", TASK_NAME, "/XML", &xml_arg, "/F"],
        );
        let _ = std::fs::remove_file(&xml_path);
        if !ok {
            return Err(format!("failed to register logon task: {out}"));
        }
    } else {
        let _ = run_capture("schtasks", &["/Delete", "/TN", TASK_NAME, "/F"]);
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_login_entry(_enable: bool) -> Result<(), String> {
    Err("autostart is only supported on Windows".into())
}

pub fn apply_launch_at_login(enable: bool) -> Result<(), String> {
    set_login_entry(enable)?;
    let mut s = settings::load();
    s.autostart.launch_at_login = enable;
    settings::save(&s)?;
    logs::append(
        "app",
        &format!("autostart: launch at login {}", if enable { "on" } else { "off" }),
    );
    Ok(())
}

/// Called from `setup`: never register a logon task here. Creating
/// `schtasks` on every launch is a Defender Behavior:Persistence.A!ml hit.
/// The task is created only when the user turns the setting on.
pub fn ensure_on_app_start() {
    #[cfg(windows)]
    {
        let _ = paths::ensure_dirs();
        let s = settings::load();
        if !s.autostart.launch_at_login && login_entry_exists() {
            if let Err(e) = set_login_entry(false) {
                logs::append("app", &format!("autostart: could not remove logon task — {e}"));
            }
        }
    }
}

async fn wait_for_zapret(timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if crate::warp::zapret_running() {
            return true;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    false
}

/// Runs after the UI has had a moment to paint. Zapret → WARP → Telegram Proxy.
pub async fn run_boot_sequence(app: AppHandle) {
    let cfg = settings::load().autostart;
    let wants_warp = cfg.auto_start_warp;
    let wants_zapret = cfg.auto_start_zapret || wants_warp;
    let wants_tg = cfg.auto_start_tg;

    if !wants_zapret && !wants_warp && !wants_tg {
        return;
    }

    logs::append("app", "autostart: boot sequence started");

    if wants_zapret && !crate::warp::zapret_running() {
        let service_running =
            crate::zapret::service::query_service_state("zapret").as_deref() == Some("RUNNING");
        if !service_running {
            let strategy = settings::load()
                .selected_strategy
                .unwrap_or_else(|| "general.bat".into());
            let strategy_log = strategy.clone();
            let app_handle = app.clone();
            let started = tokio::task::spawn_blocking(move || {
                let state = app_handle.state::<AppState>();
                crate::zapret::process::start_strategy(&state, &strategy)
            })
            .await;

            match started {
                Ok(Ok(())) => logs::append("app", &format!("autostart: zapret started ({strategy_log})")),
                Ok(Err(e)) => logs::append("app", &format!("autostart: zapret failed — {e}")),
                Err(e) => logs::append("app", &format!("autostart: zapret task failed — {e}")),
            }
        }

        if wants_warp && !wait_for_zapret(Duration::from_secs(30)).await {
            logs::append("app", "autostart: zapret not ready — skipping WARP");
        }
    }

    if wants_warp && crate::warp::is_installed() {
        let state = app.state::<AppState>();
        if crate::warp::zapret_running() && !crate::warp::quick_status().connected {
            match crate::warp::connect_with_state(&state) {
                Ok(()) => logs::append("app", "autostart: WARP connected"),
                Err(e) => logs::append("app", &format!("autostart: WARP failed — {e}")),
            }
        }
    }

    if wants_tg && paths::tg_exe().exists() && !crate::tg_proxy::proxy_running() {
        match crate::tg_proxy::start_tg() {
            Ok(()) => logs::append("app", "autostart: Telegram Proxy started"),
            Err(e) => logs::append("app", &format!("autostart: Telegram Proxy failed — {e}")),
        }
    }

    crate::tray::update_tray_now(&app);
    logs::append("app", "autostart: boot sequence finished");
}

pub fn maybe_hide_on_start(app: &AppHandle) {
    let cfg = settings::load().autostart;
    if !cfg.start_minimized {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn get_autostart_state() -> AutostartState {
    query_state()
}

#[tauri::command]
pub fn set_launch_at_login(enabled: bool) -> Result<AutostartState, String> {
    apply_launch_at_login(enabled)?;
    Ok(query_state())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn exe_path_for_task_strips_verbatim_prefix() {
        assert_eq!(
            exe_path_for_task(Path::new(r"\\?\C:\Program Files\EasyZapret\EasyZapret.exe")),
            r"C:\Program Files\EasyZapret\EasyZapret.exe"
        );
    }

    #[test]
    fn logon_task_xml_escapes_and_contains_highest_run_level() {
        let xml = logon_task_xml(
            r"C:\Program Files\EasyZapret\EasyZapret.exe",
            r"C:\Program Files\EasyZapret",
        );
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<LogonTrigger>"));
        assert!(xml.contains(r"<Command>C:\Program Files\EasyZapret\EasyZapret.exe</Command>"));
        assert!(xml.contains("<Delay>PT15S</Delay>"));
        assert_eq!(xml_escape(r"a&b<c>"), "a&amp;b&lt;c&gt;");
    }
}
