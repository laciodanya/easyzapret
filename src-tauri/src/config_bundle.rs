//! Shareable EasyZapret config: app settings, user lists and Service tab state.
//!
//! The JSON can be saved, sent to another PC and imported there. Component
//! install versions and machine-local autopilot network memory are not included.

use serde::{Deserialize, Serialize};

use crate::paths;
use crate::settings::{self, Settings};
use crate::util;
use crate::zapret::{self, strategy};

const FORMAT: &str = "easyzapret-config";
const FORMAT_VERSION: u32 = 2;

const LIST_FILES: &[&str] = &[
    "list-general-user.txt",
    "list-exclude-user.txt",
    "ipset-exclude-user.txt",
    "ipset-all.txt",
    "ipset-all.txt.backup",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceBundle {
    game_filter_mode: String,
    /// none | loaded | any — also encoded in list files; kept for clarity.
    ipset_mode: String,
    auto_update_check: bool,
    /// Basename without `.bin`, if an ACTIVE_* slot matches a known fake.
    #[serde(default)]
    active_fake_discord: Option<String>,
    #[serde(default)]
    active_fake_game: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigBundle {
    format: String,
    format_version: u32,
    #[serde(default)]
    exported_at: Option<String>,
    #[serde(default)]
    exported_by_version: Option<String>,
    settings: serde_json::Value,
    #[serde(default)]
    service: Option<ServiceBundle>,
    /// Map of list file name → full text content.
    #[serde(default)]
    lists: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigImportResult {
    pub settings: Settings,
    pub lists_written: usize,
    pub service_applied: bool,
    pub warnings: Vec<String>,
}

fn read_list_file(name: &str) -> Option<String> {
    let path = paths::zapret_lists_dir().join(name);
    std::fs::read_to_string(path).ok()
}

fn write_list_file(name: &str, content: &str) -> Result<(), String> {
    if !LIST_FILES.contains(&name) {
        return Err(format!("list file is not allowed in config: {name}"));
    }
    let dir = paths::zapret_lists_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    util::atomic_write(&dir.join(name), content.as_bytes())
}

fn collect_service() -> ServiceBundle {
    let service = zapret::service::get_service_settings();
    let fakes = zapret::service::get_active_fakes().ok();
    ServiceBundle {
        game_filter_mode: service.game_filter_mode,
        ipset_mode: service.ipset_mode,
        auto_update_check: service.auto_update_check,
        active_fake_discord: fakes.as_ref().and_then(|f| f.discord_current.clone()),
        active_fake_game: fakes.as_ref().and_then(|f| f.game_current.clone()),
    }
}

fn settings_for_export(settings: &Settings) -> Result<serde_json::Value, String> {
    let mut value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    if let Some(object) = value.as_object_mut() {
        object.remove("zapretVersion");
        object.remove("tgVersion");
        // Changelog dismissal is personal to this install.
        object.remove("lastSeenChangelogVersion");
    }
    Ok(value)
}

fn merge_settings_json(
    current: &Settings,
    incoming: serde_json::Value,
) -> Result<Settings, String> {
    let mut patch = match incoming {
        serde_json::Value::Object(map) => map,
        _ => return Err("config settings must be an object".into()),
    };
    for protected in [
        "zapretVersion",
        "tgVersion",
        "lastSeenChangelogVersion",
        "easyzapretExportVersion",
    ] {
        patch.remove(protected);
    }

    let mut merged = serde_json::to_value(current).map_err(|e| e.to_string())?;
    if let serde_json::Value::Object(target) = &mut merged {
        for (key, value) in patch {
            let nested = (key == "autopilot" || key == "autostart")
                && matches!(value, serde_json::Value::Object(_))
                && matches!(target.get(&key), Some(serde_json::Value::Object(_)));
            if nested {
                if let (Some(serde_json::Value::Object(dst)), serde_json::Value::Object(src)) =
                    (target.get_mut(&key), value)
                {
                    for (nested_key, nested_value) in src {
                        dst.insert(nested_key, nested_value);
                    }
                }
            } else {
                target.insert(key, value);
            }
        }
    }

    let mut next: Settings =
        serde_json::from_value(merged).map_err(|e| format!("invalid settings in config: {e}"))?;
    if next.theme == "purple" {
        next.theme = "dark".into();
    }
    if next.autostart.auto_start_warp {
        next.autostart.auto_start_zapret = true;
    }
    next.zapret_version = current.zapret_version.clone();
    next.tg_version = current.tg_version.clone();
    next.last_seen_changelog_version = current.last_seen_changelog_version.clone();
    Ok(next)
}

fn apply_settings_value(incoming: serde_json::Value) -> Result<Settings, String> {
    let current = settings::load();
    let next = merge_settings_json(&current, incoming)?;
    settings::update(|settings| *settings = next.clone())
}

fn apply_service(service: &ServiceBundle, warnings: &mut Vec<String>) -> bool {
    let mut ok = true;
    if let Err(e) = strategy::set_game_filter_mode(&service.game_filter_mode) {
        warnings.push(format!("game filter: {e}"));
        ok = false;
    }
    if let Err(e) = zapret::service::set_auto_update_check(service.auto_update_check) {
        warnings.push(format!("auto update check: {e}"));
        ok = false;
    }
    // List files already encode ipset mode; still try the explicit setter when
    // files were not provided, so a service-only fragment still works.
    if let Err(e) = zapret::service::set_ipset_mode(service.ipset_mode.clone()) {
        // "no_backup" is expected when lists already wrote the loaded file.
        if e != "no_backup" {
            warnings.push(format!("ipset mode: {e}"));
        }
    }
    if let Some(name) = service.active_fake_discord.as_deref() {
        if let Err(e) = zapret::service::set_active_fake("discord".into(), name.into()) {
            warnings.push(format!("discord fake '{name}': {e}"));
        }
    }
    if let Some(name) = service.active_fake_game.as_deref() {
        if let Err(e) = zapret::service::set_active_fake("game".into(), name.into()) {
            warnings.push(format!("game fake '{name}': {e}"));
        }
    }
    ok
}

/// Builds a portable JSON config that can be shared between installs.
#[tauri::command]
pub fn export_settings(app: tauri::AppHandle) -> Result<String, String> {
    let settings = settings::load();
    let mut lists = std::collections::BTreeMap::new();
    for name in LIST_FILES {
        if let Some(content) = read_list_file(name) {
            lists.insert((*name).to_string(), content);
        }
    }

    let bundle = ConfigBundle {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        exported_at: Some(chrono::Utc::now().to_rfc3339()),
        exported_by_version: Some(app.package_info().version.to_string()),
        settings: settings_for_export(&settings)?,
        service: Some(collect_service()),
        lists,
    };
    serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())
}

fn parse_bundle(json: &str) -> Result<ConfigBundle, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid config JSON: {e}"))?;

    // Legacy v1: flat settings object with easyzapretExportVersion.
    if value
        .get("format")
        .and_then(|v| v.as_str())
        .is_none_or(|f| f != FORMAT)
    {
        if value.get("easyzapretExportVersion").is_some() || value.get("autopilot").is_some() {
            return Ok(ConfigBundle {
                format: FORMAT.into(),
                format_version: 1,
                exported_at: None,
                exported_by_version: None,
                settings: value,
                service: None,
                lists: std::collections::BTreeMap::new(),
            });
        }
        return Err("not an EasyZapret config file".into());
    }

    serde_json::from_value(value).map_err(|e| format!("invalid EasyZapret config: {e}"))
}

/// Imports a previously exported (or shared) EasyZapret config.
#[tauri::command]
pub fn import_settings(json: String) -> Result<ConfigImportResult, String> {
    let bundle = parse_bundle(&json)?;
    if bundle.format_version > FORMAT_VERSION {
        return Err(format!(
            "config format version {} is newer than this app supports ({FORMAT_VERSION})",
            bundle.format_version
        ));
    }

    let mut warnings = Vec::new();
    let settings = apply_settings_value(bundle.settings)?;

    let mut lists_written = 0usize;
    for (name, content) in &bundle.lists {
        match write_list_file(name, content) {
            Ok(()) => lists_written += 1,
            Err(e) => warnings.push(e),
        }
    }

    let service_applied = if let Some(service) = bundle.service.as_ref() {
        // When list files were written, ipset mode is already correct; still
        // apply game filter / auto-update / fakes and best-effort ipset mode.
        apply_service(service, &mut warnings)
    } else {
        false
    };

    crate::logs::append(
        "app",
        &format!(
            "config imported (lists={lists_written}, service={}, warnings={})",
            service_applied,
            warnings.len()
        ),
    );

    Ok(ConfigImportResult {
        settings,
        lists_written,
        service_applied,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_legacy_flat_export() {
        let raw = json!({
            "easyzapretExportVersion": 1,
            "theme": "dark",
            "autopilot": { "enabled": true }
        })
        .to_string();
        let bundle = parse_bundle(&raw).unwrap();
        assert_eq!(bundle.format_version, 1);
        assert!(bundle.service.is_none());
        assert_eq!(bundle.settings["theme"], "dark");
    }

    #[test]
    fn parses_v2_bundle() {
        let raw = json!({
            "format": "easyzapret-config",
            "formatVersion": 2,
            "settings": { "theme": "light" },
            "service": {
                "gameFilterMode": "all",
                "ipsetMode": "loaded",
                "autoUpdateCheck": true
            },
            "lists": {
                "list-general-user.txt": "example.com\n"
            }
        })
        .to_string();
        let bundle = parse_bundle(&raw).unwrap();
        assert_eq!(bundle.format_version, 2);
        assert_eq!(bundle.lists["list-general-user.txt"], "example.com\n");
        assert_eq!(bundle.service.unwrap().game_filter_mode, "all");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = parse_bundle(r#"{"format":"other","formatVersion":1}"#).unwrap_err();
        assert!(err.contains("not an EasyZapret config") || err.contains("invalid"));
    }
}
