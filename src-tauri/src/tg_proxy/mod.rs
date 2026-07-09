use std::path::PathBuf;

use serde::Serialize;
use sysinfo::{ProcessesToUpdate, System};

use crate::{logs, paths};

/// tg-ws-proxy stores its config in %APPDATA%/TgWsProxy (see TrayConfig.md).
fn tg_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("TgWsProxy"))
}

fn read_tg_config() -> Option<serde_json::Value> {
    let path = tg_config_dir()?.join("config.json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Newest .log/.txt log file from the proxy's own config directory.
pub fn own_log_file() -> Option<PathBuf> {
    let dir = tg_config_dir()?;
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("log") || ext.eq_ignore_ascii_case("txt") {
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if newest.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                        newest = Some((modified, path));
                    }
                }
            }
        }
    }
    newest.map(|(_, p)| p)
}

pub fn proxy_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .any(|p| p.name().to_string_lossy().to_lowercase().starts_with("tgwsproxy"))
}

pub fn kill_all() {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    for p in sys.processes().values() {
        if p.name().to_string_lossy().to_lowercase().starts_with("tgwsproxy") {
            p.kill();
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TgStatus {
    pub installed: bool,
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub secret: Option<String>,
    /// tg://proxy?... deep link ("Open in Telegram")
    pub tg_link: Option<String>,
    /// https://t.me/proxy?... shareable link
    pub share_link: Option<String>,
}

#[tauri::command]
pub fn tg_status() -> TgStatus {
    build_tg_status(proxy_running())
}

/// Status for the hot poll path — caller supplies a cached running flag.
pub fn tg_status_cached(running: bool) -> TgStatus {
    build_tg_status(running)
}

fn build_tg_status(running: bool) -> TgStatus {
    let config = read_tg_config();
    let host = config
        .as_ref()
        .and_then(|c| c.get("host"))
        .and_then(|v| v.as_str())
        .unwrap_or("127.0.0.1")
        .to_string();
    let port = config
        .as_ref()
        .and_then(|c| c.get("port"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1443) as u16;
    let secret = config
        .as_ref()
        .and_then(|c| c.get("secret"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let (tg_link, share_link) = match &secret {
        Some(s) => (
            Some(format!("tg://proxy?server={host}&port={port}&secret={s}")),
            Some(format!("https://t.me/proxy?server={host}&port={port}&secret={s}")),
        ),
        None => (None, None),
    };

    TgStatus {
        installed: paths::tg_exe().exists(),
        running,
        host,
        port,
        secret,
        tg_link,
        share_link,
    }
}

#[tauri::command]
pub fn start_tg() -> Result<(), String> {
    let exe = paths::tg_exe();
    if !exe.exists() {
        return Err("not_installed".into());
    }
    if proxy_running() {
        return Ok(());
    }
    // TgWsProxy is a self-contained tray application; just launch it detached.
    std::process::Command::new(&exe)
        .current_dir(paths::tg_dir())
        .spawn()
        .map_err(|e| format!("failed to start TgWsProxy: {e}"))?;
    logs::append("tgproxy", "TgWsProxy started");
    Ok(())
}

#[tauri::command]
pub fn stop_tg() -> Result<(), String> {
    kill_all();
    logs::append("tgproxy", "TgWsProxy stopped");
    Ok(())
}
