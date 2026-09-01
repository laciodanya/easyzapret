//! Built-in VPN client (Xray-core) — Happ-style subscriptions & proxy.

mod config;
mod core;
mod parse;
mod store;
mod sysproxy;

use std::collections::HashMap;
use std::net::TcpStream;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::{logs, warp, AppState};
use store::{VpnNode, VpnSettings, VpnState, VpnSubscription};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnStatus {
    pub core_installed: bool,
    pub connected: bool,
    pub mode: String,
    pub selected_node_id: Option<String>,
    pub selected_node_name: Option<String>,
    pub socks_port: u16,
    pub http_port: u16,
    pub node_count: usize,
    pub subscription_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnDetails {
    pub status: VpnStatus,
    pub state: VpnState,
}

fn quick_status_from(state: &VpnState) -> VpnStatus {
    let connected = core::is_running();
    let selected = state
        .selected_node_id
        .as_ref()
        .and_then(|id| store::find_node(state, id));
    VpnStatus {
        core_installed: core::is_core_installed(),
        connected,
        mode: state.settings.mode.clone(),
        selected_node_id: state.selected_node_id.clone(),
        selected_node_name: selected.map(|n| n.name.clone()),
        socks_port: state.settings.socks_port,
        http_port: state.settings.http_port,
        node_count: store::all_nodes(state).len(),
        subscription_count: state.subscriptions.len(),
    }
}

pub fn quick_status() -> VpnStatus {
    quick_status_from(&store::load())
}

pub fn is_connected() -> bool {
    core::is_running()
}

#[tauri::command]
pub fn vpn_details() -> VpnDetails {
    let state = store::load();
    VpnDetails {
        status: quick_status_from(&state),
        state,
    }
}

#[tauri::command]
pub fn vpn_get_settings() -> VpnSettings {
    store::load().settings
}

#[tauri::command]
pub fn vpn_save_settings(settings: VpnSettings) -> Result<VpnSettings, String> {
    let mut state = store::load();
    state.settings = settings;
    store::save(&state)?;
    Ok(state.settings)
}

#[tauri::command]
pub async fn vpn_add_subscription(url: String) -> Result<VpnSubscription, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("vpn_empty_url".into());
    }
    let mut state = store::load();
    // Avoid duplicates
    if let Some(existing) = state.subscriptions.iter().find(|s| s.url == url) {
        return Ok(existing.clone());
    }
    let sub = fetch_subscription(&url).await?;
    state.subscriptions.push(sub.clone());
    store::save(&state)?;
    logs::append("vpn", &format!("Subscription added: {}", sub.name));
    Ok(sub)
}

#[tauri::command]
pub async fn vpn_update_subscription(id: String) -> Result<VpnSubscription, String> {
    let mut state = store::load();
    let idx = state
        .subscriptions
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| "vpn_sub_not_found".to_string())?;
    let url = state.subscriptions[idx].url.clone();
    let old_id = state.subscriptions[idx].id.clone();
    let mut fresh = fetch_subscription(&url).await?;
    fresh.id = old_id;
    // Preserve latency for matching addresses
    let old_lat: HashMap<String, Option<u32>> = state.subscriptions[idx]
        .nodes
        .iter()
        .map(|n| (format!("{}:{}", n.address, n.port), n.latency_ms))
        .collect();
    for n in &mut fresh.nodes {
        let key = format!("{}:{}", n.address, n.port);
        if let Some(ms) = old_lat.get(&key).copied().flatten() {
            n.latency_ms = Some(ms);
        }
        n.subscription_id = Some(fresh.id.clone());
    }
    state.subscriptions[idx] = fresh.clone();
    store::save(&state)?;
    Ok(fresh)
}

#[tauri::command]
pub fn vpn_remove_subscription(id: String) -> Result<(), String> {
    let mut state = store::load();
    let before = state.subscriptions.len();
    state.subscriptions.retain(|s| s.id != id);
    if state.subscriptions.len() == before {
        return Err("vpn_sub_not_found".into());
    }
    if let Some(sel) = state.selected_node_id.clone() {
        if store::find_node(&state, &sel).is_none() {
            state.selected_node_id = None;
        }
    }
    store::save(&state)?;
    Ok(())
}

#[tauri::command]
pub fn vpn_add_node(link: String) -> Result<VpnNode, String> {
    let link = link.trim().to_string();
    let mut node = parse::parse_share_link(&link).ok_or_else(|| "vpn_invalid_link".to_string())?;
    node.subscription_id = None;
    let mut state = store::load();
    state.manual_nodes.push(node.clone());
    if state.selected_node_id.is_none() {
        state.selected_node_id = Some(node.id.clone());
    }
    store::save(&state)?;
    Ok(node)
}

#[tauri::command]
pub fn vpn_remove_node(id: String) -> Result<(), String> {
    let mut state = store::load();
    let mut removed = false;
    let before_manual = state.manual_nodes.len();
    state.manual_nodes.retain(|n| n.id != id);
    if state.manual_nodes.len() != before_manual {
        removed = true;
    }
    for sub in &mut state.subscriptions {
        let before = sub.nodes.len();
        sub.nodes.retain(|n| n.id != id);
        if sub.nodes.len() != before {
            removed = true;
        }
    }
    if !removed {
        return Err("vpn_node_not_found".into());
    }
    if state.selected_node_id.as_deref() == Some(id.as_str()) {
        state.selected_node_id = None;
    }
    store::save(&state)?;
    Ok(())
}

#[tauri::command]
pub fn vpn_select_node(id: String) -> Result<(), String> {
    let mut state = store::load();
    store::find_node(&state, &id).ok_or_else(|| "vpn_node_not_found".to_string())?;
    state.selected_node_id = Some(id);
    store::save(&state)?;
    Ok(())
}

#[tauri::command]
pub fn vpn_ping_nodes(ids: Vec<String>) -> Result<Vec<(String, Option<u32>)>, String> {
    let mut state = store::load();
    let targets: Vec<(String, String, u16)> = {
        let nodes = store::all_nodes(&state);
        let filter: Vec<_> = if ids.is_empty() {
            nodes
        } else {
            nodes.into_iter().filter(|n| ids.contains(&n.id)).collect()
        };
        filter
            .into_iter()
            .map(|n| (n.id, n.address, n.port))
            .collect()
    };

    let mut results = Vec::new();
    for (id, addr, port) in targets {
        let ms = tcp_ping(&addr, port);
        store::set_node_latency(&mut state, &id, ms);
        results.push((id, ms));
    }
    store::save(&state)?;
    Ok(results)
}

fn tcp_ping(addr: &str, port: u16) -> Option<u32> {
    use std::net::ToSocketAddrs;
    let dest = format!("{addr}:{port}");
    let sockaddr = dest.to_socket_addrs().ok()?.next()?;
    let start = Instant::now();
    match TcpStream::connect_timeout(&sockaddr, Duration::from_secs(3)) {
        Ok(_) => Some(start.elapsed().as_millis() as u32),
        Err(_) => None,
    }
}

#[tauri::command]
pub async fn vpn_connect(app: AppHandle, node_id: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<crate::AppState>();
        connect_with_state(&state, node_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn connect_with_state(app: &AppState, node_id: Option<String>) -> Result<(), String> {
    if warp::quick_status().connected {
        return Err("vpn_warp_exclusive".into());
    }
    if !core::is_core_installed() {
        return Err("vpn_core_not_installed".into());
    }

    let mut vpn = store::load();
    let id = node_id
        .or_else(|| vpn.selected_node_id.clone())
        .or_else(|| pick_auto_node(&vpn))
        .ok_or_else(|| "vpn_no_node".to_string())?;

    let node = store::find_node(&vpn, &id)
        .cloned()
        .ok_or_else(|| "vpn_node_not_found".to_string())?;

    sysproxy::disable_system_proxy();

    let preferred = vpn.settings.mode.clone();
    let try_tun = preferred != "system-proxy";
    let settings = vpn.settings.clone();

    let start_with = |mode: &str| -> Result<(), String> {
        let mut s = settings.clone();
        s.mode = mode.to_string();
        let cfg = config::build_config(&node, &s)?;
        let cfg_text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        core::start(&cfg_text)
    };

    let mut used_mode = if try_tun {
        "tun".to_string()
    } else {
        "system-proxy".to_string()
    };

    if try_tun {
        let tun_err = if !core::wintun_available() {
            Some("wintun.dll missing".to_string())
        } else if let Err(e) = start_with("tun") {
            Some(e)
        } else if !core::tun_ready() {
            Some(format!("tun route missing: {}", core::last_log_tail(8)))
        } else {
            None
        };
        if let Some(e) = tun_err {
            logs::append(
                "vpn",
                &format!("TUN unavailable ({e}) — WinINet system proxy"),
            );
            core::stop();
            start_with("system-proxy")?;
            used_mode = "system-proxy".into();
        }
    } else {
        start_with("system-proxy")?;
    }

    if !core::inbound_listening(vpn.settings.http_port) {
        core::stop();
        return Err("vpn_core_exited".into());
    }

    // Happ/v2rayN: TUN and system proxy are independent. Mixing them loops traffic.
    if used_mode == "system-proxy" {
        sysproxy::enable_system_proxy(vpn.settings.http_port, vpn.settings.socks_port)?;
    }

    vpn.selected_node_id = Some(id);
    store::save(&vpn)?;
    app.vpn_active.store(true, std::sync::atomic::Ordering::SeqCst);
    logs::append("vpn", &format!("Connected ({used_mode}) → {}", node.name));
    Ok(())
}

fn pick_auto_node(vpn: &VpnState) -> Option<String> {
    let nodes = store::all_nodes(vpn);
    if nodes.is_empty() {
        return None;
    }
    match vpn.settings.autoconnect_type.as_str() {
        "lowestdelay" => nodes
            .iter()
            .filter(|n| n.latency_ms.is_some())
            .min_by_key(|n| n.latency_ms.unwrap())
            .or(nodes.first())
            .map(|n| n.id.clone()),
        _ => vpn
            .selected_node_id
            .clone()
            .or_else(|| nodes.first().map(|n| n.id.clone())),
    }
}

#[tauri::command]
pub fn vpn_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    disconnect_with_state(&state);
    Ok(())
}

pub fn disconnect_with_state(app: &AppState) {
    app.vpn_active
        .store(false, std::sync::atomic::Ordering::SeqCst);
    sysproxy::disable_system_proxy();
    core::stop();
}

/// Disconnect VPN quietly (app quit / exclusivity).
pub fn disconnect_quiet() {
    sysproxy::disable_system_proxy();
    core::stop();
}

/// If WARP connects while VPN is up, tear VPN down.
pub fn enforce_warp_exclusivity() {
    if warp::quick_status().connected && core::is_running() {
        logs::append("vpn", "WARP connected — disconnecting VPN (exclusive)");
        disconnect_quiet();
    }
}

async fn fetch_subscription(url: &str) -> Result<VpnSubscription, String> {
    let raw = url.trim();
    if raw.is_empty() {
        return Err("vpn_empty_url".into());
    }

    if looks_like_inline_payload(raw) {
        return subscription_from_payload(raw, raw);
    }

    let fetch_url = unwrap_subscription_url(raw)?;
    if looks_like_inline_payload(&fetch_url) {
        return subscription_from_payload(&fetch_url, raw);
    }

    let user_agents = [
        "Happ/3.5.1",
        "Happ/3.4.0",
        "v2rayN/6.55",
        "clash-meta/1.19.0",
        "Mozilla/5.0",
    ];

    let mut last_err = "vpn_empty_subscription".to_string();
    for ua in user_agents {
        match fetch_subscription_with_ua(&fetch_url, ua).await {
            Ok(sub) => {
                let mut sub = sub;
                sub.url = raw.to_string();
                return Ok(sub);
            }
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

fn looks_like_inline_payload(s: &str) -> bool {
    let t = s.to_ascii_lowercase();
    t.contains("vless://")
        || t.contains("vmess://")
        || t.contains("trojan://")
        || t.contains("ss://")
        || t.contains("socks://")
        || t.contains("hysteria2://")
        || t.contains("proxies:")
}

fn unwrap_subscription_url(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("sub://") || lower.starts_with("happ://") {
        let b64 = s.split_once("://").map(|(_, rest)| rest).unwrap_or("");
        if let Some(decoded) = parse::try_b64(b64) {
            let decoded = decoded.trim().to_string();
            if decoded.starts_with("http://") || decoded.starts_with("https://") {
                return Ok(decoded);
            }
            if looks_like_inline_payload(&decoded) {
                return Ok(decoded);
            }
        }
        return Err("vpn_empty_url".into());
    }
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("vpn_empty_url".into());
    }
    Ok(s.to_string())
}

fn subscription_from_payload(payload: &str, url: &str) -> Result<VpnSubscription, String> {
    let (meta, mut nodes) = parse::parse_subscription_body(payload, &HashMap::new());
    if nodes.is_empty() {
        return Err("vpn_empty_subscription".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    for n in &mut nodes {
        n.subscription_id = Some(id.clone());
    }
    Ok(VpnSubscription {
        id,
        url: url.to_string(),
        name: meta.title.unwrap_or_else(|| "Subscription".into()),
        updated_at: Some(Utc::now().to_rfc3339()),
        userinfo: meta.userinfo,
        announce: meta.announce,
        support_url: meta.support_url,
        web_page_url: meta.web_page_url,
        update_interval_hours: meta.update_interval_hours,
        nodes,
    })
}

async fn fetch_subscription_with_ua(url: &str, ua: &str) -> Result<VpnSubscription, String> {
    let client = reqwest::Client::builder()
        .user_agent(ua)
        .timeout(Duration::from_secs(40))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(url)
        .header("Accept", "*/*")
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("subscription HTTP {}", resp.status()));
    }

    let mut headers = HashMap::new();
    for (k, v) in resp.headers() {
        if let Ok(val) = v.to_str() {
            headers.insert(k.as_str().to_string(), val.to_string());
        }
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    let (meta, mut nodes) = parse::parse_subscription_body(&body, &headers);
    if nodes.is_empty() {
        logs::append(
            "vpn",
            &format!("subscription parse empty ({} bytes, ua={ua})", bytes.len()),
        );
        return Err("vpn_empty_subscription".into());
    }

    let id = uuid::Uuid::new_v4().to_string();
    for n in &mut nodes {
        n.subscription_id = Some(id.clone());
    }

    Ok(VpnSubscription {
        id,
        url: url.to_string(),
        name: meta.title.unwrap_or_else(|| "Subscription".into()),
        updated_at: Some(Utc::now().to_rfc3339()),
        userinfo: meta.userinfo,
        announce: meta.announce,
        support_url: meta.support_url,
        web_page_url: meta.web_page_url,
        update_interval_hours: meta.update_interval_hours,
        nodes,
    })
}

/// Used by updates module / status.
#[allow(dead_code)]
pub fn core_installed() -> bool {
    core::is_core_installed()
}
