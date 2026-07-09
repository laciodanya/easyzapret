//! Login autostart (Windows Run key) and ordered boot sequence on app launch.
//! Zapret always starts before WARP when WARP autostart is enabled.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::settings::{self};
use crate::{logs, paths, AppState};

#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_VALUE: &str = "EasyZapret";

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
        let (ok, _) = crate::util::run_capture("reg", &["query", RUN_KEY, "/v", RUN_VALUE]);
        ok
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
fn set_login_entry(enable: bool) -> Result<(), String> {
    if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let quoted = format!("\"{}\"", exe.to_string_lossy());
        let (ok, out) = crate::util::run_capture(
            "reg",
            &[
                "add",
                RUN_KEY,
                "/v",
                RUN_VALUE,
                "/t",
                "REG_SZ",
                "/d",
                &quoted,
                "/f",
            ],
        );
        if !ok {
            return Err(format!("failed to add Run entry: {out}"));
        }
    } else {
        let (ok, out) = crate::util::run_capture("reg", &["delete", RUN_KEY, "/v", RUN_VALUE, "/f"]);
        if !ok && !out.to_lowercase().contains("cannot find") {
            return Err(format!("failed to remove Run entry: {out}"));
        }
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
