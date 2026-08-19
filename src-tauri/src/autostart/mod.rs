//! Login autostart (Windows Run key) and ordered boot sequence on app launch.
//! Zapret always starts before WARP when WARP autostart is enabled.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::settings::{self};
use crate::{logs, paths, AppState};

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, RegType};
#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use winreg::RegValue;

#[cfg(windows)]
const RUN_VALUE: &str = "EasyZapret";
#[cfg(windows)]
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const STARTUP_APPROVED_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// Marker so 0.5.3+ enables login autostart once for existing installs,
/// without fighting the user if they later turn it off.
#[cfg(windows)]
fn login_enable_marker() -> std::path::PathBuf {
    paths::data_dir().join(".autostart-login-enabled")
}

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
        run_key_command().ok().is_some()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Quote the exe path for a Run-key command line, stripping the `\\?\` prefix
/// that `current_exe`/`canonicalize` may add — that prefix breaks startup.
#[cfg_attr(not(windows), allow(dead_code))]
fn quote_exe_path(path: &std::path::Path) -> String {
    let mut s = path.to_string_lossy().into_owned();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    format!("\"{s}\"")
}

#[cfg(windows)]
fn login_command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(quote_exe_path(&exe))
}

#[cfg(windows)]
fn run_key_command() -> Result<String, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(RUN_SUBKEY).map_err(|e| e.to_string())?;
    key.get_value::<String, _>(RUN_VALUE).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn set_startup_approved(enabled: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(STARTUP_APPROVED_SUBKEY)
        .map_err(|e| e.to_string())?;
    if enabled {
        // 02 00 00 00 + zeros = enabled in Task Manager > Startup.
        let value = RegValue {
            bytes: vec![0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            vtype: RegType::REG_BINARY,
        };
        key.set_raw_value(RUN_VALUE, &value).map_err(|e| e.to_string())?;
    } else {
        let _ = key.delete_value(RUN_VALUE);
    }
    Ok(())
}

#[cfg(windows)]
fn set_login_entry(enable: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run, _) = hkcu.create_subkey(RUN_SUBKEY).map_err(|e| e.to_string())?;
    if enable {
        let command = login_command()?;
        run.set_value(RUN_VALUE, &command).map_err(|e| e.to_string())?;
        set_startup_approved(true)?;
    } else {
        let _ = run.delete_value(RUN_VALUE);
        let _ = set_startup_approved(false);
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

/// Called once from `setup`: turn login autostart on for every 0.5.3+ user,
/// then keep the Run key in sync with the current exe path.
pub fn ensure_on_app_start() {
    #[cfg(windows)]
    {
        let _ = paths::ensure_dirs();
        let marker = login_enable_marker();
        let mut s = settings::load();
        if !marker.exists() {
            s.autostart.launch_at_login = true;
            if let Err(e) = settings::save(&s) {
                logs::append("app", &format!("autostart: failed to save default on — {e}"));
            }
            if let Err(e) = std::fs::write(&marker, "1") {
                logs::append("app", &format!("autostart: failed to write marker — {e}"));
            }
            logs::append("app", "autostart: launch at login enabled for this version");
        }
        match set_login_entry(s.autostart.launch_at_login) {
            Ok(()) => {
                if s.autostart.launch_at_login {
                    logs::append("app", "autostart: Run key registered");
                }
            }
            Err(e) => logs::append("app", &format!("autostart: Run key failed — {e}")),
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
    fn quote_exe_path_wraps_and_strips_verbatim_prefix() {
        assert_eq!(
            quote_exe_path(Path::new(r"\\?\C:\Program Files\EasyZapret\EasyZapret.exe")),
            r#""C:\Program Files\EasyZapret\EasyZapret.exe""#
        );
        assert_eq!(
            quote_exe_path(Path::new(r"C:\Program Files\EasyZapret\EasyZapret.exe")),
            r#""C:\Program Files\EasyZapret\EasyZapret.exe""#
        );
    }
}
