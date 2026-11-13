use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

use crate::paths;

static SETTINGS_IO: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutostartSettings {
    /// Register EasyZapret in the Windows Run key.
    pub launch_at_login: bool,
    /// Start Zapret (winws) when the app opens.
    pub auto_start_zapret: bool,
    /// Connect WARP after Zapret is up (forces Zapret autostart).
    pub auto_start_warp: bool,
    /// Start tg-ws-proxy when the app opens.
    pub auto_start_tg: bool,
    /// Open the app minimized to tray.
    pub start_minimized: bool,
}

impl Default for AutostartSettings {
    fn default() -> Self {
        Self {
            launch_at_login: true,
            auto_start_zapret: false,
            auto_start_warp: false,
            auto_start_tg: false,
            start_minimized: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutopilotSettings {
    pub enabled: bool,
    /// How often to run health probes while the app is open (minutes).
    pub interval_minutes: u32,
    /// stability | speed | gaming — used when switch_mode is rotate_policy
    pub policy: String,
    /// rotate_policy | rotate_custom | best_by_tests
    pub switch_mode: String,
    /// Strategy .bat files allowed when switch_mode is rotate_custom or filters best_by_tests
    pub allowed_strategies: Vec<String>,
    /// Max strategies to benchmark in best_by_tests mode
    pub max_test_strategies: u32,
    pub auto_switch_strategy: bool,
    /// Switch when health score drops below this (0–100).
    pub min_health_percent: u32,
    pub probe_discord: bool,
    pub probe_youtube: bool,
    pub probe_cloudflare: bool,
    pub probe_google: bool,
    pub probe_telegram: bool,
    /// Also run the bulk-upload freeze check (catches throttling that a plain
    /// request misses). Costs a few seconds per check.
    pub deep_check: bool,
    pub auto_enable_warp: bool,
    pub notify_on_switch: bool,
    pub notify_on_degraded: bool,
    /// Cap automatic strategy rotations per hour.
    pub max_switches_per_hour: u32,
    pub only_when_zapret_running: bool,
}

impl Default for AutopilotSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 15,
            policy: "stability".into(),
            switch_mode: "rotate_policy".into(),
            allowed_strategies: Vec::new(),
            max_test_strategies: 6,
            auto_switch_strategy: true,
            min_health_percent: 60,
            probe_discord: true,
            probe_youtube: true,
            probe_cloudflare: true,
            probe_google: false,
            probe_telegram: false,
            deep_check: true,
            auto_enable_warp: false,
            notify_on_switch: true,
            notify_on_degraded: true,
            max_switches_per_hour: 3,
            only_when_zapret_running: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "ru" | "en" | null -> follow system locale
    pub language: Option<String>,
    /// "light" | "dark" | "system" (legacy "purple" maps to dark on load)
    pub theme: String,
    /// File name of the selected strategy bat, e.g. "general (ALT5).bat"
    pub selected_strategy: Option<String>,
    /// Installed release tags, used for update checks (compare to GitHub tag)
    pub zapret_version: Option<String>,
    pub tg_version: Option<String>,
    /// Check both components for updates on app start
    pub check_updates_on_start: bool,
    pub autopilot: AutopilotSettings,
    pub autostart: AutostartSettings,
    /// Last app version for which the user dismissed the "what's new" modal.
    pub last_seen_changelog_version: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            theme: "system".into(),
            selected_strategy: None,
            zapret_version: None,
            tg_version: None,
            check_updates_on_start: true,
            autopilot: AutopilotSettings::default(),
            autostart: AutostartSettings::default(),
            last_seen_changelog_version: None,
        }
    }
}

fn backup_file() -> std::path::PathBuf {
    paths::data_dir().join("config.json.bak")
}

fn parse_settings(path: &Path) -> Option<Settings> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn load_unlocked() -> Settings {
    let mut s = parse_settings(&paths::settings_file())
        .or_else(|| parse_settings(&backup_file()))
        .unwrap_or_default();
    normalize_theme(&mut s);
    s
}

pub fn load() -> Settings {
    let _guard = SETTINGS_IO
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    load_unlocked()
}

fn normalize_theme(s: &mut Settings) {
    if s.theme == "purple" {
        s.theme = "dark".into();
    }
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let ok = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::rename(source, destination).map_err(|e| e.to_string())
}

fn save_unlocked(settings: &Settings) -> Result<(), String> {
    paths::ensure_dirs().map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let destination = paths::settings_file();
    let temporary = paths::data_dir().join(format!(
        ".config.json.{}.{}.tmp",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));

    let result = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;

        if destination.is_file() {
            std::fs::copy(&destination, backup_file()).map_err(|e| e.to_string())?;
        }
        replace_file(&temporary, &destination)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Performs a read-modify-write transaction under one process-wide lock.
pub fn update(mutator: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
    let _guard = SETTINGS_IO
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut settings = load_unlocked();
    mutator(&mut settings);
    save_unlocked(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn get_settings() -> Settings {
    load()
}

/// Collects `path: old -> new` lines for every leaf that differs.
fn diff_json(
    prefix: &str,
    old: &serde_json::Value,
    new: &serde_json::Value,
    out: &mut Vec<String>,
) {
    use serde_json::Value;
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, nv) in b {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                diff_json(&path, a.get(key).unwrap_or(&Value::Null), nv, out);
            }
        }
        _ if old != new => out.push(format!("{prefix}: {old} -> {new}")),
        _ => {}
    }
}

fn validate_patch_shape(
    prefix: &str,
    current: &serde_json::Value,
    patch: &serde_json::Value,
) -> Result<(), String> {
    let serde_json::Value::Object(patch_object) = patch else {
        return Ok(());
    };
    let serde_json::Value::Object(current_object) = current else {
        return Err(format!("settings field '{prefix}' is not an object"));
    };
    for (key, value) in patch_object {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        let Some(current_value) = current_object.get(key) else {
            return Err(format!("unknown settings field: {path}"));
        };
        validate_patch_shape(&path, current_value, value)?;
    }
    Ok(())
}

fn merge_json(target: &mut serde_json::Value, patch: serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(target), serde_json::Value::Object(patch)) => {
            for (key, value) in patch {
                if let Some(current) = target.get_mut(&key) {
                    merge_json(current, value);
                }
            }
        }
        (target, value) => *target = value,
    }
}

/// Applies only the fields sent by the UI to the latest settings snapshot.
/// This prevents a stale frontend copy from overwriting changes made by the
/// autopilot or updater between rendering and clicking a toggle.
#[tauri::command]
pub fn patch_settings(patch: serde_json::Value) -> Result<Settings, String> {
    let _guard = SETTINGS_IO
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let current = load_unlocked();
    let before = serde_json::to_value(&current).map_err(|e| e.to_string())?;
    let patch_object = patch
        .as_object()
        .ok_or_else(|| "settings patch must be an object".to_string())?;
    for protected in ["zapretVersion", "tgVersion"] {
        if patch_object.contains_key(protected) {
            return Err(format!(
                "settings field '{protected}' is managed by the updater"
            ));
        }
    }
    validate_patch_shape("", &before, &patch)?;

    let mut merged = before.clone();
    merge_json(&mut merged, patch);
    let mut next: Settings = serde_json::from_value(merged).map_err(|e| e.to_string())?;
    normalize_theme(&mut next);
    if next.autostart.auto_start_warp {
        next.autostart.auto_start_zapret = true;
    }
    let after = serde_json::to_value(&next).map_err(|e| e.to_string())?;
    let mut changes = Vec::new();
    diff_json("", &before, &after, &mut changes);
    save_unlocked(&next)?;
    if !changes.is_empty() {
        crate::logs::append("app", &format!("settings changed: {}", changes.join("; ")));
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_patch_preserves_unmentioned_nested_fields() {
        let mut current = json!({
            "selectedStrategy": "general.bat",
            "autopilot": { "enabled": false, "maxSwitchesPerHour": 3 }
        });
        let patch = json!({ "autopilot": { "enabled": true } });
        validate_patch_shape("", &current, &patch).unwrap();
        merge_json(&mut current, patch);
        assert_eq!(current["selectedStrategy"], "general.bat");
        assert_eq!(current["autopilot"]["enabled"], true);
        assert_eq!(current["autopilot"]["maxSwitchesPerHour"], 3);
    }

    #[test]
    fn settings_patch_rejects_unknown_fields() {
        let current = json!({ "autopilot": { "enabled": false } });
        let patch = json!({ "autopilot": { "typo": true } });
        assert_eq!(
            validate_patch_shape("", &current, &patch).unwrap_err(),
            "unknown settings field: autopilot.typo"
        );
    }
}
