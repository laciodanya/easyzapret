//! Curated probe sets per online service.
//!
//! One definition drives three features: the quick per-service strategy tests,
//! the autopilot health check and the live service check on the home screen.

use futures_util::future::join_all;
use serde::Serialize;

use crate::probe::{self, BulkState};

pub struct Target {
    pub name: &'static str,
    /// HTTPS endpoint probed with a HEAD request.
    pub url: Option<&'static str>,
    /// ICMP target, only used by the quick tests (autopilot skips it).
    pub ping: Option<&'static str>,
    /// Also run the 64 KB bulk-upload freeze check against this host.
    pub bulk: bool,
}

pub struct Service {
    pub id: &'static str,
    pub targets: &'static [Target],
}

const fn web(name: &'static str, url: &'static str) -> Target {
    Target { name, url: Some(url), ping: None, bulk: false }
}

const fn web_bulk(name: &'static str, url: &'static str) -> Target {
    Target { name, url: Some(url), ping: None, bulk: true }
}

const fn ping(name: &'static str, host: &'static str) -> Target {
    Target { name, url: None, ping: Some(host), bulk: false }
}

pub const SERVICES: &[Service] = &[
    Service {
        id: "discord",
        targets: &[
            web("Discord", "https://discord.com"),
            web("Gateway", "https://gateway.discord.gg"),
            web_bulk("CDN", "https://cdn.discordapp.com"),
            web("Media", "https://media.discordapp.net"),
            web("Invites", "https://discord.gg"),
        ],
    },
    Service {
        id: "youtube",
        targets: &[
            web("YouTube", "https://www.youtube.com"),
            web("Thumbnails", "https://i.ytimg.com"),
            web_bulk("Video servers", "https://redirector.googlevideo.com"),
            web("App API", "https://youtubei.googleapis.com"),
            web("Short links", "https://youtu.be"),
        ],
    },
    Service {
        id: "telegram",
        targets: &[
            web("Telegram", "https://telegram.org"),
            web("Web", "https://web.telegram.org"),
            web("t.me", "https://t.me"),
            web("Bot API", "https://api.telegram.org"),
            ping("Media DC 149.154.167.220", "149.154.167.220"),
        ],
    },
    Service {
        id: "cloudflare",
        targets: &[
            web_bulk("Cloudflare", "https://www.cloudflare.com"),
            web("DNS over HTTPS", "https://cloudflare-dns.com"),
            web("WARP endpoint", "https://engage.cloudflareclient.com"),
            ping("1.1.1.1", "1.1.1.1"),
        ],
    },
    Service {
        id: "games",
        targets: &[
            web("Roblox", "https://www.roblox.com"),
            web("Steam", "https://store.steampowered.com"),
            web("Epic Games", "https://www.epicgames.com"),
            web("Riot", "https://auth.riotgames.com"),
        ],
    },
    Service {
        id: "google",
        targets: &[
            web("Google", "https://www.google.com"),
            web("Google APIs", "https://www.googleapis.com"),
        ],
    },
];

pub fn find(id: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|s| s.id == id)
}

/// Host part of a target URL, for bulk probes.
pub fn host_of(url: &str) -> &str {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealth {
    pub id: String,
    pub ok: usize,
    pub total: usize,
    pub avg_ms: Option<u64>,
    /// The bulk-upload freeze check failed (throttling / DPI freeze).
    pub bulk_blocked: bool,
    /// Names of the targets that did not answer.
    pub failed: Vec<String>,
    /// ok | partial | down
    pub state: String,
}

impl ServiceHealth {
    fn finish(mut self) -> Self {
        self.state = if self.ok == 0 {
            "down"
        } else if self.ok == self.total && !self.bulk_blocked {
            "ok"
        } else {
            "partial"
        }
        .into();
        self
    }
}

const PROBE_TIMEOUT_SECS: u64 = 6;
const MAX_PARALLEL: usize = 12;

enum Outcome {
    Http { name: &'static str, ok: bool, ms: u64 },
    Bulk { name: &'static str, state: BulkState },
}

/// Probes every URL target of the given services (and, when `deep`, the bulk
/// freeze check of the flagged ones). Pings are skipped: ICMP is slow and often
/// filtered, and says little about whether HTTPS works.
pub async fn measure(ids: &[&str], deep: bool) -> Vec<ServiceHealth> {
    let payload = if deep { probe::ensure_payload() } else { None };

    struct Job {
        service: usize,
        target: &'static Target,
        kind: JobKind,
    }
    #[derive(Clone, Copy, PartialEq)]
    enum JobKind {
        Http,
        Bulk,
    }

    let selected: Vec<&'static Service> = ids.iter().filter_map(|id| find(id)).collect();
    let mut jobs: Vec<Job> = Vec::new();
    for (si, svc) in selected.iter().enumerate() {
        for target in svc.targets.iter().filter(|t| t.url.is_some()) {
            jobs.push(Job { service: si, target, kind: JobKind::Http });
            if deep && target.bulk && payload.is_some() {
                jobs.push(Job { service: si, target, kind: JobKind::Bulk });
            }
        }
    }

    let mut results: Vec<(usize, Outcome)> = Vec::with_capacity(jobs.len());
    for chunk in jobs.chunks(MAX_PARALLEL) {
        let futures = chunk.iter().map(|job| {
            let payload = payload.clone();
            async move {
                let url = job.target.url.unwrap_or_default();
                let outcome = match job.kind {
                    JobKind::Http => {
                        let p = probe::http(url, PROBE_TIMEOUT_SECS).await;
                        Outcome::Http { name: job.target.name, ok: p.ok, ms: p.millis }
                    }
                    JobKind::Bulk => {
                        let p = probe::bulk(
                            host_of(url),
                            payload.as_deref().unwrap_or_else(|| std::path::Path::new("")),
                            PROBE_TIMEOUT_SECS,
                        )
                        .await;
                        Outcome::Bulk { name: job.target.name, state: p.state }
                    }
                };
                (job.service, outcome)
            }
        });
        results.extend(join_all(futures).await);
    }

    selected
        .iter()
        .enumerate()
        .map(|(si, svc)| {
            let mut health = ServiceHealth {
                id: svc.id.to_string(),
                ok: 0,
                total: 0,
                avg_ms: None,
                bulk_blocked: false,
                failed: Vec::new(),
                state: String::new(),
            };
            let mut ms_sum = 0u64;
            let mut ms_count = 0u64;
            for (_, outcome) in results.iter().filter(|(s, _)| *s == si) {
                health.total += 1;
                match outcome {
                    Outcome::Http { name, ok, ms } => {
                        if *ok {
                            health.ok += 1;
                            ms_sum += ms;
                            ms_count += 1;
                        } else {
                            health.failed.push((*name).to_string());
                        }
                    }
                    Outcome::Bulk { name, state } => match state {
                        BulkState::Ok => health.ok += 1,
                        BulkState::Blocked => {
                            health.bulk_blocked = true;
                            health.failed.push(format!("{name} (bulk)"));
                        }
                        BulkState::Failed => health.failed.push(format!("{name} (bulk)")),
                    },
                }
            }
            if ms_count > 0 {
                health.avg_ms = Some(ms_sum / ms_count);
            }
            health.finish()
        })
        .collect()
}

/// Home-screen "check services" button: light check, no bulk probes.
#[tauri::command]
pub async fn check_services(ids: Vec<String>) -> Vec<ServiceHealth> {
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    let result = measure(&ids, false).await;
    for h in &result {
        crate::logs::append(
            "app",
            &format!(
                "service check {}: {}/{} ok{}{}",
                h.id,
                h.ok,
                h.total,
                h.avg_ms.map(|ms| format!(", avg {ms} ms")).unwrap_or_default(),
                if h.failed.is_empty() { String::new() } else { format!(", failed: {}", h.failed.join(", ")) },
            ),
        );
    }
    result
}
