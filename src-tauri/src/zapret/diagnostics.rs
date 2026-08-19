use serde::Serialize;

#[cfg(windows)]
use crate::util::run_capture;
#[cfg(windows)]
use crate::{logs, paths};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagItem {
    pub id: String,
    /// ok | warn | fail
    pub status: String,
    pub detail: String,
}

fn item(id: &str, status: &str, detail: impl Into<String>) -> DiagItem {
    DiagItem { id: id.into(), status: status.into(), detail: detail.into() }
}

/// Run Diagnostics — the same checks service.bat performs, returned as
/// structured items for the UI. Auto-fixes (TCP timestamps, orphaned
/// WinDivert) are applied the way the bat does.
#[tauri::command]
pub fn run_diagnostics() -> Vec<DiagItem> {
    #[cfg(not(windows))]
    {
        return vec![item("platform", "warn", "Diagnostics are available on Windows only")];
    }
    #[cfg(windows)]
    {
        let mut out = Vec::new();

        // Base Filtering Engine
        let (_, bfe) = run_capture("sc", &["query", "BFE"]);
        if bfe.to_uppercase().contains("RUNNING") {
            out.push(item("bfe", "ok", ""));
        } else {
            out.push(item("bfe", "fail", "Base Filtering Engine is not running"));
        }

        // System proxy
        let (_, proxy_enable) = run_capture(
            "reg",
            &["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings", "/v", "ProxyEnable"],
        );
        if proxy_enable.contains("0x1") {
            let (_, proxy_server) = run_capture(
                "reg",
                &["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings", "/v", "ProxyServer"],
            );
            let server = proxy_server
                .lines()
                .find(|l| l.contains("ProxyServer"))
                .and_then(|l| l.find("REG_SZ").map(|p| l[p + 6..].trim().to_string()))
                .unwrap_or_default();
            out.push(item("proxy", "warn", server));
        } else {
            out.push(item("proxy", "ok", ""));
        }

        // TCP timestamps (auto-enable like the bat)
        let (_, tcp) = run_capture("netsh", &["interface", "tcp", "show", "global"]);
        let ts_enabled = tcp
            .lines()
            .any(|l| l.to_lowercase().contains("timestamps") && l.to_lowercase().contains("enabled"));
        if ts_enabled {
            out.push(item("tcp_timestamps", "ok", ""));
        } else {
            let (fixed, _) = run_capture("netsh", &["interface", "tcp", "set", "global", "timestamps=enabled"]);
            out.push(if fixed {
                item("tcp_timestamps", "ok", "auto-enabled")
            } else {
                item("tcp_timestamps", "fail", "failed to enable TCP timestamps")
            });
        }

        // Adguard
        let (_, tasks) = run_capture("tasklist", &["/FI", "IMAGENAME eq AdguardSvc.exe"]);
        if tasks.to_lowercase().contains("adguardsvc.exe") {
            out.push(item("adguard", "fail", "https://github.com/Flowseal/zapret-discord-youtube/issues/417"));
        } else {
            out.push(item("adguard", "ok", ""));
        }

        // Conflicting vendor services found by name in the running service list
        let (_, services) = run_capture("sc", &["query"]);
        let services_upper = services.to_uppercase();

        if services_upper.contains("KILLER") {
            out.push(item("killer", "fail", "Killer services conflict with zapret"));
        } else {
            out.push(item("killer", "ok", ""));
        }

        if services_upper.contains("INTEL") && services_upper.contains("CONNECTIVITY") && services_upper.contains("NETWORK") {
            out.push(item("intel_connectivity", "fail", "Intel Connectivity Network Service conflicts with zapret"));
        } else {
            out.push(item("intel_connectivity", "ok", ""));
        }

        if services_upper.contains("TRACSRVWRAPPER") || services_upper.contains("EPWD") {
            out.push(item("checkpoint", "fail", "Check Point services conflict with zapret"));
        } else {
            out.push(item("checkpoint", "ok", ""));
        }

        if services_upper.contains("SMARTBYTE") {
            out.push(item("smartbyte", "fail", "SmartByte services conflict with zapret"));
        } else {
            out.push(item("smartbyte", "ok", ""));
        }

        // WinDivert64.sys present
        if paths::zapret_bin_dir().join("WinDivert64.sys").exists() {
            out.push(item("windivert_sys", "ok", ""));
        } else {
            out.push(item("windivert_sys", "fail", "WinDivert64.sys not found (antivirus may have removed it)"));
        }

        // VPN services
        let vpn_lines: Vec<String> = services
            .lines()
            .filter(|l| l.to_uppercase().contains("VPN"))
            .filter_map(|l| l.split(':').nth(1).map(|s| s.trim().to_string()))
            .collect();
        if vpn_lines.is_empty() {
            out.push(item("vpn", "ok", ""));
        } else {
            out.push(item("vpn", "warn", vpn_lines.join(", ")));
        }

        // Secure DNS (DoH)
        let (_, doh) = run_capture(
            "powershell",
            &["-NoProfile", "-Command",
              "Get-ChildItem -Recurse -Path 'HKLM:System\\CurrentControlSet\\Services\\Dnscache\\InterfaceSpecificParameters\\' | Get-ItemProperty | Where-Object { $_.DohFlags -gt 0 } | Measure-Object | Select-Object -ExpandProperty Count"],
        );
        let doh_count: i64 = doh.trim().lines().last().and_then(|l| l.trim().parse().ok()).unwrap_or(0);
        if doh_count > 0 {
            out.push(item("secure_dns", "ok", ""));
        } else {
            out.push(item("secure_dns", "warn", "Encrypted DNS not detected"));
        }

        // hosts entries for YouTube
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let hosts = std::fs::read_to_string(format!("{root}\\System32\\drivers\\etc\\hosts")).unwrap_or_default();
        let hosts_lower = hosts.to_lowercase();
        if hosts_lower.contains("youtube.com") || hosts_lower.contains("youtu.be") {
            out.push(item("hosts_youtube", "warn", "hosts file contains youtube.com / youtu.be entries"));
        } else {
            out.push(item("hosts_youtube", "ok", ""));
        }

        // Orphaned WinDivert (winws not running but driver service active)
        let winws_running = super::process::winws_running();
        let windivert_state = super::service::query_service_state("WinDivert");
        let windivert_active = matches!(windivert_state.as_deref(), Some("RUNNING") | Some("STOP_PENDING"));
        if !winws_running && windivert_active {
            let _ = run_capture("net", &["stop", "WinDivert"]);
            let _ = run_capture("sc", &["delete", "WinDivert"]);
            if super::service::query_service_state("WinDivert").is_some() {
                out.push(item("windivert_orphan", "fail", "WinDivert is held by another bypass and could not be removed"));
            } else {
                out.push(item("windivert_orphan", "warn", "Orphaned WinDivert service was removed"));
            }
        } else {
            out.push(item("windivert_orphan", "ok", ""));
        }

        // Known conflicting bypass services
        let mut conflicts = Vec::new();
        for svc in ["GoodbyeDPI", "discordfix_zapret", "winws1", "winws2"] {
            if super::service::query_service_state(svc).is_some() {
                conflicts.push(svc.to_string());
            }
        }
        if conflicts.is_empty() {
            out.push(item("conflicting_bypasses", "ok", ""));
        } else {
            out.push(item("conflicting_bypasses", "fail", conflicts.join(", ")));
        }

        logs::append("app", "Diagnostics executed");
        out
    }
}

/// Removes the conflicting bypass services found by diagnostics (same list
/// and same commands as service.bat, including the WinDivert cleanup).
#[tauri::command]
pub fn remove_conflicting_services() -> Result<Vec<String>, String> {
    #[cfg(not(windows))]
    {
        return Err("Windows only".into());
    }
    #[cfg(windows)]
    {
        let mut removed = Vec::new();
        for svc in ["GoodbyeDPI", "discordfix_zapret", "winws1", "winws2"] {
            if super::service::query_service_state(svc).is_some() {
                let _ = run_capture("net", &["stop", svc]);
                let (ok, _) = run_capture("sc", &["delete", svc]);
                if ok {
                    removed.push(svc.to_string());
                }
            }
        }
        let _ = run_capture("net", &["stop", "WinDivert"]);
        let _ = run_capture("sc", &["delete", "WinDivert"]);
        let _ = run_capture("net", &["stop", "WinDivert14"]);
        let _ = run_capture("sc", &["delete", "WinDivert14"]);
        logs::append("app", &format!("Removed conflicting services: {removed:?}"));
        Ok(removed)
    }
}

/// Discord editions whose cache service.bat can wipe (Stable, PTB, Canary, Development).
/// `(process, display name, %APPDATA% folder)`
#[cfg(windows)]
const DISCORD_CACHE_EDITIONS: &[(&str, &str, &str)] = &[
    ("Discord.exe", "Discord", "discord"),
    ("DiscordPTB.exe", "Discord PTB", "discordptb"),
    ("DiscordCanary.exe", "Discord Canary", "discordcanary"),
    ("DiscordDevelopment.exe", "Discord Development", "discorddevelopment"),
];

/// Clears Discord cache directories (Cache, Code Cache, GPUCache) for each
/// installed edition, closing that edition first — same as service.bat.
#[tauri::command]
pub fn clear_discord_cache() -> Result<Vec<String>, String> {
    #[cfg(not(windows))]
    {
        return Err("Windows only".into());
    }
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        let appdata = std::path::PathBuf::from(appdata);
        let mut cleared = Vec::new();
        for &(process, name, folder) in DISCORD_CACHE_EDITIONS {
            let base = appdata.join(folder);
            if !base.is_dir() {
                continue;
            }
            let _ = run_capture("taskkill", &["/IM", process, "/F"]);
            for dir in ["Cache", "Code Cache", "GPUCache"] {
                let path = base.join(dir);
                if path.exists() {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
            cleared.push(name.to_string());
        }
        logs::append("app", &format!("Discord cache cleared: {cleared:?}"));
        Ok(cleared)
    }
}
