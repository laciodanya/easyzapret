use std::io::Write;
use std::path::PathBuf;

use tauri::AppHandle;

use crate::paths;

const MAX_READ_BYTES: u64 = 2 * 1024 * 1024;
/// A log file that grows past this is rotated to `<name>.old.log`.
const ROTATE_BYTES: u64 = 4 * 1024 * 1024;

/// Logs EasyZapret writes itself. `zapret` also receives raw winws output.
const LOG_NAMES: &[&str] = &["app", "zapret", "tgproxy", "tests", "warp", "autopilot"];

#[derive(Debug, Clone, Copy)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn tag(self) -> &'static str {
        match self {
            Level::Info => "INFO ",
            Level::Warn => "WARN ",
            Level::Error => "ERROR",
        }
    }

    fn parse(value: &str) -> Level {
        match value.to_lowercase().as_str() {
            "error" => Level::Error,
            "warn" | "warning" => Level::Warn,
            _ => Level::Info,
        }
    }
}

fn log_file(name: &str) -> Result<PathBuf, String> {
    if LOG_NAMES.contains(&name) {
        Ok(paths::logs_dir().join(format!("{name}.log")))
    } else {
        Err(format!("unknown log: {name}"))
    }
}

fn rotate_if_needed(path: &PathBuf) {
    let too_big = std::fs::metadata(path)
        .map(|m| m.len() > ROTATE_BYTES)
        .unwrap_or(false);
    if too_big {
        let old = path.with_extension("old.log");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(path, old);
    }
}

fn write_line(name: &str, line: &str) {
    let Ok(path) = log_file(name) else {
        return;
    };
    let _ = std::fs::create_dir_all(paths::logs_dir());
    rotate_if_needed(&path);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

fn write_leveled(name: &str, level: Level, message: &str) {
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    write_line(name, &format!("[{stamp}] {} {message}", level.tag()));
}

/// Appends an INFO line to one of the app log files.
pub fn append(name: &str, line: &str) {
    write_leveled(name, Level::Info, line);
}

pub fn warn(name: &str, line: &str) {
    write_leveled(name, Level::Warn, line);
}

pub fn error(name: &str, line: &str) {
    write_leveled(name, Level::Error, line);
}

/// Appends a raw line (already formatted by an external process).
pub fn append_raw(name: &str, line: &str) {
    write_line(name, line);
}

fn windows_version() -> String {
    #[cfg(windows)]
    {
        let (_, out) = crate::util::run_capture("cmd", &["/C", "ver"]);
        let text = out
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("");
        if !text.is_empty() {
            return text.to_string();
        }
    }
    std::env::consts::OS.to_string()
}

/// Writes the header that opens every session in the app log, so a pasted log
/// always says which build and system it came from.
pub fn session_start(app_version: &str) {
    let version = app_version.to_string();
    std::thread::spawn(move || {
        append(
            "app",
            "==================== EasyZapret session ====================",
        );
        append(
            "app",
            &format!(
                "version {version} · {} {} · admin: {}",
                std::env::consts::OS,
                std::env::consts::ARCH,
                crate::admin::is_admin()
            ),
        );
        append("app", &format!("system: {}", windows_version()));
        let s = crate::settings::load();
        append(
            "app",
            &format!(
                "language: {} · theme: {} · strategy: {} · zapret: {} · tg-ws-proxy: {}",
                s.language.as_deref().unwrap_or("auto"),
                s.theme,
                s.selected_strategy.as_deref().unwrap_or("-"),
                s.zapret_version.as_deref().unwrap_or("not installed"),
                s.tg_version.as_deref().unwrap_or("not installed"),
            ),
        );
    });
}

#[tauri::command]
pub fn read_log(name: String, max_lines: usize) -> Result<String, String> {
    // The Telegram proxy keeps its own log inside %APPDATA%/TgWsProxy.
    let path = if name == "tgproxy-own" {
        match crate::tg_proxy::own_log_file() {
            Some(p) => p,
            None => return Ok(String::new()),
        }
    } else {
        log_file(&name)?
    };

    if !path.exists() {
        return Ok(String::new());
    }
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let text = if meta.len() > MAX_READ_BYTES {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        f.seek(SeekFrom::End(-(MAX_READ_BYTES as i64)))
            .map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        String::from_utf8_lossy(&buf).to_string()
    } else {
        String::from_utf8_lossy(&std::fs::read(&path).map_err(|e| e.to_string())?).to_string()
    };

    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

#[tauri::command]
pub fn clear_log(name: String) -> Result<(), String> {
    let path = log_file(&name)?;
    if path.exists() {
        std::fs::write(path, "").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn logs_dir_path() -> String {
    paths::logs_dir().to_string_lossy().to_string()
}

/// Entry point for the UI to record its own events and errors in `app.log`.
#[tauri::command]
pub fn log_ui(level: String, message: String) {
    let message: String = message.chars().take(2000).collect();
    write_leveled("app", Level::parse(&level), &format!("[ui] {message}"));
}

/// Removes personal data from text that is about to be shared publicly.
fn redact(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    for secret in secrets.iter().filter(|s| s.len() >= 8) {
        out = out.replace(secret.as_str(), "<hidden>");
    }
    for var in ["USERNAME", "USER"] {
        if let Ok(user) = std::env::var(var) {
            if user.len() >= 3 {
                out = out.replace(&format!("Users\\{user}"), "Users\\<user>");
                out = out.replace(&format!("Users/{user}"), "Users/<user>");
            }
        }
    }
    out
}

fn tail_of(name: &str, lines: usize) -> String {
    read_log(name.to_string(), lines).unwrap_or_default()
}

/// Builds a Markdown report (system, components, settings and the tail of every
/// log) that users can paste into a GitHub issue. Secrets and user names are
/// removed.
pub fn build_report(app: &AppHandle, lines_per_log: usize) -> String {
    let settings = crate::settings::load();
    let components = crate::updates::get_components_state();
    let tg = crate::tg_proxy::tg_status();
    let warp = crate::warp::quick_status();
    let service_state = crate::zapret::service::query_service_state("zapret");
    let windivert_state = crate::zapret::service::query_service_state("WinDivert");

    let mut secrets = Vec::new();
    if let Some(s) = tg.secret.clone() {
        secrets.push(s.trim_start_matches("dd").to_string());
        secrets.push(s);
    }

    let mut report = String::new();
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    report.push_str(&format!("# EasyZapret report\n\nGenerated: {stamp}\n\n"));

    report.push_str("## Environment\n\n");
    report.push_str(&format!("- App version: {}\n", app.package_info().version));
    report.push_str(&format!("- System: {}\n", windows_version()));
    report.push_str(&format!("- Architecture: {}\n", std::env::consts::ARCH));
    report.push_str(&format!("- Administrator: {}\n", crate::admin::is_admin()));
    report.push_str(&format!(
        "- Language: {}\n",
        settings.language.as_deref().unwrap_or("auto")
    ));
    report.push_str(&format!("- Theme: {}\n\n", settings.theme));

    report.push_str("## Components\n\n");
    report.push_str(&format!(
        "- Zapret: {} (installed: {})\n",
        components.zapret_version.as_deref().unwrap_or("-"),
        components.zapret_installed
    ));
    report.push_str(&format!(
        "- Selected strategy: {}\n",
        settings.selected_strategy.as_deref().unwrap_or("-")
    ));
    report.push_str(&format!(
        "- winws running: {} · service: {} · WinDivert: {}\n",
        crate::zapret::process::winws_running(),
        service_state.as_deref().unwrap_or("not installed"),
        windivert_state.as_deref().unwrap_or("not installed"),
    ));
    report.push_str(&format!(
        "- Telegram proxy: {} (installed: {}, running: {}, port: {})\n",
        components.tg_version.as_deref().unwrap_or("-"),
        components.tg_installed,
        tg.running,
        tg.port
    ));
    report.push_str(&format!(
        "- WARP: installed {} · connected {} · {}\n\n",
        warp.installed,
        warp.connected,
        warp.detail.as_deref().unwrap_or("-").replace('\n', " ")
    ));

    report.push_str("## Settings\n\n```json\n");
    let settings_json = serde_json::json!({
        "autopilot": settings.autopilot,
        "autostart": settings.autostart,
        "checkUpdatesOnStart": settings.check_updates_on_start,
    });
    report.push_str(&serde_json::to_string_pretty(&settings_json).unwrap_or_default());
    report.push_str("\n```\n\n");

    for (name, title) in [
        ("app", "Application"),
        ("autopilot", "Autopilot"),
        ("zapret", "Zapret"),
        ("warp", "WARP"),
        ("tgproxy", "Telegram proxy"),
        ("tests", "Tests"),
    ] {
        let tail = tail_of(name, lines_per_log);
        report.push_str(&format!(
            "## {title} log (last {lines_per_log} lines)\n\n```\n"
        ));
        if tail.trim().is_empty() {
            report.push_str("(empty)\n");
        } else {
            report.push_str(&tail);
            report.push('\n');
        }
        report.push_str("```\n\n");
    }

    redact(&report, &secrets)
}

#[tauri::command]
pub fn get_log_report(app: AppHandle, lines_per_log: usize) -> String {
    let lines = lines_per_log.clamp(20, 600);
    append("app", "log report generated");
    build_report(&app, lines)
}

/// Writes the report next to the logs and returns the file path.
#[tauri::command]
pub fn save_log_report(app: AppHandle) -> Result<String, String> {
    let report = build_report(&app, 400);
    let name = format!(
        "easyzapret-report-{}.md",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let path = paths::logs_dir().join(name);
    std::fs::create_dir_all(paths::logs_dir()).map_err(|e| e.to_string())?;
    std::fs::write(&path, report).map_err(|e| e.to_string())?;
    append("app", &format!("log report saved: {}", path.display()));
    Ok(path.to_string_lossy().to_string())
}
