//! Cloudflare WARP integration via the official `warp-cli`.
//!
//! In several regions WARP only succeeds when a DPI-bypass (Zapret) is already
//! running, so this module enforces a hard dependency: WARP can only be
//! connected while Zapret is up, and is auto-disconnected the moment Zapret
//! stops. We never bundle WARP itself — the user installs the official 1.1.1.1
//! client and we drive it through `warp-cli`.

use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use crate::util;
use crate::{logs, AppState};

/// Process-name substrings that commonly break WARP + Zapret together.
const CONFLICT_MARKERS: &[(&str, &str)] = &[
    ("exitlag", "ExitLag"),
    ("gearup", "GearUP"),
    ("gear up", "GearUP"),
    ("wtfast", "WTFast"),
    ("mudfish", "Mudfish"),
    ("goodbyedpi", "GoodbyeDPI"),
    ("zapretgui", "ZapretGUI"),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarpStatus {
    /// `warp-cli` is present on the machine.
    pub installed: bool,
    /// Tunnel is currently connected.
    pub connected: bool,
    /// Current mode (warp | doh | warp+doh) when known.
    pub mode: Option<String>,
    /// Device has a valid registration / keys.
    pub registered: bool,
    /// Raw `warp-cli status` line, for diagnostics.
    pub detail: Option<String>,
}

impl WarpStatus {
    fn not_installed() -> Self {
        Self {
            installed: false,
            connected: false,
            mode: None,
            registered: false,
            detail: None,
        }
    }
}

struct CliCache {
    path: Option<String>,
    checked_at: Option<Instant>,
}

static CLI_CACHE: Mutex<CliCache> = Mutex::new(CliCache {
    path: None,
    checked_at: None,
});

#[cfg_attr(not(windows), allow(dead_code))]
const WARP_CLI_PATHS: &[&str] = &[
    "C:\\Program Files\\Cloudflare\\Cloudflare WARP\\warp-cli.exe",
    "C:\\Program Files (x86)\\Cloudflare\\Cloudflare WARP\\warp-cli.exe",
];

fn detect_warp_cli() -> Option<String> {
    #[cfg(windows)]
    {
        for p in WARP_CLI_PATHS {
            if std::path::Path::new(p).exists() {
                return Some((*p).to_string());
            }
        }
        // EasyZapret is always elevated. Falling back to a bare executable
        // name would let an unprivileged user place warp-cli.exe earlier in
        // PATH and gain administrator code execution.
        None
    }

    #[cfg(not(windows))]
    {
        // Non-Windows builds are development-only and may use PATH.
        let prog = "warp-cli";
        let (ok, _) = util::run_capture_timeout(prog, &["--version"], Duration::from_secs(4));
        if ok {
            Some(prog.to_string())
        } else {
            None
        }
    }
}

/// Resolves the `warp-cli` program, caching the result. A found path is cached
/// permanently; a miss is re-probed at most every 10s so installing WARP while
/// the app is open is picked up without a restart.
fn warp_cli() -> Option<String> {
    {
        let cache = crate::util::lock_mutex(&CLI_CACHE);
        if let Some(ref p) = cache.path {
            return Some(p.clone());
        }
        if let Some(t) = cache.checked_at {
            if t.elapsed() < Duration::from_secs(10) {
                return None;
            }
        }
    }
    let resolved = detect_warp_cli();
    let mut cache = crate::util::lock_mutex(&CLI_CACHE);
    cache.checked_at = Some(Instant::now());
    cache.path = resolved.clone();
    resolved
}

/// Runs `warp-cli` with the modern subcommand form, falling back to the legacy
/// form on older clients that don't recognise it.
fn run(cli: &str, modern: &[&str], legacy: &[&str]) -> (bool, String) {
    let mut args = vec!["--accept-tos"];
    args.extend_from_slice(modern);
    let (ok, out) = util::run_capture_timeout(cli, &args, Duration::from_secs(4));
    if ok {
        return (true, out);
    }
    let low = out.to_lowercase();
    let looks_like_bad_syntax = low.contains("unrecognized")
        || low.contains("unexpected")
        || low.contains("invalid")
        || low.contains("usage:")
        || low.contains("subcommand")
        || low.contains("--help");
    if !legacy.is_empty() && looks_like_bad_syntax {
        let (ok2, out2) = util::run_capture_timeout(cli, legacy, Duration::from_secs(4));
        if ok2 || looks_like_bad_syntax {
            return (ok2, out2);
        }
    }
    (ok, out)
}

/// True when Zapret is active either as our manual winws or as the installed service.
pub fn zapret_running() -> bool {
    crate::zapret::process::winws_running()
        || crate::zapret::service::query_service_state("zapret").as_deref() == Some("RUNNING")
}

fn parse_connected(status_out: &str) -> bool {
    let low = status_out.to_lowercase();
    // "disconnected" / "connecting" both contain "connected", so exclude them.
    low.contains("connected") && !low.contains("disconnected") && !low.contains("connecting")
}

/// Maps warp-cli's wording ("Warp", "WarpWithDnsOverHttps", "DnsOverHttps",
/// "warp+doh", ...) to the identifiers `warp-cli mode` accepts.
fn normalize_mode(raw: &str) -> Option<String> {
    let low: String = raw
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+')
        .collect();
    if low.is_empty() {
        return None;
    }
    let has_warp = low.contains("warp");
    let doh = low.contains("dnsoverhttps") || low.contains("doh");
    let dot = low.contains("dnsovertls") || low.contains("dot");
    let mode = if low.contains("proxy") {
        "proxy"
    } else if low.contains("tunnelonly") {
        "tunnel_only"
    } else if has_warp && doh {
        "warp+doh"
    } else if has_warp && dot {
        "warp+dot"
    } else if doh {
        "doh"
    } else if dot {
        "dot"
    } else if has_warp {
        "warp"
    } else {
        return None;
    };
    Some(mode.to_string())
}

/// Reads the `Mode:` line of `warp-cli settings`, ignoring look-alikes such as
/// "Families Mode" and the "(user set)" provenance prefix.
fn parse_mode(settings_out: &str) -> Option<String> {
    for line in settings_out.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.rsplit(')').next().unwrap_or(key).trim().to_lowercase();
        let is_mode = key == "mode"
            || (key.ends_with("mode") && !key.contains("famil") && !key.contains("always"));
        if is_mode {
            if let Some(mode) = normalize_mode(value) {
                return Some(mode);
            }
        }
    }
    None
}

/// Cheap status used by the polling endpoint and tray: a single `warp-cli`
/// call when WARP is installed, nothing otherwise.
pub fn quick_status() -> WarpStatus {
    let Some(cli) = warp_cli() else {
        return WarpStatus::not_installed();
    };
    let (_, out) = run(&cli, &["status"], &["status"]);
    WarpStatus {
        installed: true,
        connected: parse_connected(&out),
        mode: None,
        registered: false,
        detail: Some(out.trim().to_string()),
    }
}

/// Full status with mode and registration — used by the WARP page on demand.
#[tauri::command]
pub fn warp_details() -> WarpStatus {
    let Some(cli) = warp_cli() else {
        return WarpStatus::not_installed();
    };
    let mut status = quick_status();
    let (_, settings_out) = run(&cli, &["settings"], &["settings"]);
    status.mode = parse_mode(&settings_out);
    let (reg_ok, reg_out) = run(&cli, &["registration", "show"], &["account"]);
    let low = reg_out.to_lowercase();
    status.registered = reg_ok
        && !low.contains("missing")
        && !low.contains("not registered")
        && !low.contains("no registration");
    status
}

pub fn is_installed() -> bool {
    warp_cli().is_some()
}

#[tauri::command]
pub fn warp_connect(state: State<'_, AppState>) -> Result<(), String> {
    connect_with_state(&state)
}

pub fn connect_with_state(state: &AppState) -> Result<(), String> {
    let _transition = crate::vpn::without_vpn()?;
    let Some(cli) = warp_cli() else {
        return Err("warp_not_installed".into());
    };
    if !zapret_running() {
        logs::warn("warp", "connect refused: Zapret is not running");
        return Err("zapret_required".into());
    }

    // Make sure we have a registration; create one on first use.
    let (_, reg_out) = run(&cli, &["registration", "show"], &["account"]);
    let low = reg_out.to_lowercase();
    if low.contains("missing") || low.contains("not registered") || low.contains("no registration")
    {
        let (ok, out) = run(&cli, &["registration", "new"], &["register"]);
        if !ok {
            return Err(format!("warp registration failed: {}", out.trim()));
        }
        logs::append("warp", "Created new WARP registration");
    }

    let (ok, out) = run(&cli, &["connect"], &["connect"]);
    if !ok {
        logs::error("warp", &format!("connect failed: {}", out.trim()));
        return Err(format!("warp connect failed: {}", out.trim()));
    }
    state.warp_active.store(true, Ordering::SeqCst);
    logs::append(
        "warp",
        &format!("connect requested: {}", out.trim().replace('\n', " ")),
    );
    Ok(())
}

#[tauri::command]
pub fn warp_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let _transition = util::lock_mutex(&crate::NETWORK_TRANSITION);
    let Some(cli) = warp_cli() else {
        return Err("warp_not_installed".into());
    };
    state.warp_active.store(false, Ordering::SeqCst);
    let (ok, out) = run(&cli, &["disconnect"], &["disconnect"]);
    if !ok {
        logs::error("warp", &format!("disconnect failed: {}", out.trim()));
        return Err(format!("warp disconnect failed: {}", out.trim()));
    }
    logs::append("warp", "disconnected");
    Ok(())
}

fn confirmed_disconnected(output: &str) -> bool {
    output.lines().any(|line| {
        let line = line.trim().to_ascii_lowercase();
        line == "disconnected"
            || line.starts_with("status update: disconnected")
            || line.starts_with("status: disconnected")
            || line.contains("registration missing")
            || line.contains("not registered")
    })
}

/// Called under NETWORK_TRANSITION before VPN opens any sockets or adapters.
pub fn disconnect_for_vpn(state: &AppState) -> Result<(), String> {
    let Some(cli) = warp_cli()
        .filter(|path| std::path::Path::new(path).is_file())
        .or_else(detect_warp_cli)
    else {
        state.warp_active.store(false, Ordering::SeqCst);
        return Ok(());
    };
    let (_, status) = run(&cli, &["status"], &["status"]);
    if !confirmed_disconnected(&status) {
        let (_, disconnect) = run(&cli, &["disconnect"], &["disconnect"]);
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            let (_, status) = run(&cli, &["status"], &["status"]);
            if confirmed_disconnected(&status) {
                break;
            }
            // The CLI cannot talk to a stopped Windows daemon.
            if status.to_ascii_lowercase().contains("daemon")
                && crate::zapret::service::query_service_state("CloudflareWARP").as_deref()
                    == Some("STOPPED")
            {
                break;
            }
            if Instant::now() >= deadline {
                logs::error(
                    "vpn",
                    &format!("WARP disconnect could not be confirmed: {disconnect}; {status}"),
                );
                return Err("vpn_warp_stop_failed".into());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
    state.warp_active.store(false, Ordering::SeqCst);
    Ok(())
}

/// Resets encryption keys the same way the official client's "Reset" does:
/// delete the current registration and create a fresh one (new WireGuard keys).
#[tauri::command]
pub fn warp_reset_keys(state: State<'_, AppState>) -> Result<(), String> {
    let _transition = crate::vpn::without_vpn()?;
    let Some(cli) = warp_cli() else {
        return Err("warp_not_installed".into());
    };
    let was_connected = quick_status().connected;

    let _ = run(&cli, &["disconnect"], &["disconnect"]);
    let _ = run(&cli, &["registration", "delete"], &["delete"]);

    let (ok, out) = run(&cli, &["registration", "new"], &["register"]);
    if !ok {
        state.warp_active.store(false, Ordering::SeqCst);
        return Err(format!("warp re-registration failed: {}", out.trim()));
    }
    logs::append("warp", "WARP encryption keys reset (new registration)");

    // Reconnect only if it was on and Zapret is still up.
    if was_connected && zapret_running() {
        let (_, _) = run(&cli, &["connect"], &["connect"]);
        state.warp_active.store(true, Ordering::SeqCst);
    } else {
        state.warp_active.store(false, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub fn warp_set_mode(mode: String) -> Result<String, String> {
    let _transition = crate::vpn::without_vpn()?;
    let Some(cli) = warp_cli() else {
        return Err("warp_not_installed".into());
    };
    if !["warp", "doh", "warp+doh"].contains(&mode.as_str()) {
        return Err(format!("unknown warp mode: {mode}"));
    }
    let (ok, out) = run(&cli, &["mode", &mode], &["set-mode", &mode]);
    if !ok {
        logs::error("warp", &format!("set mode {mode} failed: {}", out.trim()));
        return Err(format!("warp set mode failed: {}", out.trim()));
    }
    // Read the setting back so the UI shows what the client really uses.
    let (_, settings_out) = run(&cli, &["settings"], &["settings"]);
    let applied = parse_mode(&settings_out).unwrap_or_else(|| mode.clone());
    logs::append(
        "warp",
        &format!("mode set to {mode} (client reports {applied})"),
    );
    Ok(applied)
}

/// Kept for status-poll call sites. Connecting still requires Zapret, but once
/// WARP is up it stays connected even if Zapret is stopped.
pub fn enforce_dependency(_state: &AppState) {}

/// Disconnects WARP unconditionally (used on app quit).
pub fn disconnect_quiet() {
    if let Some(cli) = warp_cli() {
        let _ = run(&cli, &["disconnect"], &["disconnect"]);
    }
}

/// Detects third-party bypass / gaming-VPN tools that often conflict with WARP.
fn conflict_app_name(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    let name = name.strip_suffix(".exe")?;
    CONFLICT_MARKERS.iter().find_map(|(marker, label)| {
        (name == *marker
            || name.strip_prefix(marker).is_some_and(|suffix| {
                [
                    "launcher",
                    "service",
                    "svc",
                    "booster",
                    "boosterservice",
                    "booster_ball",
                    "booster_render",
                    "booster_translate",
                    "client",
                    "helper",
                    "tray",
                    "ui",
                    "cli",
                    "gui",
                ]
                .contains(&suffix.trim_start_matches([' ', '-', '_']))
            }))
        .then_some(*label)
    })
}

fn conflict_app(process: &sysinfo::Process) -> Option<&'static str> {
    let name = process.name().to_string_lossy();
    conflict_app_name(&name).or_else(|| {
        if !name.eq_ignore_ascii_case("launcher.exe") {
            return None;
        }
        // Generic launchers are only related to a booster when their own executable
        // sits under the corresponding product directory.
        for ancestor in process.exe()?.ancestors().skip(1) {
            let Some(directory) = ancestor.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if directory.eq_ignore_ascii_case("GearUPBooster") {
                return Some("GearUP");
            }
            if directory.eq_ignore_ascii_case("ExitLag") {
                return Some("ExitLag");
            }
        }
        None
    })
}

fn conflict_apps(system: &sysinfo::System) -> Vec<String> {
    let mut apps: Vec<_> = system
        .processes()
        .values()
        .filter_map(conflict_app)
        .collect();
    apps.sort_unstable();
    apps.dedup();
    apps.into_iter().map(str::to_owned).collect()
}

#[tauri::command]
pub fn warp_conflict_apps() -> Result<Vec<String>, String> {
    util::process_snapshot()
        .as_ref()
        .map(conflict_apps)
        .ok_or_else(|| "warp_process_scan_failed".into())
}

#[tauri::command]
pub async fn warp_close_conflict_apps() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(|| {
        let system = util::process_snapshot().ok_or("warp_process_scan_failed")?;
        let mut attempted = false;
        for process in system
            .processes()
            .values()
            .filter(|p| conflict_app(p).is_some())
        {
            attempted = true;
            if !process.kill() {
                logs::warn(
                    "warp",
                    &format!("Could not close conflicting process {}", process.pid()),
                );
            }
        }
        if attempted {
            std::thread::sleep(Duration::from_millis(700));
        }
        warp_conflict_apps()
    })
    .await
    .map_err(|_| "warp_process_scan_failed")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_known_conflicting_executable_names() {
        assert_eq!(conflict_app_name("ExitLagService.exe"), Some("ExitLag"));
        assert_eq!(conflict_app_name("GearUP Booster.exe"), Some("GearUP"));
        assert_eq!(conflict_app_name("gearup_booster_ball.exe"), Some("GearUP"));
        assert_eq!(conflict_app_name("GoodbyeDPI.exe"), Some("GoodbyeDPI"));
        assert_eq!(conflict_app_name("exitlag-notes.exe"), None);
        assert_eq!(conflict_app_name("notexitlag.exe"), None);
        assert_eq!(conflict_app_name("gearup.dll"), None);
    }

    #[test]
    fn confirms_warp_is_off_before_switching() {
        assert!(confirmed_disconnected("Status update: Disconnected"));
        assert!(confirmed_disconnected(
            "Status update: Unable\nReason: Registration Missing"
        ));
        assert!(!confirmed_disconnected("Status update: Disconnecting"));
        assert!(!confirmed_disconnected("Status update: Connected"));
        assert!(!confirmed_disconnected(
            "Unable to connect to the WARP daemon"
        ));
    }

    #[test]
    fn normalizes_warp_cli_mode_names() {
        assert_eq!(normalize_mode("Warp").as_deref(), Some("warp"));
        assert_eq!(
            normalize_mode(" WarpWithDnsOverHttps ").as_deref(),
            Some("warp+doh")
        );
        assert_eq!(normalize_mode("DnsOverHttps").as_deref(), Some("doh"));
        assert_eq!(normalize_mode("warp+doh").as_deref(), Some("warp+doh"));
        assert_eq!(normalize_mode("doh").as_deref(), Some("doh"));
        assert_eq!(
            normalize_mode("WarpWithDnsOverTls").as_deref(),
            Some("warp+dot")
        );
        assert_eq!(
            normalize_mode("WarpProxy on port 40000").as_deref(),
            Some("proxy")
        );
        assert_eq!(normalize_mode(""), None);
    }

    #[test]
    fn reads_mode_line_from_settings_output() {
        let out = "Merged configuration\n\
                   (user set)\tAlways On: true\n\
                   (user set)\tFamilies Mode: Off\n\
                   (user set)\tMode: WarpWithDnsOverHttps\n\
                   (network policy)\tResolve via: engage.cloudflareclient.com\n";
        assert_eq!(parse_mode(out).as_deref(), Some("warp+doh"));
        assert_eq!(parse_mode("Mode: Warp\n").as_deref(), Some("warp"));
        assert_eq!(parse_mode("Families Mode: Off\n"), None);
    }
}
