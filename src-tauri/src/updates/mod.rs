use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::{logs, paths, settings};

static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub const ZAPRET_REPO: &str = "Flowseal/zapret-discord-youtube";
pub const TG_REPO: &str = "Flowseal/tg-ws-proxy";
pub const APP_REPO: &str = "laciodanya/easyzapret";

/// Component executables run elevated, so their trust root must be shipped in
/// the signed EasyZapret binary. GitHub's asset `digest` is useful metadata,
/// but it is controlled by the same upstream repository as the asset and is
/// not an independent authenticity check.
#[derive(Debug, Clone, Copy)]
struct TrustedComponentAsset {
    component: &'static str,
    repo: &'static str,
    tag: &'static str,
    name: &'static str,
    size: u64,
    sha256: &'static str,
}

const TRUSTED_COMPONENT_ASSETS: &[TrustedComponentAsset] = &[
    TrustedComponentAsset {
        component: "zapret",
        repo: ZAPRET_REPO,
        tag: "1.10.4",
        name: "zapret-discord-youtube-1.10.4.zip",
        size: 1_518_649,
        sha256: "291af673bd0238506f9548e0e94ca0abf244000f4c9a06960544fb01c5f97b34",
    },
    TrustedComponentAsset {
        component: "tgproxy",
        repo: TG_REPO,
        tag: "v1.11.4",
        name: "TgWsProxy_windows.exe",
        size: 21_805_650,
        sha256: "454205cbcf76f333934108d893b58a528373bce8096624e27f0b6dc16eea660f",
    },
];

fn trusted_component(component: &str) -> Result<&'static TrustedComponentAsset, String> {
    TRUSTED_COMPONENT_ASSETS
        .iter()
        .find(|asset| asset.component == component)
        .ok_or_else(|| format!("no trusted asset is pinned for {component}"))
}

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
    pub digest: Option<String>,
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
    pub data_dir: String,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!(
            "EasyZapret/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/laciodanya/easyzapret)"
        ))
        .https_only(true)
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.stop();
            }
            if trusted_github_host(attempt.url()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|e| e.to_string())
}

fn trusted_github_host(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some(
                "api.github.com"
                    | "github.com"
                    | "objects.githubusercontent.com"
                    | "release-assets.githubusercontent.com"
            )
        )
}

fn validate_asset_url(trusted: &TrustedComponentAsset, raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|e| format!("invalid asset URL: {e}"))?;
    let expected_path = format!(
        "/{}/releases/download/{}/{}",
        trusted.repo, trusted.tag, trusted.name
    );
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.path() != expected_path
    {
        return Err(format!("refusing untrusted release asset URL: {url}"));
    }
    Ok(url)
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

async fn fetch_release_by_tag(repo: &str, tag: &str) -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/tags/{tag}");
    let resp = http_client()?
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API returned HTTP {}", resp.status()));
    }
    let release = resp
        .json::<ReleaseInfo>()
        .await
        .map_err(|e| e.to_string())?;
    if normalize_tag(&release.tag_name) != normalize_tag(tag) {
        return Err(format!(
            "GitHub returned release '{}' for pinned tag '{tag}'",
            release.tag_name
        ));
    }
    Ok(release)
}

/// Tags are compared as normalized strings ("v1.7.2" == "1.7.2"); any
/// difference from the installed tag is treated as an available update,
/// mirroring the version check in Flowseal's service.bat.
fn normalize_tag(tag: &str) -> String {
    tag.trim()
        .trim_start_matches(['v', 'V'])
        .to_ascii_lowercase()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VersionPart {
    Number(u64),
    Text(String),
}

fn version_parts(tag: &str) -> Vec<VersionPart> {
    let normalized = normalize_tag(tag);
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut numeric: Option<bool> = None;
    let flush = |parts: &mut Vec<VersionPart>, current: &mut String, numeric: Option<bool>| {
        if current.is_empty() {
            return;
        }
        if numeric == Some(true) {
            parts.push(VersionPart::Number(current.parse().unwrap_or(0)));
        } else {
            parts.push(VersionPart::Text(std::mem::take(current)));
            return;
        }
        current.clear();
    };

    for ch in normalized.chars() {
        if !ch.is_ascii_alphanumeric() {
            flush(&mut parts, &mut current, numeric);
            numeric = None;
            continue;
        }
        let is_numeric = ch.is_ascii_digit();
        if numeric.is_some() && numeric != Some(is_numeric) {
            flush(&mut parts, &mut current, numeric);
        }
        numeric = Some(is_numeric);
        current.push(ch);
    }
    flush(&mut parts, &mut current, numeric);
    parts
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let left = version_parts(left);
    let right = version_parts(right);
    let max = left.len().max(right.len());
    for index in 0..max {
        let ordering = match (left.get(index), right.get(index)) {
            (Some(VersionPart::Number(a)), Some(VersionPart::Number(b))) => a.cmp(b),
            (Some(VersionPart::Text(a)), Some(VersionPart::Text(b))) => a.cmp(b),
            (Some(VersionPart::Number(_)), Some(VersionPart::Text(_))) => Ordering::Greater,
            (Some(VersionPart::Text(_)), Some(VersionPart::Number(_))) => Ordering::Less,
            (Some(VersionPart::Number(0)), None) | (None, Some(VersionPart::Number(0))) => {
                Ordering::Equal
            }
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
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
        Some(cur) => compare_versions(cur, latest).is_lt(),
        None => false,
    }
}

async fn download_asset(
    app: &AppHandle,
    trusted: &TrustedComponentAsset,
    asset: &ReleaseAsset,
    dest: &Path,
) -> Result<(), String> {
    if asset.name != trusted.name || asset.size != trusted.size {
        return Err(format!(
            "GitHub metadata for {} does not match the trusted catalog",
            trusted.name
        ));
    }
    if let Some(api_digest) = asset.digest.as_deref() {
        let api_digest = api_digest.strip_prefix("sha256:").unwrap_or(api_digest);
        if !api_digest.eq_ignore_ascii_case(trusted.sha256) {
            return Err(format!(
                "GitHub metadata digest for {} differs from the digest pinned in EasyZapret",
                trusted.name
            ));
        }
    }
    let url = validate_asset_url(trusted, &asset.browser_download_url)?;
    let expected_digest = trusted.sha256;

    let resp = http_client()?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }
    if !trusted_github_host(resp.url()) {
        return Err(format!(
            "download redirected to an untrusted host: {}",
            resp.url()
        ));
    }
    let total = resp.content_length().unwrap_or(0);
    if total != 0 && total != trusted.size {
        return Err(format!(
            "download size advertised as {total} bytes, trusted catalog says {}",
            trusted.size
        ));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let result = async {
        let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut hasher = Sha256::new();
        let mut last_emit = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            hasher.update(&chunk);
            downloaded += chunk.len() as u64;
            if last_emit.elapsed().as_millis() > 100 {
                last_emit = std::time::Instant::now();
                let _ = app.emit(
                    "install-progress",
                    InstallProgress {
                        component: trusted.component.to_string(),
                        phase: "downloading".into(),
                        downloaded,
                        total: trusted.size,
                    },
                );
            }
        }
        file.sync_all().map_err(|e| e.to_string())?;
        if downloaded != trusted.size {
            return Err(format!(
                "incomplete download: received {downloaded} of {} bytes",
                trusted.size
            ));
        }
        let actual_digest = format!("{:x}", hasher.finalize());
        if actual_digest != expected_digest {
            return Err(format!(
                "SHA-256 mismatch for {}: expected {expected_digest}, got {actual_digest}",
                asset.name
            ));
        }
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

/// Extracts a zip archive. If every entry lives under a single top-level
/// directory, that directory is stripped so files land directly in `dest`.
fn extract_zip(archive_path: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    const MAX_UNCOMPRESSED_BYTES: u64 = 1_073_741_824;
    let mut uncompressed = 0u64;

    let mut common_root: Option<std::ffi::OsString> = None;
    let mut has_root = true;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        validate_zip_entry(&entry)?;
        uncompressed = uncompressed
            .checked_add(entry.size())
            .ok_or("zip archive size overflow")?;
        if uncompressed > MAX_UNCOMPRESSED_BYTES {
            return Err("zip archive expands beyond the 1 GiB safety limit".into());
        }
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| format!("unsafe path in zip: {}", entry.name()))?;
        let mut components = enclosed.components();
        let root = match components.next() {
            Some(std::path::Component::Normal(value)) => value.to_os_string(),
            _ => return Err(format!("unsafe path in zip: {}", entry.name())),
        };
        if !has_root {
            continue;
        }
        if components.next().is_none() && !entry.is_dir() {
            has_root = false;
            continue;
        }
        match &common_root {
            None => common_root = Some(root),
            Some(r) if *r != root => {
                has_root = false;
            }
            _ => {}
        }
    }
    let strip = if has_root { common_root } else { None };

    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut extracted = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        validate_zip_entry(&entry)?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| format!("unsafe path in zip: {}", entry.name()))?;
        let rel = match &strip {
            Some(root) => enclosed
                .strip_prefix(Path::new(root))
                .map_err(|_| format!("invalid common root in zip: {}", entry.name()))?,
            None => enclosed.as_path(),
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out_path = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            let remaining = MAX_UNCOMPRESSED_BYTES.saturating_sub(extracted);
            let copied = std::io::copy(&mut entry.by_ref().take(remaining + 1), &mut out)
                .map_err(|e| e.to_string())?;
            if copied > remaining {
                return Err("zip archive expands beyond the 1 GiB safety limit".into());
            }
            extracted += copied;
        }
    }
    Ok(())
}

fn validate_zip_entry(entry: &zip::read::ZipFile<'_>) -> Result<(), String> {
    let normalized = entry.name().replace('\\', "/");
    let bytes = normalized.as_bytes();
    let drive_prefixed = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if normalized.starts_with('/')
        || normalized.starts_with("//")
        || drive_prefixed
        || normalized.split('/').any(|part| part == "..")
        || normalized.contains(':')
    {
        return Err(format!("unsafe path in zip: {}", entry.name()));
    }
    if entry.is_symlink() {
        return Err(format!(
            "symbolic links are not allowed in zip: {}",
            entry.name()
        ));
    }
    if entry.enclosed_name().is_none() {
        return Err(format!("unsafe path in zip: {}", entry.name()));
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

fn restore_user_files(saved: &[(String, PathBuf)], target_root: &Path) -> Result<(), String> {
    for (rel, backup) in saved {
        let dst = target_root.join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(backup, dst).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn temporary_path(label: &str) -> PathBuf {
    paths::tmp_dir().join(format!(
        "{label}.{}.{}",
        std::process::id(),
        rand::random::<u64>()
    ))
}

fn remove_temporary(path: &Path) {
    if path.is_dir() {
        let _ = crate::util::force_remove_dir(path);
    } else {
        let _ = std::fs::remove_file(path);
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
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
    }
}

/// Moves the existing zapret tree aside without deleting it. The returned
/// directory remains a rollback point until the new installation is fully
/// committed.
async fn relocate_zapret_dir(app: &AppHandle, backup: &Path) -> Result<bool, String> {
    let target = paths::zapret_dir();
    if !target.exists() {
        return Ok(false);
    }

    let mut last_err = String::new();
    for attempt in 0..8u32 {
        if attempt > 0 {
            let state = app.state::<crate::AppState>();
            crate::zapret::service::ensure_zapret_released(&state, false);
            tokio::time::sleep(std::time::Duration::from_millis(
                400 + u64::from(attempt) * 350,
            ))
            .await;
        }
        match std::fs::rename(&target, backup) {
            Ok(()) => {
                logs::append(
                    "app",
                    &format!("Relocated old zapret dir to {}", backup.display()),
                );
                return Ok(true);
            }
            Err(e) => last_err = e.to_string(),
        }
    }

    Err(format!(
        "cannot preserve the existing zapret directory for rollback (close other zapret tools and retry): {last_err}"
    ))
}

fn rollback_zapret_swap(backup: &Path) -> Result<(), String> {
    let target = paths::zapret_dir();
    let failed = temporary_path("zapret.failed");
    if target.exists() {
        std::fs::rename(&target, &failed)
            .map_err(|e| format!("failed to move incomplete zapret install aside: {e}"))?;
    }
    if backup.exists() {
        std::fs::rename(backup, &target)
            .map_err(|e| format!("failed to restore previous zapret install: {e}"))?;
    }
    if failed.exists() {
        schedule_staged_cleanup(failed);
    }
    Ok(())
}

/// Tries to delete a relocated zapret tree in the background once handles drop.
fn schedule_staged_cleanup(path: std::path::PathBuf) {
    std::thread::spawn(move || {
        for i in 0..24 {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            if crate::util::force_remove_dir(&path).is_ok() || !path.exists() {
                logs::append(
                    "app",
                    &format!("Removed staged zapret dir {}", path.display()),
                );
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
    let previous_version = settings::load().zapret_version;
    let trusted = trusted_component("zapret")?;
    let release = fetch_release_by_tag(trusted.repo, trusted.tag).await?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == trusted.name)
        .ok_or_else(|| format!("trusted asset {} is missing from the release", trusted.name))?;

    logs::append(
        "app",
        &format!("Downloading zapret {} ...", release.tag_name),
    );

    // Download, verify and extract before touching the working installation.
    let archive = temporary_path("zapret.download.zip");
    let staging = temporary_path("zapret.staging");
    let backup_dir = temporary_path("zapret.user-backup");
    remove_temporary(&archive);
    remove_temporary(&staging);
    remove_temporary(&backup_dir);
    if let Err(e) = download_asset(app, trusted, asset, &archive).await {
        remove_temporary(&archive);
        return Err(e);
    }

    let _ = app.emit(
        "install-progress",
        InstallProgress {
            component: "zapret".into(),
            phase: "extracting".into(),
            downloaded: 0,
            total: 0,
        },
    );

    if let Err(e) = extract_zip(&archive, &staging) {
        remove_temporary(&archive);
        remove_temporary(&staging);
        return Err(format!("failed to extract verified zapret archive: {e}"));
    }
    if !staging.join("bin").join("winws.exe").is_file() {
        remove_temporary(&archive);
        remove_temporary(&staging);
        return Err("verified zapret archive does not contain bin/winws.exe".into());
    }

    // Keep user lists/flags across updates and place them into staging before
    // the atomic swap, so no post-swap copy can leave a half-configured tree.
    let saved = backup_user_files(&backup_dir);
    if let Err(e) = restore_user_files(&saved, &staging) {
        remove_temporary(&archive);
        remove_temporary(&staging);
        remove_temporary(&backup_dir);
        return Err(format!("failed to prepare user files for update: {e}"));
    }
    unblock_files(&staging);

    // Only now stop the active instance and preserve the complete old tree as
    // a rollback point.
    let prev_state = {
        let state = app.state::<crate::AppState>();
        crate::zapret::service::stop_for_update(&state)
    };
    let old_tree = temporary_path("zapret.previous");
    remove_temporary(&old_tree);

    let old_tree_exists = match relocate_zapret_dir(app, &old_tree).await {
        Ok(value) => value,
        Err(e) => {
            let state = app.state::<crate::AppState>();
            let restore = crate::zapret::service::restore_after_update(&state, &prev_state);
            remove_temporary(&archive);
            remove_temporary(&staging);
            remove_temporary(&backup_dir);
            return Err(match restore {
                Ok(()) => e,
                Err(restore_error) => {
                    format!("{e}; previous run state could not be restored: {restore_error}")
                }
            });
        }
    };

    let target = paths::zapret_dir();
    if let Err(e) = std::fs::rename(&staging, &target) {
        let rollback = rollback_zapret_swap(&old_tree);
        let state = app.state::<crate::AppState>();
        let restore = if rollback.is_ok() {
            crate::zapret::service::restore_after_update(&state, &prev_state)
        } else {
            Err("skipped because file rollback failed".into())
        };
        remove_temporary(&archive);
        remove_temporary(&staging);
        remove_temporary(&backup_dir);
        let activation_error = match rollback {
            Ok(()) => format!("failed to activate prepared zapret update: {e}"),
            Err(rollback_error) => format!(
                "failed to activate prepared zapret update: {e}; rollback also failed: {rollback_error}"
            ),
        };
        return Err(match restore {
            Ok(()) => activation_error,
            Err(restore_error) => {
                format!(
                    "{activation_error}; previous run state could not be restored: {restore_error}"
                )
            }
        });
    }

    // Create the *-user.txt lists the release does not ship (service.bat does
    // this on launch); winws fails to start without them.
    let commit_result = crate::zapret::strategy::ensure_user_lists()
        .map_err(|e| format!("failed to create zapret user lists: {e}"))
        .and_then(|_| {
            settings::update(|s| s.zapret_version = Some(release.tag_name.clone())).map(|_| ())
        });
    if let Err(e) = commit_result {
        let rollback = rollback_zapret_swap(&old_tree);
        let state = app.state::<crate::AppState>();
        let restore = if rollback.is_ok() {
            crate::zapret::service::restore_after_update(&state, &prev_state)
        } else {
            Err("skipped because file rollback failed".into())
        };
        remove_temporary(&archive);
        remove_temporary(&backup_dir);
        let commit_error = match rollback {
            Ok(()) => format!("zapret update was rolled back: {e}"),
            Err(rollback_error) => {
                format!("zapret update failed: {e}; rollback also failed: {rollback_error}")
            }
        };
        return Err(match restore {
            Ok(()) => commit_error,
            Err(restore_error) => {
                format!("{commit_error}; previous run state could not be restored: {restore_error}")
            }
        });
    }

    // Restore whatever was running before the update (service or manual strategy).
    let restore_result = {
        let state = app.state::<crate::AppState>();
        crate::zapret::service::restore_after_update(&state, &prev_state)
    };
    if let Err(restore_error) = restore_result {
        let state = app.state::<crate::AppState>();
        crate::zapret::service::ensure_zapret_released(&state, false);
        let rollback = rollback_zapret_swap(&old_tree);
        let version_rollback = settings::update(|s| s.zapret_version = previous_version.clone());
        let runtime_rollback = if rollback.is_ok() {
            crate::zapret::service::restore_after_update(&state, &prev_state)
        } else {
            Err("skipped because file rollback failed".into())
        };
        remove_temporary(&archive);
        remove_temporary(&backup_dir);

        let mut details = vec![format!(
            "new files could not restore the previous run state: {restore_error}"
        )];
        if let Err(e) = rollback {
            details.push(format!("file rollback failed: {e}"));
        }
        if let Err(e) = version_rollback {
            details.push(format!("version rollback failed: {e}"));
        }
        if let Err(e) = runtime_rollback {
            details.push(format!("old run state could not be restored: {e}"));
        }
        return Err(format!(
            "zapret update was not committed; {}",
            details.join("; ")
        ));
    }

    remove_temporary(&archive);
    remove_temporary(&backup_dir);
    if old_tree_exists {
        schedule_staged_cleanup(old_tree);
    }

    logs::append("app", &format!("zapret {} installed", release.tag_name));
    Ok(release.tag_name)
}

fn rollback_tg_swap(destination: &Path, backup: &Path) -> Result<(), String> {
    let failed = temporary_path("tgproxy.failed.exe");
    if destination.exists() {
        std::fs::rename(destination, &failed)
            .map_err(|e| format!("failed to move incomplete TG proxy aside: {e}"))?;
    }
    if backup.exists() {
        std::fs::rename(backup, destination)
            .map_err(|e| format!("failed to restore previous TG proxy: {e}"))?;
    }
    remove_temporary(&failed);
    Ok(())
}

fn start_and_verify_tg() -> Result<(), String> {
    crate::tg_proxy::start_tg()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut running_since: Option<std::time::Instant> = None;
    while std::time::Instant::now() < deadline {
        if crate::tg_proxy::proxy_running() {
            let since = running_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= std::time::Duration::from_secs(3) {
                return Ok(());
            }
        } else if running_since.is_some() {
            return Err("TG proxy exited immediately after start".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("TG proxy did not remain running for 3 seconds".into())
}

async fn install_tg(app: &AppHandle) -> Result<String, String> {
    let trusted = trusted_component("tgproxy")?;
    let release = fetch_release_by_tag(trusted.repo, trusted.tag).await?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == trusted.name)
        .ok_or_else(|| format!("trusted asset {} is missing from the release", trusted.name))?;

    logs::append(
        "app",
        &format!("Downloading tg-ws-proxy {} ...", release.tag_name),
    );
    let dest = paths::tg_exe();
    let downloaded = temporary_path("tgproxy.download.exe");
    let backup = temporary_path("tgproxy.previous.exe");
    remove_temporary(&downloaded);
    remove_temporary(&backup);
    download_asset(app, trusted, asset, &downloaded).await?;
    unblock_files(&downloaded);

    let was_running = crate::tg_proxy::proxy_running();
    crate::tg_proxy::kill_all();
    for _ in 0..30 {
        if !crate::tg_proxy::proxy_running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if crate::tg_proxy::proxy_running() {
        remove_temporary(&downloaded);
        return Err("cannot update TG proxy while its process is still running".into());
    }

    let had_previous = dest.exists();
    if had_previous {
        if let Err(e) = std::fs::rename(&dest, &backup) {
            remove_temporary(&downloaded);
            let restore = if was_running {
                start_and_verify_tg()
            } else {
                Ok(())
            };
            let update_error = format!("failed to preserve current TG proxy for rollback: {e}");
            return Err(match restore {
                Ok(()) => update_error,
                Err(restore_error) => {
                    format!(
                        "{update_error}; current TG proxy could not be restarted: {restore_error}"
                    )
                }
            });
        }
    }
    if let Err(e) = std::fs::rename(&downloaded, &dest) {
        let rollback = rollback_tg_swap(&dest, &backup);
        let restore = if rollback.is_ok() && was_running {
            start_and_verify_tg()
        } else if rollback.is_err() && was_running {
            Err("old TG proxy restart skipped because file rollback failed".into())
        } else {
            Ok(())
        };
        remove_temporary(&downloaded);
        let update_error = match rollback {
            Ok(()) => format!("failed to activate verified TG proxy: {e}"),
            Err(rollback_error) => format!(
                "failed to activate verified TG proxy: {e}; rollback also failed: {rollback_error}"
            ),
        };
        return Err(match restore {
            Ok(()) => update_error,
            Err(restore_error) => {
                format!("{update_error}; previous TG proxy could not be restored: {restore_error}")
            }
        });
    }

    let restart_result = if was_running {
        start_and_verify_tg()
    } else {
        Ok(())
    };
    let commit_result = restart_result.and_then(|_| {
        settings::update(|s| s.tg_version = Some(release.tag_name.clone())).map(|_| ())
    });
    if let Err(e) = commit_result {
        crate::tg_proxy::kill_all();
        let rollback = rollback_tg_swap(&dest, &backup);
        let restore = if rollback.is_ok() && was_running {
            start_and_verify_tg()
        } else if rollback.is_err() && was_running {
            Err("old TG proxy restart skipped because file rollback failed".into())
        } else {
            Ok(())
        };
        let update_error = match rollback {
            Ok(()) => format!("TG proxy update was rolled back: {e}"),
            Err(rollback_error) => {
                format!("TG proxy update failed: {e}; rollback also failed: {rollback_error}")
            }
        };
        return Err(match restore {
            Ok(()) => update_error,
            Err(restore_error) => {
                format!("{update_error}; previous TG proxy could not be restored: {restore_error}")
            }
        });
    }

    if had_previous {
        remove_temporary(&backup);
    }

    logs::append(
        "app",
        &format!("tg-ws-proxy {} installed", release.tag_name),
    );
    Ok(release.tag_name)
}

#[tauri::command]
pub async fn install_component(app: AppHandle, component: String) -> Result<String, String> {
    let _install_guard = INSTALL_LOCK
        .try_lock()
        .map_err(|_| "another component installation is already running".to_string())?;
    paths::ensure_dirs().map_err(|e| e.to_string())?;
    logs::append("app", &format!("install requested: {component}"));
    let result = match component.as_str() {
        "zapret" => install_zapret(&app).await,
        "tgproxy" => install_tg(&app).await,
        _ => Err(format!("unknown component: {component}")),
    };
    let _ = app.emit(
        "install-progress",
        InstallProgress {
            component: component.clone(),
            phase: if result.is_ok() {
                "done".into()
            } else {
                "error".into()
            },
            downloaded: 0,
            total: 0,
        },
    );
    if let Err(e) = &result {
        logs::error("app", &format!("install {component} failed: {e}"));
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
        tg_version: s.tg_version,
        data_dir: paths::data_dir().to_string_lossy().to_string(),
    }
}

#[tauri::command]
pub async fn check_updates() -> Vec<UpdateStatus> {
    let s = settings::load();
    let mut out = Vec::new();
    logs::append("app", "checking trusted component updates");
    for trusted in TRUSTED_COMPONENT_ASSETS {
        let installed = match trusted.component {
            "zapret" => installed_zapret_version(),
            "tgproxy" => s.tg_version.clone(),
            _ => None,
        };
        match fetch_release_by_tag(trusted.repo, trusted.tag).await {
            Ok(rel) => out.push(UpdateStatus {
                component: trusted.component.into(),
                installed: installed.clone(),
                latest: Some(rel.tag_name.clone()),
                update_available: is_update_available(&installed, &rel.tag_name),
                release_url: Some(rel.html_url),
                error: None,
            }),
            Err(e) => {
                logs::warn(
                    "app",
                    &format!("trusted update check for {} failed: {e}", trusted.component),
                );
                out.push(UpdateStatus {
                    component: trusted.component.into(),
                    installed: installed.clone(),
                    latest: None,
                    update_available: false,
                    release_url: None,
                    error: Some(e),
                });
            }
        }
    }
    for u in &out {
        logs::append(
            "app",
            &format!(
                "update check {}: installed {} · latest {} · update available: {}",
                u.component,
                u.installed.as_deref().unwrap_or("-"),
                u.latest.as_deref().unwrap_or("-"),
                u.update_available
            ),
        );
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

    fn test_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "easyzapret-{label}-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ))
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, data) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

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
        assert!(!is_update_available(&Some("1.10.4".into()), "1.9.9a"));
        assert!(is_update_available(&Some("1.9.9a".into()), "1.10.4"));
        assert!(!is_update_available(&Some("0.7.0".into()), "v0.6.0"));
        assert!(!is_update_available(&None, "1.9.9a"));
    }

    #[test]
    fn zip_extraction_rejects_traversal_and_absolute_paths() {
        for (index, name) in [
            "../outside.txt",
            "/absolute.txt",
            "C:/Windows/evil.txt",
            "safe/../../evil.txt",
        ]
        .iter()
        .enumerate()
        {
            let root = test_directory(&format!("unsafe-zip-{index}"));
            std::fs::create_dir_all(&root).unwrap();
            let archive = root.join("archive.zip");
            let destination = root.join("out");
            write_zip(&archive, &[(name, b"not safe")]);
            assert!(
                extract_zip(&archive, &destination).is_err(),
                "accepted {name}"
            );
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn zip_extraction_strips_one_common_root() {
        let root = test_directory("safe-zip");
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("archive.zip");
        let destination = root.join("out");
        write_zip(
            &archive,
            &[
                ("release/bin/winws.exe", b"exe"),
                ("release/lists/list.txt", b"list"),
            ],
        );
        extract_zip(&archive, &destination).unwrap();
        assert_eq!(
            std::fs::read(destination.join("bin/winws.exe")).unwrap(),
            b"exe"
        );
        assert!(!destination.join("release").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
