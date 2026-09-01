//! Persistent VPN state: subscriptions, nodes, settings.

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VpnUserInfo {
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    /// Unix timestamp seconds
    pub expire: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnNode {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub raw: String,
    pub latency_ms: Option<u32>,
    pub subscription_id: Option<String>,
    /// Protocol-specific fields used when building xray outbound.
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnSubscription {
    pub id: String,
    pub url: String,
    pub name: String,
    pub updated_at: Option<String>,
    pub userinfo: Option<VpnUserInfo>,
    pub announce: Option<String>,
    pub support_url: Option<String>,
    pub web_page_url: Option<String>,
    pub update_interval_hours: Option<u32>,
    pub nodes: Vec<VpnNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VpnSettings {
    /// system-proxy | tun (tun reserved / experimental)
    pub mode: String,
    pub socks_port: u16,
    pub http_port: u16,
    pub mux_enabled: bool,
    pub mux_concurrency: u32,
    pub sniffing: bool,
    pub allow_insecure: bool,
    pub dns: String,
    pub bypass_lan: bool,
    pub bypass_private: bool,
    pub fragmentation: bool,
    pub fragmentation_packets: String,
    pub fragmentation_length: String,
    pub fragmentation_interval: String,
    pub auto_connect: bool,
    pub autoconnect_type: String,
    pub auto_update_subs: bool,
    pub update_on_open: bool,
    pub routing_enabled: bool,
    /// last-used | lowest-delay | manual
    pub select_strategy: String,
}

impl Default for VpnSettings {
    fn default() -> Self {
        Self {
            mode: "system-proxy".into(),
            socks_port: 10808,
            http_port: 10809,
            mux_enabled: false,
            mux_concurrency: 8,
            sniffing: true,
            allow_insecure: false,
            dns: "1.1.1.1".into(),
            bypass_lan: true,
            bypass_private: true,
            fragmentation: false,
            fragmentation_packets: "tlshello".into(),
            fragmentation_length: "50-100".into(),
            fragmentation_interval: "10-20".into(),
            auto_connect: false,
            autoconnect_type: "lastused".into(),
            auto_update_subs: true,
            update_on_open: false,
            routing_enabled: true,
            select_strategy: "manual".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VpnState {
    pub subscriptions: Vec<VpnSubscription>,
    pub manual_nodes: Vec<VpnNode>,
    pub selected_node_id: Option<String>,
    pub settings: VpnSettings,
}

pub fn load() -> VpnState {
    let path = paths::vpn_state_file();
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => VpnState::default(),
    }
}

pub fn save(state: &VpnState) -> Result<(), String> {
    paths::ensure_dirs().map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(paths::vpn_state_file(), text).map_err(|e| e.to_string())
}

pub fn find_node<'a>(state: &'a VpnState, id: &str) -> Option<&'a VpnNode> {
    state
        .manual_nodes
        .iter()
        .find(|n| n.id == id)
        .or_else(|| {
            state
                .subscriptions
                .iter()
                .flat_map(|s| s.nodes.iter())
                .find(|n| n.id == id)
        })
}

pub fn all_nodes(state: &VpnState) -> Vec<VpnNode> {
    let mut out = state.manual_nodes.clone();
    for sub in &state.subscriptions {
        out.extend(sub.nodes.clone());
    }
    out
}

pub fn set_node_latency(state: &mut VpnState, id: &str, ms: Option<u32>) {
    for n in &mut state.manual_nodes {
        if n.id == id {
            n.latency_ms = ms;
            return;
        }
    }
    for sub in &mut state.subscriptions {
        for n in &mut sub.nodes {
            if n.id == id {
                n.latency_ms = ms;
                return;
            }
        }
    }
}
