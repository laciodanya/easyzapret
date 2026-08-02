use serde::Serialize;
use tauri::State;

#[cfg(windows)]
use crate::util::run_capture;
use crate::zapret::strategy;
use crate::{logs, paths, AppState};

const IPSET_DUMMY: &str = "203.0.113.113/32";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZapretStatus {
    /// winws.exe process is alive (either our child or external)
    pub running: bool,
    /// Strategy we started manually in this session
    pub current_strategy: Option<String>,
    pub service_installed: bool,
    pub service_state: Option<String>,
    /// Strategy name stored in the registry by Install Service
    pub service_strategy: Option<String>,
    pub windivert_state: Option<String>,
    pub windivert_sys_present: bool,
}

/// Parses `sc query <name>` output and returns the STATE value (e.g. RUNNING).
pub fn query_service_state(name: &str) -> Option<String> {
    #[cfg(not(windows))]
    {
        let _ = name;
        return None;
    }
    #[cfg(windows)]
    {
        let (ok, out) = run_capture("sc", &["query", name]);
        if !ok && !out.to_uppercase().contains("STATE") {
            return None;
        }
        for line in out.lines() {
            let upper = line.to_uppercase();
            if upper.contains("STATE") {
                // "        STATE              : 4  RUNNING"
                let tail = line.split(':').nth(1)?.trim();
                let word = tail.split_whitespace().last()?.to_string();
                return Some(word);
            }
        }
        None
    }
}

fn registry_strategy() -> Option<String> {
    #[cfg(not(windows))]
    {
        return None;
    }
    #[cfg(windows)]
    {
        let (ok, out) = run_capture(
            "reg",
            &[
                "query",
                r"HKLM\System\CurrentControlSet\Services\zapret",
                "/v",
                "zapret-discord-youtube",
            ],
        );
        if !ok {
            return None;
        }
        for line in out.lines() {
            if line.contains("zapret-discord-youtube") {
                // "    zapret-discord-youtube    REG_SZ    general (ALT5)"
                if let Some(pos) = line.find("REG_SZ") {
                    let value = line[pos + "REG_SZ".len()..].trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
        None
    }
}

#[tauri::command]
pub fn zapret_status(state: State<'_, AppState>) -> ZapretStatus {
    let service_state = query_service_state("zapret");
    ZapretStatus {
        running: super::process::winws_running(),
        current_strategy: state.current_strategy.lock().unwrap().clone(),
        service_installed: service_state.is_some(),
        service_state,
        service_strategy: registry_strategy(),
        windivert_state: query_service_state("WinDivert"),
        windivert_sys_present: paths::zapret_bin_dir().join("WinDivert64.sys").exists(),
    }
}

/// Enables TCP timestamps like service.bat does before installing/starting.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn tcp_enable() {
    #[cfg(windows)]
    {
        let (_, out) = run_capture("netsh", &["interface", "tcp", "show", "global"]);
        let enabled = out
            .lines()
            .any(|l| l.to_lowercase().contains("timestamps") && l.to_lowercase().contains("enabled"));
        if !enabled {
            let _ = run_capture("netsh", &["interface", "tcp", "set", "global", "timestamps=enabled"]);
        }
    }
}

/// Install Service: parity with service.bat — parse strategy args, recreate
/// the "zapret" service with start=auto and remember the strategy in registry.
#[tauri::command]
pub fn install_zapret_service(strategy_file: String) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = strategy_file;
        return Err("Windows only".into());
    }
    #[cfg(windows)]
    {
        strategy::ensure_user_lists().map_err(|e| format!("failed to create user lists: {e}"))?;
        let args = strategy::winws_args(&strategy_file)?;
        tcp_enable();

        let exe = paths::zapret_bin_dir().join("winws.exe");
        let bin_path = format!("\"{}\" {}", exe.display(), strategy::service_args_string(&args));

        let _ = run_capture("net", &["stop", "zapret"]);
        let _ = run_capture("sc", &["delete", "zapret"]);

        let (ok, out) = run_capture(
            "sc",
            &[
                "create", "zapret",
                "binPath=", &bin_path,
                "DisplayName=", "zapret",
                "start=", "auto",
            ],
        );
        if !ok {
            return Err(format!("sc create failed: {}", out.trim()));
        }
        let _ = run_capture("sc", &["description", "zapret", "Zapret DPI bypass software"]);
        let (started, start_out) = run_capture("sc", &["start", "zapret"]);

        let strategy_name = strategy_file.trim_end_matches(".bat");
        let _ = run_capture(
            "reg",
            &[
                "add",
                r"HKLM\System\CurrentControlSet\Services\zapret",
                "/v", "zapret-discord-youtube",
                "/t", "REG_SZ",
                "/d", strategy_name,
                "/f",
            ],
        );

        logs::append("zapret", &format!("Service installed with strategy: {strategy_name}"));
        if !started {
            return Err(format!("service created but failed to start: {}", start_out.trim()));
        }
        Ok(())
    }
}

fn registry_strategy_bat() -> Option<String> {
    registry_strategy().map(|s| {
        if s.to_ascii_lowercase().ends_with(".bat") {
            s
        } else {
            format!("{s}.bat")
        }
    })
}

/// Kills winws and stops WinDivert. When `delete_services` is true, also
/// removes the zapret / WinDivert SCM entries so nothing can respawn winws
/// while zapret/bin files are being replaced (mirrors installer hooks.nsh).
pub fn ensure_zapret_released(state: &AppState, delete_services: bool) {
    #[cfg(windows)]
    {
        for _ in 0..3 {
            let _ = run_capture("net", &["stop", "zapret"]);
            std::thread::sleep(std::time::Duration::from_millis(350));
        }
    }

    super::process::stop(state);
    super::process::force_kill_winws();

    #[cfg(windows)]
    {
        if delete_services {
            let _ = run_capture("sc", &["delete", "zapret"]);
        }
        for name in ["WinDivert", "WinDivert14"] {
            let _ = run_capture("net", &["stop", name]);
            let _ = run_capture("sc", &["stop", name]);
            if delete_services {
                let _ = run_capture("sc", &["delete", name]);
            }
        }
        super::process::force_kill_winws();
        super::process::wait_for_winws_exit(std::time::Duration::from_secs(6));
    }

    #[cfg(not(windows))]
    {
        let _ = delete_services;
    }
}

/// Snapshot of how zapret was running before an update, so the previous
/// state can be restored once the new files are in place.
pub struct ZapretRunState {
    pub manual_strategy: Option<String>,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub service_installed: bool,
    /// Only consulted on Windows.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub service_was_running: bool,
    /// Strategy bat for reinstalling the service after an update.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub service_strategy: Option<String>,
}

/// Fully stops zapret so its files can be replaced during an update.
pub fn stop_for_update(state: &AppState) -> ZapretRunState {
    let manual_strategy = state.current_strategy.lock().unwrap().clone();

    #[cfg(windows)]
    let service_installed = query_service_state("zapret").is_some();
    #[cfg(not(windows))]
    let service_installed = false;

    #[cfg(windows)]
    let service_was_running = query_service_state("zapret").as_deref() == Some("RUNNING");
    #[cfg(not(windows))]
    let service_was_running = false;

    let service_strategy = registry_strategy_bat();

    ensure_zapret_released(state, true);
    logs::append("zapret", "Stopped zapret for update (processes and services removed)");
    ZapretRunState {
        manual_strategy,
        service_installed,
        service_was_running,
        service_strategy,
    }
}

/// Restores the zapret run state captured by `stop_for_update` after the
/// files have been replaced. Best-effort: failures are ignored.
pub fn restore_after_update(state: &AppState, prev: &ZapretRunState) {
    #[cfg(windows)]
    {
        if prev.service_installed {
            if let Some(ref bat) = prev.service_strategy {
                match install_zapret_service(bat.clone()) {
                    Ok(()) => {
                        if !prev.service_was_running {
                            let _ = run_capture("net", &["stop", "zapret"]);
                        }
                        logs::append("zapret", "Service reinstalled after update");
                    }
                    Err(e) => logs::append("zapret", &format!("Service reinstall after update failed: {e}")),
                }
                return;
            }
        }
    }
    if let Some(bat) = &prev.manual_strategy {
        if super::process::start_strategy(state, bat).is_ok() {
            logs::append("zapret", &format!("Restarted strategy after update: {bat}"));
        }
    }
}

/// Remove Services: stops/deletes zapret, kills winws, removes WinDivert(14).
#[tauri::command]
pub fn remove_zapret_services(state: State<'_, AppState>) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = state;
        return Err("Windows only".into());
    }
    #[cfg(windows)]
    {
        ensure_zapret_released(&state, true);
        logs::append("zapret", "Services removed (zapret, WinDivert, WinDivert14)");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSettings {
    pub game_filter_mode: String,
    /// none | loaded | any
    pub ipset_mode: String,
    pub auto_update_check: bool,
}

/// IPSet mode is encoded in lists/ipset-all.txt content (service.bat logic):
/// empty file -> "any", dummy 203.0.113.113/32 -> "none", otherwise "loaded".
pub fn ipset_mode() -> String {
    let file = paths::zapret_lists_dir().join("ipset-all.txt");
    let Ok(content) = std::fs::read_to_string(&file) else {
        return "none".into();
    };
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        "any".into()
    } else if lines.iter().any(|l| l.trim() == IPSET_DUMMY) {
        "none".into()
    } else {
        "loaded".into()
    }
}

#[tauri::command]
pub fn get_service_settings() -> ServiceSettings {
    ServiceSettings {
        game_filter_mode: strategy::game_filter().mode,
        ipset_mode: ipset_mode(),
        auto_update_check: paths::zapret_utils_dir().join("check_updates.enabled").exists(),
    }
}

#[tauri::command]
pub fn set_game_filter(mode: String) -> Result<(), String> {
    strategy::set_game_filter_mode(&mode)?;
    logs::append("zapret", &format!("Game filter -> {mode} (restart zapret to apply)"));
    Ok(())
}

#[tauri::command]
pub fn set_ipset_mode(mode: String) -> Result<(), String> {
    let lists = paths::zapret_lists_dir();
    std::fs::create_dir_all(&lists).map_err(|e| e.to_string())?;
    let file = lists.join("ipset-all.txt");
    let backup = lists.join("ipset-all.txt.backup");
    let current = ipset_mode();
    if current == mode {
        return Ok(());
    }

    match mode.as_str() {
        "none" => {
            if current == "loaded" && file.exists() {
                let _ = std::fs::remove_file(&backup);
                std::fs::rename(&file, &backup).map_err(|e| e.to_string())?;
            }
            std::fs::write(&file, format!("{IPSET_DUMMY}\r\n")).map_err(|e| e.to_string())?;
        }
        "any" => {
            if current == "loaded" && file.exists() {
                let _ = std::fs::remove_file(&backup);
                std::fs::rename(&file, &backup).map_err(|e| e.to_string())?;
            }
            std::fs::write(&file, "").map_err(|e| e.to_string())?;
        }
        "loaded" => {
            if backup.exists() {
                let _ = std::fs::remove_file(&file);
                std::fs::rename(&backup, &file).map_err(|e| e.to_string())?;
            } else {
                return Err("no_backup".into());
            }
        }
        _ => return Err(format!("unknown ipset mode: {mode}")),
    }
    logs::append("zapret", &format!("IPSet filter -> {mode} (restart zapret to apply)"));
    Ok(())
}

#[tauri::command]
pub fn set_auto_update_check(enabled: bool) -> Result<(), String> {
    let utils = paths::zapret_utils_dir();
    std::fs::create_dir_all(&utils).map_err(|e| e.to_string())?;
    let flag = utils.join("check_updates.enabled");
    if enabled {
        std::fs::write(&flag, "ENABLED\r\n").map_err(|e| e.to_string())?;
    } else if flag.exists() {
        std::fs::remove_file(&flag).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Update IPSet List: downloads .service/ipset-service.txt into ipset-all.txt.
#[tauri::command]
pub async fn update_ipset_list() -> Result<(), String> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/refs/heads/main/.service/ipset-service.txt",
        crate::updates::ZAPRET_REPO
    );
    let resp = reqwest::Client::builder()
        .user_agent("EasyZapret/0.1")
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let lists = paths::zapret_lists_dir();
    std::fs::create_dir_all(&lists).map_err(|e| e.to_string())?;
    std::fs::write(lists.join("ipset-all.txt"), body).map_err(|e| e.to_string())?;
    logs::append("zapret", "ipset-all.txt updated from repository");
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsCheck {
    pub up_to_date: bool,
    /// Downloaded reference file (kept for manual merge, like service.bat)
    pub temp_file: String,
    pub hosts_file: String,
}

/// Update Hosts File: downloads the reference hosts and checks whether its
/// first/last lines are present in the system hosts file. Like service.bat,
/// we do not patch hosts automatically — the user merges manually.
#[tauri::command]
pub async fn check_hosts_file() -> Result<HostsCheck, String> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/refs/heads/main/.service/hosts",
        crate::updates::ZAPRET_REPO
    );
    let resp = reqwest::Client::builder()
        .user_agent("EasyZapret/0.1")
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;

    let temp_file = paths::tmp_dir().join("zapret_hosts.txt");
    std::fs::create_dir_all(paths::tmp_dir()).map_err(|e| e.to_string())?;
    std::fs::write(&temp_file, &body).map_err(|e| e.to_string())?;

    let hosts_path = if cfg!(windows) {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        format!("{root}\\System32\\drivers\\etc\\hosts")
    } else {
        "/etc/hosts".to_string()
    };
    let hosts = std::fs::read_to_string(&hosts_path).unwrap_or_default();

    let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
    let first = lines.first().copied().unwrap_or_default();
    let last = lines.last().copied().unwrap_or_default();
    let up_to_date = !first.is_empty() && hosts.contains(first) && hosts.contains(last);

    Ok(HostsCheck {
        up_to_date,
        temp_file: temp_file.to_string_lossy().to_string(),
        hosts_file: hosts_path,
    })
}

const ACTIVE_DISCORD_FAKE: &str = "ACTIVE_DISCORD_UDP.bin";
const ACTIVE_GAME_FAKE: &str = "ACTIVE_GAME_UDP.bin";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveFakesStatus {
    /// Available `.bin` basenames in `bin/` (without extension), excluding ACTIVE_*.
    pub files: Vec<String>,
    /// Basename that currently matches ACTIVE_DISCORD_UDP.bin, if any.
    pub discord_current: Option<String>,
    /// Basename that currently matches ACTIVE_GAME_UDP.bin, if any.
    pub game_current: Option<String>,
}

fn list_fake_bin_names(bin: &std::path::Path) -> Result<Vec<String>, String> {
    if !bin.is_dir() {
        return Err("bin folder not found".into());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(bin).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("bin") {
            continue;
        }
        if stem.to_ascii_uppercase().starts_with("ACTIVE_") {
            continue;
        }
        names.push(stem.to_string());
    }
    names.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    Ok(names)
}

fn match_active_fake(bin: &std::path::Path, active_name: &str, candidates: &[String]) -> Option<String> {
    let active = bin.join(active_name);
    let Ok(active_bytes) = std::fs::read(&active) else {
        return None;
    };
    for name in candidates {
        let path = bin.join(format!("{name}.bin"));
        if let Ok(bytes) = std::fs::read(&path) {
            if bytes == active_bytes {
                return Some(name.clone());
            }
        }
    }
    None
}

/// Lists replaceable fake `.bin` files and which ACTIVE_* slots they currently match.
/// Mirrors FlowSeal service.bat menu item 7.
#[tauri::command]
pub fn get_active_fakes() -> Result<ActiveFakesStatus, String> {
    let bin = paths::zapret_bin_dir();
    let files = list_fake_bin_names(&bin)?;
    let discord_current = match_active_fake(&bin, ACTIVE_DISCORD_FAKE, &files);
    let game_current = match_active_fake(&bin, ACTIVE_GAME_FAKE, &files);
    Ok(ActiveFakesStatus {
        files,
        discord_current,
        game_current,
    })
}

/// Replaces `ACTIVE_DISCORD_UDP.bin` or `ACTIVE_GAME_UDP.bin` with a chosen fake file.
/// `kind` is `discord` or `game`; `file` is the basename without `.bin`.
#[tauri::command]
pub fn set_active_fake(kind: String, file: String) -> Result<ActiveFakesStatus, String> {
    let bin = paths::zapret_bin_dir();
    let files = list_fake_bin_names(&bin)?;
    let stem = file.trim().trim_end_matches(".bin").trim_end_matches(".BIN");
    if stem.is_empty() || !files.iter().any(|f| f.eq_ignore_ascii_case(stem)) {
        return Err("invalid fake file".into());
    }
    let source = bin.join(format!("{stem}.bin"));
    if !source.is_file() {
        return Err("fake file not found".into());
    }
    let active_name = match kind.as_str() {
        "discord" => ACTIVE_DISCORD_FAKE,
        "game" => ACTIVE_GAME_FAKE,
        _ => return Err(format!("unknown fake kind: {kind}")),
    };
    let dest = bin.join(active_name);
    let _ = std::fs::remove_file(&dest);
    std::fs::copy(&source, &dest).map_err(|e| format!("failed to replace active fake: {e}"))?;
    logs::append(
        "zapret",
        &format!("Active fake {active_name} <- {stem}.bin (restart zapret to apply)"),
    );
    get_active_fakes()
}
