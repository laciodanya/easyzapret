use serde_json::{json, Value};
use std::net::IpAddr;

use super::import::validate_profile;
use super::types::{Mode, Profile, VpnSettings};

pub const TUN_NAME: &str = "EasyZapretVPN";
pub const TUN_V4: &str = "172.29.231.1";
pub const TUN_V6: &str = "fdfe:ea:5a::1";

pub fn validate_settings(settings: &VpnSettings) -> Result<(), String> {
    if !(1280..=9000).contains(&settings.mtu)
        || !(1024..=65533).contains(&settings.local_port)
        || !matches!(settings.stack.as_str(), "system" | "gvisor" | "mixed")
        || settings.apps.len() > 256
        || settings.rules.len() > 256
    {
        return Err("vpn_invalid_settings".into());
    }
    dns_server(&settings.dns_server, settings.dns_through_vpn)?;
    for app in &settings.apps {
        if app.is_empty()
            || app.len() > 1024
            || !app.to_ascii_lowercase().ends_with(".exe")
            || app.contains(['\n', '\r', '\0', '*', '?'])
        {
            return Err("vpn_invalid_application".into());
        }
    }
    for rule in &settings.rules {
        if !matches!(rule.outbound.as_str(), "proxy" | "direct" | "reject")
            || !matches!(
                rule.kind.as_str(),
                "domain"
                    | "domain_suffix"
                    | "domain_keyword"
                    | "domain_regex"
                    | "ip_cidr"
                    | "process_name"
                    | "process_path"
            )
            || rule.values.is_empty()
            || rule.values.len() > 256
        {
            return Err("vpn_invalid_rule".into());
        }
        for value in &rule.values {
            if value.is_empty() || value.len() > 1024 || value.contains(['\n', '\r', '\0']) {
                return Err("vpn_invalid_rule".into());
            }
            if rule.kind == "ip_cidr" {
                let (ip, prefix) = value.split_once('/').ok_or("vpn_invalid_cidr")?;
                let ip: IpAddr = ip.parse().map_err(|_| "vpn_invalid_cidr")?;
                let prefix: u8 = prefix.parse().map_err(|_| "vpn_invalid_cidr")?;
                if prefix > if ip.is_ipv4() { 32 } else { 128 } {
                    return Err("vpn_invalid_cidr".into());
                }
            }
        }
    }
    Ok(())
}

fn dns_server(raw: &str, through_vpn: bool) -> Result<Value, String> {
    let mut server = if let Ok(ip) = raw.parse::<IpAddr>() {
        json!({"type":"udp","server":ip.to_string()})
    } else {
        let url = reqwest::Url::parse(raw).map_err(|_| "vpn_invalid_dns")?;
        let host = url
            .host_str()
            .ok_or("vpn_invalid_dns")?
            .trim_matches(['[', ']']);
        if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
            return Err("vpn_invalid_dns".into());
        }
        match url.scheme() {
            "https" => json!({"type":"https","server":host,"server_port":url.port().unwrap_or(443),
                "path":if url.path() == "/" {"/dns-query"} else {url.path()},"tls":{"enabled":true,"server_name":host}}),
            "tls" => json!({"type":"tls","server":host,"server_port":url.port().unwrap_or(853),
                "tls":{"enabled":true,"server_name":host}}),
            "udp" => json!({"type":"udp","server":host,"server_port":url.port().unwrap_or(53)}),
            _ => return Err("vpn_invalid_dns".into()),
        }
    };
    server["tag"] = json!("dns-selected");
    server["detour"] = json!(if through_vpn { "proxy" } else { "direct" });
    server["domain_resolver"] = json!("dns-bootstrap");
    Ok(server)
}

fn app_match(apps: &[String]) -> Value {
    let (paths, names): (Vec<_>, Vec<_>) = apps
        .iter()
        .cloned()
        .partition(|app| app.contains(['\\', '/']));
    let mut rules = Vec::new();
    if !names.is_empty() {
        rules.push(json!({"process_name":names}));
    }
    if !paths.is_empty() {
        rules.push(json!({"process_path":paths}));
    }
    json!({"type":"logical","mode":"or","rules":rules})
}

pub fn build(settings: &VpnSettings, profile: &Profile) -> Result<Value, String> {
    validate_settings(settings)?;
    validate_profile(profile)?;
    if matches!(settings.mode, Mode::OnlyApps | Mode::BypassApps) && settings.apps.is_empty() {
        return Err("vpn_select_apps".into());
    }
    let mut inbounds = vec![json!({"type":"mixed","tag":"mixed-in","listen":"127.0.0.1",
        "listen_port":settings.local_port})];
    if settings.mode != Mode::Proxy {
        // Both address families are captured. Disabling IPv6 rejects it for VPN
        // traffic after the per-app gate, preventing IPv6 from escaping the tunnel.
        let mut tun = json!({"type":"tun","tag":"tun-in","interface_name":TUN_NAME,
            "address":[format!("{TUN_V4}/30"),format!("{TUN_V6}/126")],
            "mtu":settings.mtu,"auto_route":true,"strict_route":false,"stack":settings.stack});
        if settings.bypass_lan {
            tun["route_exclude_address"] = json!([
                "127.0.0.0/8",
                "10.0.0.0/8",
                "172.16.0.0/12",
                "192.168.0.0/16",
                "169.254.0.0/16",
                "224.0.0.0/4",
                "::1/128",
                "fc00::/7",
                "fe80::/10",
                "ff00::/8"
            ]);
        }
        inbounds.push(tun);
    }
    let proxy = if profile.engine == "sing-box" {
        let mut proxy = profile.outbound.clone();
        proxy["tag"] = json!("proxy");
        proxy["domain_resolver"] = json!("dns-bootstrap");
        proxy
    } else {
        json!({"type":"socks","tag":"proxy","server":"127.0.0.1",
        "server_port":settings.local_port+1,"version":"5"})
    };
    let mut rules = vec![
        json!({"inbound":["tun-in"],"process_name":["EasyZapret.exe","easyzapret.exe","sing-box.exe",
            "xray.exe","winws.exe","TgWsProxy_windows.exe"],"action":"route","outbound":"direct"}),
        json!({"port":53,"action":"hijack-dns"}),
    ];
    if settings.sniff {
        rules.push(json!({"action":"sniff"}));
    }
    if settings.bypass_lan {
        rules.push(json!({"ip_is_private":true,"action":"route","outbound":"direct"}));
    }
    // An explicitly configured local HTTP/SOCKS client always uses the selected server.
    rules.push(json!({"inbound":["mixed-in"],"action":"route","outbound":"proxy"}));
    if matches!(settings.mode, Mode::OnlyApps | Mode::BypassApps) {
        let mut gate = app_match(&settings.apps);
        gate["invert"] = json!(settings.mode == Mode::OnlyApps);
        gate["action"] = json!("route");
        gate["outbound"] = json!("direct");
        rules.push(gate);
    }
    if !settings.ipv6 {
        rules.push(json!({"ip_version":6,"action":"reject"}));
    }
    for rule in &settings.rules {
        let mut value = json!({});
        value[&rule.kind] = json!(rule.values);
        if rule.outbound == "reject" {
            value["action"] = json!("reject");
        } else {
            value["action"] = json!("route");
            value["outbound"] = json!(rule.outbound);
        }
        rules.push(value);
    }
    Ok(json!({"log":{"disabled":true},
        "dns":{"servers":[{"type":"udp","tag":"dns-bootstrap","server":"1.1.1.1","detour":"direct"},
            dns_server(&settings.dns_server,settings.dns_through_vpn)?],"final":"dns-selected",
            "strategy":if settings.ipv6 {"prefer_ipv4"} else {"ipv4_only"}},
        "inbounds":inbounds,"outbounds":[proxy,{"type":"direct","tag":"direct"}],
        "route":{"rules":rules,"final":"proxy","auto_detect_interface":true,"find_process":true,
            "default_domain_resolver":"dns-bootstrap"}}))
}

pub fn xray_config(settings: &VpnSettings, profile: &Profile) -> Value {
    json!({"log":{"loglevel":"none"},"inbounds":[{"listen":"127.0.0.1","port":settings.local_port+1,
        "protocol":"socks","settings":{"auth":"noauth","udp":true}}],"outbounds":[profile.outbound]})
}
