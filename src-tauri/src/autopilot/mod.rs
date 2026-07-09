//! Background health monitoring and optional automatic strategy rotation.

use std::sync::atomic::Ordering;
use std::sync::Mutex;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use futures_util::future::join_all;

use crate::settings::{self, AutopilotSettings};
use crate::{logs, paths, AppState};

const CURL_TIMEOUT_SECS: u64 = 6;

fn curl_bin() -> &'static str {
    if cfg!(windows) {
        "curl.exe"
    } else {
        "curl"
    }
}

fn devnull() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedState {
    strategy_index: usize,
    #[serde(default)]
    switch_timestamps_ms: Vec<i64>,
}

fn state_file() -> std::path::PathBuf {
    paths::data_dir().join("autopilot-state.json")
}

fn load_persisted() -> PersistedState {
    std::fs::read_to_string(state_file())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_persisted(state: &PersistedState) {
    let _ = std::fs::create_dir_all(paths::data_dir());
    if let Ok(text) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(state_file(), text);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotStatus {
    pub enabled: bool,
    pub checking: bool,
    pub last_check_at: Option<String>,
    pub last_health_percent: Option<u32>,
    pub last_message: Option<String>,
    pub policy: String,
    pub switch_mode: String,
    pub switches_this_hour: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotEvent {
    pub kind: String,
    pub message: String,
    pub from_strategy: Option<String>,
    pub to_strategy: Option<String>,
    pub health_percent: Option<u32>,
}

struct RuntimeCache {
    checking: bool,
    last_check_at: Option<String>,
    last_health_percent: Option<u32>,
    last_message: Option<String>,
}

static CACHE: Mutex<RuntimeCache> = Mutex::new(RuntimeCache {
    checking: false,
    last_check_at: None,
    last_health_percent: None,
    last_message: None,
});

fn set_cache(patch: impl FnOnce(&mut RuntimeCache)) {
    let mut g = CACHE.lock().unwrap();
    patch(&mut g);
}

pub fn status() -> AutopilotStatus {
    let cfg = settings::load();
    let persisted = load_persisted();
    let cache = CACHE.lock().unwrap();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let hour_ago = now_ms - 3_600_000;
    let switches = persisted
        .switch_timestamps_ms
        .iter()
        .filter(|t| **t >= hour_ago)
        .count() as u32;
    AutopilotStatus {
        enabled: cfg.autopilot.enabled,
        checking: cache.checking,
        last_check_at: cache.last_check_at.clone(),
        last_health_percent: cache.last_health_percent,
        last_message: cache.last_message.clone(),
        policy: cfg.autopilot.policy.clone(),
        switch_mode: cfg.autopilot.switch_mode.clone(),
        switches_this_hour: switches,
    }
}

#[tauri::command]
pub fn get_autopilot_status() -> AutopilotStatus {
    status()
}

fn emit_event(app: &AppHandle, event: AutopilotEvent) {
    let _ = app.emit("autopilot-event", &event);
    logs::append("app", &format!("autopilot: {} — {}", event.kind, event.message));
}

struct Probe {
    url: &'static str,
}

fn probes_for(cfg: &AutopilotSettings) -> Vec<Probe> {
    let mut list = Vec::new();
    if cfg.probe_discord {
        list.push(Probe {
            url: "https://discord.com",
        });
    }
    if cfg.probe_youtube {
        list.push(Probe {
            url: "https://www.youtube.com",
        });
    }
    if cfg.probe_cloudflare {
        list.push(Probe {
            url: "https://www.cloudflare.com",
        });
    }
    if cfg.probe_google {
        list.push(Probe {
            url: "https://www.google.com",
        });
    }
    list
}

async fn probe_url(url: &str) -> bool {
    let mut cmd = crate::util::hidden_command_async(curl_bin());
    cmd.args([
        "-I",
        "-s",
        "-m",
        &CURL_TIMEOUT_SECS.to_string(),
        "-o",
        devnull(),
        "-w",
        "%{http_code}",
        "--http1.1",
        url,
    ])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null());

    let Ok(Ok(out)) = tokio::time::timeout(
        std::time::Duration::from_secs(CURL_TIMEOUT_SECS + 3),
        cmd.output(),
    )
    .await
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
    code.starts_with('2') || code.starts_with('3')
}

async fn measure_health(cfg: &AutopilotSettings) -> (u32, usize, usize) {
    let probes = probes_for(cfg);
    if probes.is_empty() {
        return (100, 0, 0);
    }
    let results = join_all(probes.iter().map(|p| probe_url(p.url))).await;
    let ok = results.iter().filter(|&&r| r).count();
    let total = probes.len();
    let pct = ((ok as f64 / total as f64) * 100.0).round() as u32;
    (pct, ok, total)
}

/// Order strategies for the active policy.
fn strategy_candidates(policy: &str, all: &[String]) -> Vec<String> {
    let lower: Vec<(String, String)> = all
        .iter()
        .map(|s| (s.to_lowercase(), s.clone()))
        .collect();

    let pick = |pred: fn(&str) -> bool| -> Vec<String> {
        let mut v: Vec<String> = lower
            .iter()
            .filter(|(l, _)| pred(l))
            .map(|(_, o)| o.clone())
            .collect();
        v.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        v
    };

    match policy {
        "speed" => {
            let mut v = pick(|l| l.contains("simple fake"));
            let rest = pick(|l| l.starts_with("general") && !l.contains("fake tls"));
            for s in rest {
                if !v.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
                    v.push(s);
                }
            }
            for (_, s) in &lower {
                if !v.iter().any(|x| x.eq_ignore_ascii_case(s)) {
                    v.push(s.clone());
                }
            }
            v
        }
        "gaming" => {
            let mut v = pick(|l| l.starts_with("general") && !l.contains("alt"));
            let rest = pick(|l| l.contains("alt") && !l.contains("fake tls"));
            for s in rest {
                if !v.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
                    v.push(s);
                }
            }
            for (_, s) in &lower {
                if !v.iter().any(|x| x.eq_ignore_ascii_case(s)) {
                    v.push(s.clone());
                }
            }
            v
        }
        _ => {
            let mut v = pick(|l| l == "general.bat");
            let alt = pick(|l| l.starts_with("general (alt") || l.starts_with("general(alt"));
            for s in alt {
                if !v.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
                    v.push(s);
                }
            }
            let rest = pick(|l| l.starts_with("general"));
            for s in rest {
                if !v.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
                    v.push(s);
                }
            }
            for (_, s) in &lower {
                if !v.iter().any(|x| x.eq_ignore_ascii_case(s)) {
                    v.push(s.clone());
                }
            }
            v
        }
    }
}

fn switches_in_last_hour(persisted: &PersistedState) -> usize {
    let hour_ago = chrono::Utc::now().timestamp_millis() - 3_600_000;
    persisted
        .switch_timestamps_ms
        .iter()
        .filter(|t| **t >= hour_ago)
        .count()
}

fn record_switch(persisted: &mut PersistedState) {
    let now = chrono::Utc::now().timestamp_millis();
    persisted.switch_timestamps_ms.push(now);
    let hour_ago = now - 3_600_000;
    persisted.switch_timestamps_ms.retain(|t| *t >= hour_ago);
}

fn filter_allowed(cfg: &AutopilotSettings, all: &[String]) -> Vec<String> {
    cfg.allowed_strategies
        .iter()
        .filter(|s| all.iter().any(|a| a.eq_ignore_ascii_case(s)))
        .cloned()
        .collect()
}

fn resolve_candidate_pool(cfg: &AutopilotSettings, all: &[String]) -> Vec<String> {
    match cfg.switch_mode.as_str() {
        "rotate_custom" => filter_allowed(cfg, all),
        "best_by_tests" => {
            let base = if cfg.allowed_strategies.is_empty() {
                strategy_candidates(&cfg.policy, all)
            } else {
                filter_allowed(cfg, all)
            };
            let cap = cfg.max_test_strategies.clamp(1, 12) as usize;
            base.into_iter().take(cap).collect()
        }
        _ => strategy_candidates(&cfg.policy, all),
    }
}

async fn pick_best_by_tests(candidates: &[String], from: &str) -> Option<String> {
    let mut ranked: Vec<(String, usize)> = Vec::new();
    for strategy in candidates {
        if let Ok((score, _)) = crate::tests::quick_benchmark_strategy(strategy).await {
            ranked.push((strategy.clone(), score));
            logs::append("app", &format!("autopilot benchmark {strategy}: score {score}"));
        }
    }
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .find(|(s, score)| *score > 0 && !s.eq_ignore_ascii_case(from))
        .map(|(s, _)| s)
}

async fn apply_strategy_switch(
    app: &AppHandle,
    cfg: &AutopilotSettings,
    persisted: &mut PersistedState,
    from: &str,
    next: &str,
) -> Option<String> {
    if next.eq_ignore_ascii_case(from) {
        return None;
    }

    let next_owned = next.to_string();
    let from_owned = from.to_string();
    let app_handle = app.clone();

    set_cache(|c| {
        c.checking = true;
        c.last_message = Some(format!("Switching {from_owned} → {next_owned}…"));
    });

    let started = tokio::task::spawn_blocking(move || {
        let state = app_handle.state::<AppState>();
        crate::zapret::process::stop(&state);
        crate::zapret::process::wait_for_winws_exit(std::time::Duration::from_secs(8));
        crate::zapret::process::start_strategy(&state, &next_owned)
    })
    .await;

    set_cache(|c| c.checking = false);

    match started {
        Ok(Ok(())) => {}
        _ => {
            set_cache(|c| c.last_message = Some("Strategy switch failed".into()));
            return None;
        }
    }

    let mut s = settings::load();
    s.selected_strategy = Some(next.to_string());
    let _ = settings::save(&s);

    record_switch(persisted);
    save_persisted(persisted);

    if cfg.notify_on_switch {
        emit_event(
            app,
            AutopilotEvent {
                kind: "strategy_switched".into(),
                message: format!("{from} → {next}"),
                from_strategy: Some(from.into()),
                to_strategy: Some(next.into()),
                health_percent: None,
            },
        );
    }
    Some(next.to_string())
}

async fn try_switch_strategy(
    app: &AppHandle,
    cfg: &AutopilotSettings,
    persisted: &mut PersistedState,
    from: &str,
) -> Option<String> {
    if !cfg.auto_switch_strategy {
        return None;
    }
    if switches_in_last_hour(persisted) >= cfg.max_switches_per_hour as usize {
        return None;
    }

    if crate::zapret::service::query_service_state("zapret").is_some() {
        return None;
    }

    let all = crate::zapret::strategy::list_strategies().ok()?;
    let candidates = resolve_candidate_pool(cfg, &all);
    if candidates.is_empty() {
        if cfg.switch_mode == "rotate_custom" {
            set_cache(|c| c.last_message = Some("custom list empty or invalid".into()));
        }
        return None;
    }

    let next = match cfg.switch_mode.as_str() {
        "best_by_tests" => {
            set_cache(|c| c.checking = true);
            let best = pick_best_by_tests(&candidates, from).await;
            set_cache(|c| c.checking = false);
            best?
        }
        _ => {
            if persisted.strategy_index >= candidates.len() {
                persisted.strategy_index = 0;
            }
            let next = candidates[persisted.strategy_index].clone();
            persisted.strategy_index = (persisted.strategy_index + 1) % candidates.len();
            save_persisted(persisted);
            if next.eq_ignore_ascii_case(from) {
                return None;
            }
            next
        }
    };

    apply_strategy_switch(app, cfg, persisted, from, &next).await
}

async fn maybe_enable_modules(app: &AppHandle, state: &AppState, cfg: &AutopilotSettings) {
    if cfg.auto_enable_warp && crate::warp::is_installed() && !crate::warp::quick_status().connected {
        if crate::warp::zapret_running() {
            let _ = crate::warp::connect_with_state(state);
        }
    }
    let _ = app;
}

struct AutopilotBusyGuard(AppHandle);

impl Drop for AutopilotBusyGuard {
    fn drop(&mut self) {
        self.0
            .state::<AppState>()
            .autopilot_busy
            .store(false, Ordering::SeqCst);
    }
}

pub async fn run_health_cycle(app: AppHandle) {
    let state = app.state::<AppState>();
    if state
        .autopilot_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let _busy = AutopilotBusyGuard(app.clone());

    let cfg = settings::load().autopilot;
    if !cfg.enabled {
        return;
    }

    if state.tests_running.load(Ordering::SeqCst) {
        return;
    }

    let zapret_on = crate::zapret::process::winws_running()
        || crate::zapret::service::query_service_state("zapret").as_deref() == Some("RUNNING");

    if cfg.only_when_zapret_running && !zapret_on {
        set_cache(|c| {
            c.last_message = Some("skipped: zapret off".into());
        });
        return;
    }

    set_cache(|c| c.checking = true);

    #[cfg(windows)]
    if cfg.policy == "gaming" && cfg.switch_mode != "best_by_tests" && zapret_on {
        let _ = crate::zapret::service::set_game_filter("all".into());
    }

    let (health, ok, total) = measure_health(&cfg).await;
    let stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    set_cache(|c| {
        c.checking = false;
        c.last_check_at = Some(stamp);
        c.last_health_percent = Some(health);
        c.last_message = Some(format!("{ok}/{total} probes OK ({health}%)"));
    });

    logs::append(
        "app",
        &format!("autopilot health: {health}% ({ok}/{total})"),
    );

    if health < cfg.min_health_percent {
        if cfg.notify_on_degraded {
            emit_event(
                &app,
                AutopilotEvent {
                    kind: "health_degraded".into(),
                    message: format!("{health}% < {}", cfg.min_health_percent),
                    from_strategy: None,
                    to_strategy: None,
                    health_percent: Some(health),
                },
            );
        }

        if zapret_on {
            let current = settings::load()
                .selected_strategy
                .unwrap_or_else(|| "general.bat".into());
            let mut persisted = load_persisted();
            let _ = try_switch_strategy(&app, &cfg, &mut persisted, &current).await;
            maybe_enable_modules(&app, &state, &cfg).await;
        }
    }

    crate::tray::update_tray_now(&app);
}

pub fn start_background_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(45)).await;
        loop {
            let minutes = settings::load().autopilot.interval_minutes.clamp(5, 120);
            let app_clone = app.clone();
            run_health_cycle(app_clone).await;
            tokio::time::sleep(std::time::Duration::from_secs(u64::from(minutes) * 60)).await;
        }
    });
}

#[tauri::command]
pub async fn run_autopilot_check_now(app: AppHandle) -> AutopilotStatus {
    let state = app.state::<AppState>();
    if state.autopilot_busy.load(Ordering::SeqCst) {
        return status();
    }
    tauri::async_runtime::spawn(async move {
        run_health_cycle(app).await;
    });
    status()
}
