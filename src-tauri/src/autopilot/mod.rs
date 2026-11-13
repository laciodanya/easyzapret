//! Background health monitoring with self-verifying strategy recovery.
//!
//! A cycle measures every enabled service, re-checks before reacting (so a
//! single network hiccup never triggers a switch), tells "bypass broke" apart
//! from "internet is down", and then walks through candidate strategies while
//! *measuring each one*. It keeps whichever worked, or rolls back to the best
//! one it saw, and remembers per-strategy results to try proven ones first.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::services::{self, ServiceHealth};
use crate::settings::{self, AutopilotSettings};
use crate::{logs, paths, AppState};

/// Pause before re-measuring a bad result, to ride out transient failures.
const CONFIRM_DELAY_SECS: u64 = 15;
/// Time winws needs to start filtering before a new strategy is measured.
const WARMUP_SECS: u64 = 6;
const OFFLINE_RETRY_MINUTES: u32 = 2;
const UNRESOLVED_RETRY_MINUTES: u32 = 10;
/// After a recovery that found nothing, leave the connection alone for a while.
const FAILURE_COOLDOWN_MS: i64 = 20 * 60_000;
/// A strategy that failed this recently is tried last.
const RECENT_FAIL_MS: i64 = 30 * 60_000;
const MAX_ROTATE_TRIALS: usize = 4;
const HISTORY_LIMIT: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StrategyStat {
    #[serde(default)]
    ok: u32,
    #[serde(default)]
    fail: u32,
    #[serde(default)]
    last_ok_ms: i64,
    #[serde(default)]
    last_fail_ms: i64,
    #[serde(default)]
    last_health: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedState {
    #[serde(default)]
    switch_timestamps_ms: Vec<i64>,
    #[serde(default)]
    stats: HashMap<String, StrategyStat>,
    #[serde(default)]
    cooldown_until_ms: i64,
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

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub at: String,
    /// check_ok | degraded | flaky | offline | switch | recovered | failed | skipped
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotStatus {
    pub enabled: bool,
    pub checking: bool,
    /// A strategy change is in progress; Zapret toggles should wait.
    pub switching: bool,
    pub last_check_at: Option<String>,
    pub last_health_percent: Option<u32>,
    pub last_message: Option<String>,
    pub policy: String,
    pub switch_mode: String,
    pub switches_this_hour: u32,
    pub services: Vec<ServiceHealth>,
    pub internet_ok: Option<bool>,
    pub next_check_at: Option<String>,
    /// Newest first.
    pub history: Vec<HistoryEntry>,
    /// Zapret runs as a Windows service, so only monitoring is possible.
    pub monitoring_only: bool,
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
    switching: bool,
    last_check_at: Option<String>,
    last_health_percent: Option<u32>,
    last_message: Option<String>,
    services: Vec<ServiceHealth>,
    internet_ok: Option<bool>,
    next_check_at: Option<String>,
    history: VecDeque<HistoryEntry>,
}

static CACHE: Mutex<RuntimeCache> = Mutex::new(RuntimeCache {
    checking: false,
    switching: false,
    last_check_at: None,
    last_health_percent: None,
    last_message: None,
    services: Vec::new(),
    internet_ok: None,
    next_check_at: None,
    history: VecDeque::new(),
});

fn set_cache(patch: impl FnOnce(&mut RuntimeCache)) {
    let mut g = CACHE.lock().unwrap();
    patch(&mut g);
}

/// Records a decision in the in-app history and the autopilot log.
fn note(kind: &str, message: impl Into<String>) {
    let message = message.into();
    match kind {
        "degraded" | "offline" | "failed" => logs::warn("autopilot", &message),
        _ => logs::append("autopilot", &message),
    }
    let entry = HistoryEntry {
        at: Local::now().format("%H:%M:%S").to_string(),
        kind: kind.to_string(),
        message: message.clone(),
    };
    set_cache(|c| {
        c.last_message = Some(message);
        c.history.push_front(entry);
        c.history.truncate(HISTORY_LIMIT);
    });
}

pub fn status() -> AutopilotStatus {
    status_with(crate::zapret::service::query_service_state("zapret").is_some())
}

/// Same as [`status`] when the caller already knows whether the Windows
/// service exists (the polling endpoint caches that).
pub fn status_with(service_installed: bool) -> AutopilotStatus {
    let cfg = settings::load();
    let persisted = load_persisted();
    let cache = CACHE.lock().unwrap();
    AutopilotStatus {
        enabled: cfg.autopilot.enabled,
        checking: cache.checking,
        switching: cache.switching,
        last_check_at: cache.last_check_at.clone(),
        last_health_percent: cache.last_health_percent,
        last_message: cache.last_message.clone(),
        policy: cfg.autopilot.policy.clone(),
        switch_mode: cfg.autopilot.switch_mode.clone(),
        switches_this_hour: switches_in_last_hour(&persisted) as u32,
        services: cache.services.clone(),
        internet_ok: cache.internet_ok,
        next_check_at: cache.next_check_at.clone(),
        history: cache.history.iter().cloned().collect(),
        monitoring_only: service_installed,
    }
}

#[tauri::command]
pub fn get_autopilot_status() -> AutopilotStatus {
    status()
}

/// Toast notification for the UI; the decision itself is logged by `note`.
fn emit_toast(app: &AppHandle, event: AutopilotEvent) {
    let _ = app.emit("autopilot-event", &event);
}

// ---------------------------------------------------------------- measuring

fn enabled_service_ids(cfg: &AutopilotSettings) -> Vec<&'static str> {
    let mut ids = Vec::new();
    if cfg.probe_discord {
        ids.push("discord");
    }
    if cfg.probe_youtube {
        ids.push("youtube");
    }
    if cfg.probe_telegram {
        ids.push("telegram");
    }
    if cfg.probe_cloudflare {
        ids.push("cloudflare");
    }
    if cfg.probe_google {
        ids.push("google");
    }
    ids
}

struct Measurement {
    health: u32,
    services: Vec<ServiceHealth>,
}

impl Measurement {
    /// Below the threshold, or any single service is completely unreachable.
    fn degraded(&self, cfg: &AutopilotSettings) -> bool {
        self.health < cfg.min_health_percent || self.services.iter().any(|s| s.state == "down")
    }

    fn summary(&self) -> String {
        self.services
            .iter()
            .map(|s| {
                let mut part = format!("{} {}/{}", s.id, s.ok, s.total);
                if s.bulk_blocked {
                    part.push_str(" (throttled)");
                }
                part
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn failing(&self) -> String {
        self.services
            .iter()
            .flat_map(|s| s.failed.iter().map(move |f| format!("{}: {f}", s.id)))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

async fn measure(cfg: &AutopilotSettings) -> Measurement {
    let ids = enabled_service_ids(cfg);
    if ids.is_empty() {
        return Measurement {
            health: 100,
            services: Vec::new(),
        };
    }
    let services = services::measure(&ids, cfg.deep_check).await;
    let ok: usize = services.iter().map(|s| s.ok).sum();
    let total: usize = services.iter().map(|s| s.total).sum();
    let health = if total == 0 {
        100
    } else {
        ((ok as f64 / total as f64) * 100.0).round() as u32
    };
    Measurement { health, services }
}

// ----------------------------------------------------------------- strategies

/// Order strategies for the active policy.
fn strategy_candidates(policy: &str, all: &[String]) -> Vec<String> {
    let lower: Vec<(String, String)> = all.iter().map(|s| (s.to_lowercase(), s.clone())).collect();

    let pick = |pred: fn(&str) -> bool| -> Vec<String> {
        let mut v: Vec<String> = lower
            .iter()
            .filter(|(l, _)| pred(l))
            .map(|(_, o)| o.clone())
            .collect();
        v.sort_by_key(|a| a.to_lowercase());
        v
    };

    let push_unique = |v: &mut Vec<String>, items: Vec<String>| {
        for s in items {
            if !v.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
                v.push(s);
            }
        }
    };

    let mut ordered: Vec<String> = match policy {
        "speed" => pick(|l| l.contains("simple fake")),
        "gaming" => pick(|l| l.starts_with("general") && !l.contains("alt")),
        _ => pick(|l| l == "general.bat"),
    };

    match policy {
        "speed" => push_unique(
            &mut ordered,
            pick(|l| l.starts_with("general") && !l.contains("fake tls")),
        ),
        "gaming" => push_unique(
            &mut ordered,
            pick(|l| l.contains("alt") && !l.contains("fake tls")),
        ),
        _ => {
            push_unique(
                &mut ordered,
                pick(|l| l.starts_with("general (alt") || l.starts_with("general(alt")),
            );
            push_unique(&mut ordered, pick(|l| l.starts_with("general")));
        }
    }
    push_unique(&mut ordered, lower.iter().map(|(_, o)| o.clone()).collect());
    ordered
}

fn switches_in_last_hour(persisted: &PersistedState) -> usize {
    let hour_ago = now_ms() - 3_600_000;
    persisted
        .switch_timestamps_ms
        .iter()
        .filter(|t| **t >= hour_ago)
        .count()
}

fn remaining_switch_budget(persisted: &PersistedState, maximum: u32) -> usize {
    (maximum as usize).saturating_sub(switches_in_last_hour(persisted))
}

fn record_switch(persisted: &mut PersistedState) {
    let now = now_ms();
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

/// Strategies that worked before go first, ones that just failed go last;
/// everything else keeps the policy order.
fn order_by_history(candidates: Vec<String>, persisted: &PersistedState) -> Vec<String> {
    let now = now_ms();
    let tier = |name: &String| -> u8 {
        match persisted.stats.get(name) {
            Some(s) if s.last_ok_ms > s.last_fail_ms => 0,
            Some(s) if s.last_fail_ms > s.last_ok_ms && now - s.last_fail_ms < RECENT_FAIL_MS => 2,
            _ => 1,
        }
    };
    let mut keyed: Vec<(u8, usize, String)> = candidates
        .into_iter()
        .enumerate()
        .map(|(i, c)| (tier(&c), i, c))
        .collect();
    keyed.sort_by_key(|(t, i, _)| (*t, *i));
    keyed.into_iter().map(|(_, _, c)| c).collect()
}

fn mark_result(persisted: &mut PersistedState, strategy: &str, health: u32, healthy: bool) {
    let stat = persisted.stats.entry(strategy.to_string()).or_default();
    stat.last_health = health;
    if healthy {
        stat.ok += 1;
        stat.last_ok_ms = now_ms();
    } else {
        stat.fail += 1;
        stat.last_fail_ms = now_ms();
    }
}

fn current_strategy(state: &AppState) -> String {
    state
        .current_strategy
        .lock()
        .unwrap()
        .clone()
        .or_else(|| settings::load().selected_strategy)
        .unwrap_or_else(|| "general.bat".into())
}

fn pretty(name: &str) -> String {
    name.trim_end_matches(".bat")
        .trim_end_matches(".BAT")
        .to_string()
}

/// Stops winws and starts `next`. Runs on a blocking thread.
async fn switch_to(app: &AppHandle, next: &str) -> Result<(), String> {
    let handle = app.clone();
    let name = next.to_string();
    let result = tokio::task::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        crate::zapret::process::stop(&state);
        crate::zapret::process::wait_for_winws_exit(Duration::from_secs(8));
        crate::zapret::process::start_strategy(&state, &name)
    })
    .await
    .map_err(|e| format!("switch task failed: {e}"))?;
    if result.is_ok() {
        let _ = settings::update(|s| s.selected_strategy = Some(next.to_string()));
    }
    result
}

struct Trial {
    strategy: String,
    health: u32,
    healthy: bool,
}

/// Walks through candidate strategies, measuring each, and ends on the best
/// one. Returns whether the connection is healthy afterwards.
async fn recover(
    app: &AppHandle,
    cfg: &AutopilotSettings,
    persisted: &mut PersistedState,
    from: &str,
    before: &Measurement,
    force: bool,
) -> bool {
    if !cfg.auto_switch_strategy {
        note("skipped", "Auto-switching is off — nothing changed");
        return false;
    }
    if crate::zapret::service::query_service_state("zapret").is_some() {
        note(
            "skipped",
            "Zapret runs as a Windows service, so the strategy cannot be changed automatically",
        );
        return false;
    }
    if !force && now_ms() < persisted.cooldown_until_ms {
        let mins = ((persisted.cooldown_until_ms - now_ms()) / 60_000).max(1);
        note(
            "skipped",
            format!("Cooling down after an unsuccessful recovery ({mins} min left)"),
        );
        return false;
    }
    let switch_budget = remaining_switch_budget(persisted, cfg.max_switches_per_hour);
    // Keep one switch in reserve so a failed benchmark can always return to
    // the original/best strategy without exceeding the configured limit.
    if switch_budget < 2 {
        note("skipped", "Hourly limit of automatic switches reached");
        return false;
    }

    let all = match crate::zapret::strategy::list_strategies() {
        Ok(v) => v,
        Err(e) => {
            note("failed", format!("Cannot list strategies: {e}"));
            return false;
        }
    };
    let best_mode = cfg.switch_mode == "best_by_tests";
    let mut candidates: Vec<String> = resolve_candidate_pool(cfg, &all)
        .into_iter()
        .filter(|c| !c.eq_ignore_ascii_case(from))
        .collect();
    if !best_mode {
        candidates = order_by_history(candidates, persisted);
        candidates.truncate(MAX_ROTATE_TRIALS);
    }
    candidates.truncate(switch_budget - 1);
    if candidates.is_empty() {
        let reason = if cfg.switch_mode == "rotate_custom" {
            "Your custom strategy list is empty or contains unknown files"
        } else {
            "No other strategies available"
        };
        note("skipped", reason);
        return false;
    }

    set_cache(|c| {
        c.switching = true;
        c.checking = true;
    });

    note(
        "degraded",
        format!(
            "Trying up to {} strateg{} to fix {} ({}% now)",
            candidates.len(),
            if candidates.len() == 1 { "y" } else { "ies" },
            before.failing(),
            before.health
        ),
    );

    let mut trials: Vec<Trial> = vec![Trial {
        strategy: from.to_string(),
        health: before.health,
        healthy: false,
    }];

    for candidate in &candidates {
        set_cache(|c| c.last_message = Some(format!("Testing {}…", pretty(candidate))));
        if let Err(e) = switch_to(app, candidate).await {
            logs::warn("autopilot", &format!("{candidate}: could not start — {e}"));
            mark_result(persisted, candidate, 0, false);
            trials.push(Trial {
                strategy: candidate.clone(),
                health: 0,
                healthy: false,
            });
            continue;
        }
        record_switch(persisted);
        save_persisted(persisted);
        tokio::time::sleep(Duration::from_secs(WARMUP_SECS)).await;

        let running = tokio::task::spawn_blocking(crate::zapret::process::winws_running)
            .await
            .unwrap_or(false);
        let m = if running {
            measure(cfg).await
        } else {
            Measurement {
                health: 0,
                services: Vec::new(),
            }
        };
        let healthy = running && !m.degraded(cfg);
        logs::append(
            "autopilot",
            &format!(
                "{}: {}% [{}] -> {}",
                pretty(candidate),
                m.health,
                m.summary(),
                if healthy { "healthy" } else { "not enough" }
            ),
        );
        mark_result(persisted, candidate, m.health, healthy);
        trials.push(Trial {
            strategy: candidate.clone(),
            health: m.health,
            healthy,
        });

        if healthy && !best_mode {
            break;
        }
    }
    save_persisted(persisted);

    // Pick the winner: best healthy result, otherwise the best seen overall
    // (which may be the strategy we started with — that is the rollback).
    // Reversed so that ties resolve to the earliest trial (the original strategy).
    let winner = trials
        .iter()
        .rev()
        .filter(|t| t.healthy)
        .max_by_key(|t| t.health)
        .or_else(|| trials.iter().rev().max_by_key(|t| t.health))
        .map(|t| (t.strategy.clone(), t.health, t.healthy));
    let (winner_name, winner_health, winner_healthy) =
        winner.unwrap_or((from.to_string(), before.health, false));
    let changed = !winner_name.eq_ignore_ascii_case(from);
    let mut winner_active = true;

    let running_now = current_strategy(&app.state::<AppState>());
    let still_running = crate::zapret::process::winws_running();
    if !winner_name.eq_ignore_ascii_case(&running_now) || !still_running {
        match switch_to(app, &winner_name).await {
            Ok(()) => {
                record_switch(persisted);
                save_persisted(persisted);
            }
            Err(e) => {
                winner_active = false;
                note(
                    "failed",
                    format!("Could not return to {}: {e}", pretty(&winner_name)),
                );
            }
        }
    }

    set_cache(|c| {
        c.switching = false;
        c.checking = false;
    });

    if winner_active && changed {
        emit_toast(
            app,
            AutopilotEvent {
                kind: "strategy_changed".into(),
                message: winner_name.clone(),
                from_strategy: Some(from.into()),
                to_strategy: Some(winner_name.clone()),
                health_percent: Some(winner_health),
            },
        );
    }

    if winner_healthy && winner_active {
        persisted.cooldown_until_ms = 0;
        save_persisted(persisted);
        note(
            if changed { "switch" } else { "recovered" },
            format!(
                "{} → {} ({}%)",
                pretty(from),
                pretty(&winner_name),
                winner_health
            ),
        );
        if changed && cfg.notify_on_switch {
            emit_toast(
                app,
                AutopilotEvent {
                    kind: "strategy_switched".into(),
                    message: format!("{from} → {winner_name}"),
                    from_strategy: Some(from.into()),
                    to_strategy: Some(winner_name.clone()),
                    health_percent: Some(winner_health),
                },
            );
        }
        true
    } else {
        persisted.cooldown_until_ms = now_ms() + FAILURE_COOLDOWN_MS;
        save_persisted(persisted);
        note(
            "failed",
            if winner_active {
                format!(
                    "No strategy reached the target. Kept {} ({}%). Next automatic attempt in ~20 min",
                    pretty(&winner_name),
                    winner_health
                )
            } else {
                format!(
                    "Could not activate {}. Next automatic attempt in ~20 min",
                    pretty(&winner_name)
                )
            },
        );
        if cfg.notify_on_switch {
            emit_toast(
                app,
                AutopilotEvent {
                    kind: "no_fix".into(),
                    message: pretty(&winner_name),
                    from_strategy: Some(from.into()),
                    to_strategy: winner_active.then_some(winner_name),
                    health_percent: Some(winner_health),
                },
            );
        }
        false
    }
}

async fn maybe_enable_modules(state: &AppState, cfg: &AutopilotSettings) {
    if cfg.auto_enable_warp
        && crate::warp::is_installed()
        && !crate::warp::quick_status().connected
        && crate::warp::zapret_running()
    {
        match crate::warp::connect_with_state(state) {
            Ok(()) => note("info", "WARP enabled automatically"),
            Err(e) => note("failed", format!("WARP could not be enabled: {e}")),
        }
    }
}

struct AutopilotBusyGuard(AppHandle);

impl Drop for AutopilotBusyGuard {
    fn drop(&mut self) {
        self.0
            .state::<AppState>()
            .autopilot_busy
            .store(false, Ordering::SeqCst);
        set_cache(|c| {
            c.checking = false;
            c.switching = false;
        });
    }
}

/// One full check. Returns the number of minutes to wait before the next one,
/// or `None` when the cycle did not run.
pub async fn run_health_cycle(app: AppHandle, force: bool) -> Option<u32> {
    let state = app.state::<AppState>();
    if state
        .autopilot_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return None;
    }
    let _busy = AutopilotBusyGuard(app.clone());

    let cfg = settings::load().autopilot;
    if !cfg.enabled || state.tests_running.load(Ordering::SeqCst) {
        return None;
    }
    let interval = cfg.interval_minutes.clamp(5, 120);

    let zapret_on = crate::zapret::process::winws_running()
        || crate::zapret::service::query_service_state("zapret").as_deref() == Some("RUNNING");

    if cfg.only_when_zapret_running && !zapret_on {
        set_cache(|c| {
            c.last_message = Some("Waiting: Zapret is off".into());
            c.services.clear();
            c.last_health_percent = None;
        });
        return Some(interval);
    }

    set_cache(|c| c.checking = true);

    #[cfg(windows)]
    if cfg.policy == "gaming" && cfg.switch_mode != "best_by_tests" && zapret_on {
        let _ = crate::zapret::service::set_game_filter("all".into());
    }

    let strategy = current_strategy(&state);
    let mut m = measure(&cfg).await;

    if m.degraded(&cfg) {
        logs::warn(
            "autopilot",
            &format!(
                "first check: {}% [{}] — re-checking in {CONFIRM_DELAY_SECS}s",
                m.health,
                m.summary()
            ),
        );
        tokio::time::sleep(Duration::from_secs(CONFIRM_DELAY_SECS)).await;
        m = measure(&cfg).await;
    }

    let stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    set_cache(|c| {
        c.last_check_at = Some(stamp);
        c.last_health_percent = Some(m.health);
        c.services = m.services.clone();
    });
    logs::append(
        "autopilot",
        &format!(
            "health {}% [{}] · strategy {}",
            m.health,
            m.summary(),
            pretty(&strategy)
        ),
    );

    let mut persisted = load_persisted();
    let mut next_in = interval;

    if !m.degraded(&cfg) {
        set_cache(|c| c.internet_ok = Some(true));
        note(
            "check_ok",
            format!("Everything works: {}% ({})", m.health, m.summary()),
        );
        if zapret_on {
            mark_result(&mut persisted, &strategy, m.health, true);
            save_persisted(&persisted);
        }
    } else if !crate::probe::internet_reachable().await {
        set_cache(|c| c.internet_ok = Some(false));
        note(
            "offline",
            "No internet connection — strategies left unchanged",
        );
        next_in = OFFLINE_RETRY_MINUTES;
    } else {
        set_cache(|c| c.internet_ok = Some(true));
        note(
            "degraded",
            format!("Access degraded: {}% — {}", m.health, m.failing()),
        );
        if cfg.notify_on_degraded {
            emit_toast(
                &app,
                AutopilotEvent {
                    kind: "health_degraded".into(),
                    message: format!("{}% < {}", m.health, cfg.min_health_percent),
                    from_strategy: None,
                    to_strategy: None,
                    health_percent: Some(m.health),
                },
            );
        }
        if zapret_on {
            let fixed = recover(&app, &cfg, &mut persisted, &strategy, &m, force).await;
            if !fixed {
                maybe_enable_modules(&state, &cfg).await;
                next_in = interval.min(UNRESOLVED_RETRY_MINUTES);
            }
        }
    }

    crate::tray::update_tray_now(&app);
    Some(next_in)
}

pub fn start_background_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Let the app and an optional autostart finish before the first look.
        tokio::time::sleep(Duration::from_secs(30)).await;
        let mut was_enabled = false;
        let mut next_due = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
            let cfg = settings::load().autopilot;
            if !cfg.enabled {
                if was_enabled {
                    set_cache(|c| c.next_check_at = None);
                }
                was_enabled = false;
                continue;
            }
            if !was_enabled {
                was_enabled = true;
                next_due = std::time::Instant::now() + Duration::from_secs(5);
            }
            if std::time::Instant::now() < next_due {
                continue;
            }
            let minutes = run_health_cycle(app.clone(), false)
                .await
                .unwrap_or_else(|| cfg.interval_minutes.clamp(5, 120));
            next_due = std::time::Instant::now() + Duration::from_secs(u64::from(minutes) * 60);
            let at = (Local::now() + chrono::Duration::minutes(i64::from(minutes)))
                .format("%H:%M")
                .to_string();
            set_cache(|c| c.next_check_at = Some(at));
        }
    });
}

#[tauri::command]
pub async fn run_autopilot_check_now(app: AppHandle) -> AutopilotStatus {
    let state = app.state::<AppState>();
    if state.autopilot_busy.load(Ordering::SeqCst) {
        return status();
    }
    logs::append("autopilot", "manual check requested");
    set_cache(|c| c.checking = true);
    tauri::async_runtime::spawn(async move {
        run_health_cycle(app, true).await;
    });
    status()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn stability_policy_starts_with_general() {
        let all = names(&[
            "general (ALT2).bat",
            "general.bat",
            "general (ALT).bat",
            "general (SIMPLE FAKE).bat",
        ]);
        let ordered = strategy_candidates("stability", &all);
        assert_eq!(ordered[0], "general.bat");
        assert_eq!(ordered.len(), all.len());
    }

    #[test]
    fn proven_strategies_go_first_and_recent_failures_last() {
        let mut persisted = PersistedState::default();
        persisted.stats.insert(
            "b.bat".into(),
            StrategyStat {
                last_ok_ms: now_ms() - 1000,
                last_fail_ms: 0,
                ..Default::default()
            },
        );
        persisted.stats.insert(
            "a.bat".into(),
            StrategyStat {
                last_fail_ms: now_ms() - 1000,
                last_ok_ms: 0,
                ..Default::default()
            },
        );
        let ordered = order_by_history(names(&["a.bat", "c.bat", "b.bat"]), &persisted);
        assert_eq!(ordered, names(&["b.bat", "c.bat", "a.bat"]));
    }

    #[test]
    fn switch_budget_counts_each_recent_transition() {
        let now = now_ms();
        let mut persisted = PersistedState {
            switch_timestamps_ms: vec![now - 1_000, now - 7_200_000],
            ..Default::default()
        };
        assert_eq!(switches_in_last_hour(&persisted), 1);
        assert_eq!(remaining_switch_budget(&persisted, 3), 2);

        record_switch(&mut persisted);
        assert_eq!(switches_in_last_hour(&persisted), 2);
        assert_eq!(remaining_switch_budget(&persisted, 3), 1);
        assert!(persisted
            .switch_timestamps_ms
            .iter()
            .all(|t| *t >= now - 3_600_000));
    }

    #[test]
    fn degraded_when_a_service_is_fully_down() {
        let cfg = AutopilotSettings::default();
        let svc = |id: &str, ok: usize, total: usize| ServiceHealth {
            id: id.into(),
            ok,
            total,
            avg_ms: None,
            bulk_blocked: false,
            failed: Vec::new(),
            state: if ok == 0 { "down".into() } else { "ok".into() },
        };
        let fine = Measurement {
            health: 90,
            services: vec![svc("discord", 5, 5), svc("youtube", 4, 5)],
        };
        assert!(!fine.degraded(&cfg));
        let one_down = Measurement {
            health: 75,
            services: vec![svc("discord", 0, 5), svc("youtube", 5, 5)],
        };
        assert!(one_down.degraded(&cfg));
        let low = Measurement {
            health: 30,
            services: vec![svc("discord", 1, 5)],
        };
        assert!(low.degraded(&cfg));
    }
}
