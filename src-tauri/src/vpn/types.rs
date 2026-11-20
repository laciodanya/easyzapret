use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    All,
    OnlyApps,
    BypassApps,
    Proxy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct VpnSettings {
    pub mode: Mode,
    pub apps: Vec<String>,
    pub bypass_lan: bool,
    pub dns_through_vpn: bool,
    pub dns_server: String,
    pub mtu: u32,
    pub stack: String,
    pub ipv6: bool,
    pub local_port: u16,
    pub sniff: bool,
    pub rules: Vec<RouteRule>,
    pub auto_connect: bool,
}

impl Default for VpnSettings {
    fn default() -> Self {
        Self {
            mode: Mode::OnlyApps,
            apps: Vec::new(),
            bypass_lan: true,
            dns_through_vpn: false,
            dns_server: "1.1.1.1".into(),
            mtu: 1500,
            stack: "mixed".into(),
            ipv6: true,
            local_port: 2080,
            sniff: true,
            rules: Vec::new(),
            auto_connect: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteRule {
    pub kind: String,
    pub values: Vec<String>,
    pub outbound: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub outbound: Value,
    pub subscription_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    pub settings: VpnSettings,
    pub selected_profile: Option<String>,
    pub profiles: Vec<Profile>,
    pub subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub server: String,
    pub subscription_id: Option<String>,
}

impl Profile {
    pub fn server(&self) -> &str {
        if self.engine == "sing-box" {
            self.outbound["server"].as_str().unwrap_or_default()
        } else {
            self.outbound
                .pointer("/settings/vnext/0/address")
                .or_else(|| self.outbound.pointer("/settings/servers/0/address"))
                .and_then(Value::as_str)
                .unwrap_or_default()
        }
    }

    pub fn port(&self) -> u16 {
        let value = if self.engine == "sing-box" {
            &self.outbound["server_port"]
        } else {
            self.outbound
                .pointer("/settings/vnext/0/port")
                .or_else(|| self.outbound.pointer("/settings/servers/0/port"))
                .unwrap_or(&Value::Null)
        };
        value.as_u64().unwrap_or_default() as u16
    }

    pub fn info(&self) -> ProfileInfo {
        ProfileInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            protocol: self.outbound[if self.engine == "sing-box" {
                "type"
            } else {
                "protocol"
            }]
            .as_str()
            .unwrap_or_default()
            .into(),
            server: self.server().into(),
            subscription_id: self.subscription_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnStatus {
    pub installed: bool,
    pub running: bool,
    pub mode: Mode,
    pub active_profile: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnDetails {
    pub status: VpnStatus,
    pub settings: VpnSettings,
    pub selected_profile: Option<String>,
    pub profiles: Vec<ProfileInfo>,
    pub subscriptions: Vec<SubscriptionInfo>,
}

pub fn new_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}
