use std::io::Write;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{logs, paths, settings};

pub const ZAPRET_REPO: &str = "Flowseal/zapret-discord-youtube";
pub const TG_REPO: &str = "Flowseal/tg-ws-proxy";
pub const APP_REPO: &str = "laciodanya/easyzapret";
pub const XRAY_REPO: &str = "XTLS/Xray-core";

/// User lists and flags that must survive a zapret update.
const ZAPRET_USER_FILES: &[&str] = &[
    "lists/list-general-user.txt",
    "lists/list-exclude-user.txt",
    "lists/ipset-exclude-user.txt",
    "lists/ipset-all.txt",
    "lists/ipset-all.txt.backup",
    "utils/game_filter.enabled",
    "utils/check_updates.enabled",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub component: String,
    /// downloading | extracting | done
    pub phase: String,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub component: String,
    pub installed: Option<String>,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStatus {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentsState {
    pub zapret_installed: bool,
    pub zapret_version: Option<String>,
    pub tg_installed: bool,
    pub tg_version: Option<String>,
    pub vpn_core_installed: bool,
    pub vpn_core_version: Option<String>,
    pub data_dir: String,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("EasyZapret/0.6.2 (https://github.com/laciodanya/easyzapret)")
        .build()
        .map_err(|e| e.to_string())
}

pub async fn fetch_latest_release(repo: &str) -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = http_client()?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API error: HTTP {}", resp.status()));
    }
    resp.json::<ReleaseInfo>().await.map_err(|e| e.to_string())
}

/// Tags are compared as normalized strings ("v1.7.2" == "1.7.2"); any
/// difference from the installed tag is treated as an available update,
/// mirroring the version check in Flowseal's service.bat.
fn normalize_tag(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).to_string()
}

/// Reads the actually installed zapret version from `service.bat`
/// (`set "LOCAL_VERSION=1.9.9a"`). This survives manual file replacement,
/// unlike the version recorded in our config.json, which is used as a
/// fallback only.
pub fn installed_zapret_version() -> Option<String> {
    let bat = paths::zapret_dir().join("service.bat");
    if let Ok(bytes) = std::fs::read(&bat) {
        if let Some(version) = parse_local_version(&String::from_utf8_lossy(&bytes)) {
            return Some(version);
        }
    }
    settings::load().zapret_version
}

fn parse_local_version(text: &str) -> Option<String> {
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(pos) = lower.find("local_version=") {
            let tail = &line[pos + "local_version=".len()..];
            let version: String = tail
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '"' && *c != '&')
                .collect();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }
    None
}

pub fn is_update_available(installed: &Option<String>, latest: &str) -> bool {
    match installed {
        Some(cur) => normalize_tag(cur) != normalize_tag(latest),
        None => false,
    }
}

async fn download_file(
    app: &AppHandle,
    component: &str,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    let resp = http_client()?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed().as_millis() > 100 {
            last_emit = std::time::Instant::now();
            let _ = app.emit(
                "install-progress",
                InstallProgress {
                    component: component.to_string(),
                    phase: "downloading".into(),
                    downloaded,
                    total,
                },
            );
        }
    }
    Ok(())
}

/// Extracts a zip archive. If every entry lives under a single top-level
/// directory, that directory is stripped so files land directly in `dest`.
fn extract_zip(archive_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut common_root: Option<String> = None;
    let mut has_root = true;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().replace('\\', "/");
        let root = name.split('/').next().unwrap_or("").to_string();
        if root.is_empty() || !name.contains('/') && !entry.is_dir() {
            has_root = false;
            break;
        }
        match &common_root {
            None => common_root = Some(root),
            Some(r) if *r != root => {
                has_root = false;
                break;
            }
            _ => {}
        }
    }
    let strip = if has_root { common_root } else { None };

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let raw = entry.name().replace('\\', "/");
        let rel = match &strip {
            Some(root) => raw
                .strip_prefix(&format!("{root}/"))
                .unwrap_or_default()
                .to_string(),
            None => raw,
        };
        if rel.is_empty() {
            continue;
        }
        // Guard against zip-slip
        if rel.contains("..") {
            continue;
        }
        let out_path = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn backup_user_files(backup_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut saved = Vec::new();
    for rel in ZAPRET_USER_FILES {
        let src = paths::zapret_dir().join(rel);
        if src.exists() {
            let dst = backup_dir.join(rel.replace('/', "_"));
            if std::fs::create_dir_all(backup_dir).is_ok() && std::fs::copy(&src, &dst).is_ok() {
                saved.push((rel.to_string(), dst));
            }
        }
    }
    saved
}

fn restore_user_files(saved: &[(String, PathBuf)]) {
    for (rel, backup) in saved {
        let dst = paths::zapret_dir().join(rel);
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::copy(backup, dst);
    }
}

/// Best-effort removal of the NTFS Mark-of-the-Web from extracted files
/// ("Unblock" in file properties), so SmartScreen does not block winws.exe.
fn unblock_files(dir: &Path) {
    #[cfg(windows)]
    {
        let script = format!(
            "Get-ChildItem -Recurse -LiteralPath '{}' | Unblock-File",
            dir.display()
        );
        let _ = crate::util::hidden_command("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
    }
}

/// Moves the existing zapret tree out of the way so a fresh one can be
/// extracted. Renaming works on Windows even when a DLL inside bin/ is still
/// held open (delete often fails in that case).
async fn clear_zapret_dir_for_update(app: &AppHandle) -> Result<(), String> {
    let target = paths::zapret_dir();
    if !target.exists() {
        return Ok(());
    }

    {
        let state = app.state::<crate::AppState>();
        crate::zapret::service::ensure_zapret_released(&state, true);
    }
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;

    let stamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let staged = paths::tmp_dir().join(format!("zapret.old.{stamp}"));
    let _ = std::fs::create_dir_all(paths::tmp_dir());

    let mut last_err = String::new();
    for attempt in 0..8u32 {
        if attempt > 0 {
            let state = app.state::<crate::AppState>();
            crate::zapret::service::ensure_zapret_released(&state, true);
            tokio::time::sleep(std::time::Duration::from_millis(400 + u64::from(attempt) * 350)).await;
        }

        match std::fs::rename(&target, &staged) {
            Ok(()) => {
                logs::append("app", &format!("Relocated old zapret dir to {}", staged.display()));
                schedule_staged_cleanup(staged);
                return Ok(());
            }
            Err(e) => {
                last_err = e.to_string();
                if crate::util::force_remove_dir(&target).is_ok() {
                    logs::append("app", "Removed old zapret dir via force delete");
                    return Ok(());
                }
            }
        }
    }

    Err(format!(
        "cannot replace zapret directory (winws or WinDivert still holds files in bin/ — close other zapret tools and retry): {last_err}"
    ))
}

/// Tries to delete a relocated zapret tree in the background once handles drop.
fn schedule_staged_cleanup(path: std::path::PathBuf) {
    std::thread::spawn(move || {
        for i in 0..24 {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            crate::zapret::process::force_kill_winws();
            if crate::util::force_remove_dir(&path).is_ok() {
                logs::append("app", &format!("Removed staged zapret dir {}", path.display()));
                return;
            }
        }
        logs::append(
            "app",
            &format!(
                "Staged zapret dir left at {} (locked file; safe to delete after reboot)",
                path.display()
            ),
        );
    });
}

async fn install_zapret(app: &AppHandle) -> Result<String, String> {
    let release = fetch_latest_release(ZAPRET_REPO).await?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".zip"))
        .ok_or("no .zip asset in the latest zapret release")?;

    logs::append("app", &format!("Downloading zapret {} ...", release.tag_name));

    // Fully stop zapret (manual winws, the service and the WinDivert driver)
    // so none of its files stay locked while we replace them. Remember the
    // previous run state to restore it afterwards.
    let prev_state = {
        let state = app.state::<crate::AppState>();
        crate::zapret::service::stop_for_update(&state)
    };

    let archive = paths::tmp_dir().join("zapret.zip");
    download_file(app, "zapret", &asset.browser_download_url, &archive).await?;

    let _ = app.emit(
        "install-progress",
        InstallProgress {
            component: "zapret".into(),
            phase: "extracting".into(),
            downloaded: 0,
            total: 0,
        },
    );

    // Keep user lists/flags across updates (read before the dir is relocated).
    let backup_dir = paths::tmp_dir().join("zapret-user-backup");
    let saved = backup_user_files(&backup_dir);

    if let Err(e) = clear_zapret_dir_for_update(app).await {
        let state = app.state::<crate::AppState>();
        crate::zapret::service::restore_after_update(&state, &prev_state);
        return Err(e);
    }

    let target = paths::zapret_dir();
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    extract_zip(&archive, &target)?;
    restore_user_files(&saved);
    unblock_files(&target);

    // Create the *-user.txt lists the release does not ship (service.bat does
    // this on launch); winws fails to start without them.
    let _ = crate::zapret::strategy::ensure_user_lists();

    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_dir_all(&backup_dir);

    let mut s = settings::load();
    s.zapret_version = Some(release.tag_name.clone());
    settings::save(&s)?;

    // Restore whatever was running before the update (service or manual strategy).
    {
        let state = app.state::<crate::AppState>();
        crate::zapret::service::restore_after_update(&state, &prev_state);
    }

    logs::append("app", &format!("zapret {} installed", release.tag_name));
    Ok(release.tag_name)
}

async fn install_tg(app: &AppHandle) -> Result<String, String> {
    let release = fetch_latest_release(TG_REPO).await?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == "TgWsProxy_windows.exe")
        .ok_or("no TgWsProxy_windows.exe asset in the latest release")?;

    logs::append("app", &format!("Downloading tg-ws-proxy {} ...", release.tag_name));

    // The binary cannot be replaced while it is running.
    crate::tg_proxy::kill_all();

    let dest = paths::tg_exe();
    download_file(app, "tgproxy", &asset.browser_download_url, &dest).await?;
    unblock_files(&paths::tg_dir());

    let mut s = settings::load();
    s.tg_version = Some(release.tag_name.clone());
    settings::save(&s)?;

    logs::append("app", &format!("tg-ws-proxy {} installed", release.tag_name));
    Ok(release.tag_name)
}

async fn install_xray(app: &AppHandle) -> Result<String, String> {
    let release = fetch_latest_release(XRAY_REPO).await?;
    let asset = release
        .assets
        .iter()
        .find(|a| {
            let n = a.name.to_ascii_lowercase();
            n.contains("windows") && n.contains("64") && n.ends_with(".zip") && !n.contains("arm")
        })
        .ok_or("no Xray-windows-64.zip asset in the latest release")?;

    logs::append("app", &format!("Downloading Xray-core {} ...", release.tag_name));
    crate::vpn::disconnect_quiet();

    let archive = paths::tmp_dir().join("xray-core.zip");
    download_file(app, "vpncore", &asset.browser_download_url, &archive).await?;

    let target = paths::vpn_core_dir();
    let _ = crate::util::force_remove_dir(&target);
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    extract_zip(&archive, &target)?;
    unblock_files(&target);
    let _ = std::fs::remove_file(&archive);

    if !paths::vpn_core_exe().exists() {
        return Err("xray.exe missing after extract".into());
    }

    let mut s = settings::load();
    s.vpn_core_version = Some(release.tag_name.clone());
    settings::save(&s)?;

    logs::append("app", &format!("Xray-core {} installed", release.tag_name));
    Ok(release.tag_name)
}

#[tauri::command]
pub async fn install_component(app: AppHandle, component: String) -> Result<String, String> {
    paths::ensure_dirs().map_err(|e| e.to_string())?;
    let result = match component.as_str() {
        "zapret" => install_zapret(&app).await,
        "tgproxy" => install_tg(&app).await,
        "vpncore" => install_xray(&app).await,
        _ => Err(format!("unknown component: {component}")),
    };
    let _ = app.emit(
        "install-progress",
        InstallProgress {
            component: component.clone(),
            phase: if result.is_ok() { "done".into() } else { "error".into() },
            downloaded: 0,
            total: 0,
        },
    );
    if let Err(e) = &result {
        logs::append("app", &format!("install {component} failed: {e}"));
    }
    result
}

#[tauri::command]
pub fn get_components_state() -> ComponentsState {
    let s = settings::load();
    ComponentsState {
        zapret_installed: paths::zapret_bin_dir().join("winws.exe").exists(),
        zapret_version: installed_zapret_version(),
        tg_installed: paths::tg_exe().exists(),
        tg_version: s.tg_version.clone(),
        vpn_core_installed: paths::vpn_core_exe().exists(),
        vpn_core_version: s.vpn_core_version,
        data_dir: paths::data_dir().to_string_lossy().to_string(),
    }
}

#[tauri::command]
pub async fn check_updates() -> Vec<UpdateStatus> {
    let s = settings::load();
    let mut out = Vec::new();
    for (component, repo, installed) in [
        ("zapret", ZAPRET_REPO, installed_zapret_version()),
        ("tgproxy", TG_REPO, s.tg_version.clone()),
        ("vpncore", XRAY_REPO, s.vpn_core_version.clone()),
    ] {
        match fetch_latest_release(repo).await {
            Ok(rel) => out.push(UpdateStatus {
                component: component.into(),
                installed: installed.clone(),
                latest: Some(rel.tag_name.clone()),
                update_available: is_update_available(&installed, &rel.tag_name),
                release_url: Some(rel.html_url),
                error: None,
            }),
            Err(e) => out.push(UpdateStatus {
                component: component.into(),
                installed: installed.clone(),
                latest: None,
                update_available: false,
                release_url: None,
                error: Some(e),
            }),
        }
    }
    out
}

/// Checks the EasyZapret GitHub repo for a newer app release (UI notification
/// + Tauri updater install path).
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> AppUpdateStatus {
    let current = app.package_info().version.to_string();
    match fetch_latest_release(APP_REPO).await {
        Ok(rel) => AppUpdateStatus {
            update_available: is_update_available(&Some(current.clone()), &rel.tag_name),
            current,
            latest: Some(rel.tag_name.clone()),
            release_url: Some(rel.html_url),
            error: None,
        },
        Err(e) => AppUpdateStatus {
            current,
            latest: None,
            update_available: false,
            release_url: None,
            error: Some(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_version_from_service_bat() {
        let bat = "@echo off\r\nchcp 65001 > nul\r\nset \"LOCAL_VERSION=1.9.9a\"\r\necho hi\r\n";
        assert_eq!(parse_local_version(bat), Some("1.9.9a".to_string()));
    }

    #[test]
    fn parses_local_version_without_quotes() {
        assert_eq!(
            parse_local_version("set LOCAL_VERSION=1.8.1\r\n"),
            Some("1.8.1".to_string())
        );
        assert_eq!(parse_local_version("echo nothing here"), None);
    }

    #[test]
    fn update_available_compares_normalized_tags() {
        assert!(is_update_available(&Some("1.8.1".into()), "v1.9.9a"));
        assert!(!is_update_available(&Some("v1.9.9a".into()), "1.9.9a"));
        assert!(!is_update_available(&None, "1.9.9a"));
    }
}
