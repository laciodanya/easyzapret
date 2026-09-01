//! Subscription and share-link parsers (vless / vmess / trojan / ss / socks / hy2).

use std::collections::HashMap;

use base64::{engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::store::{VpnNode, VpnUserInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedSubMeta {
    pub title: Option<String>,
    pub userinfo: Option<VpnUserInfo>,
    pub announce: Option<String>,
    pub support_url: Option<String>,
    pub web_page_url: Option<String>,
    pub update_interval_hours: Option<u32>,
}

pub fn parse_subscription_body(body: &str, headers: &HashMap<String, String>) -> (ParsedSubMeta, Vec<VpnNode>) {
    let mut meta = ParsedSubMeta {
        title: header_or_body(headers, body, &["profile-title", "Profile-Title"]),
        userinfo: parse_userinfo(
            headers
                .get("subscription-userinfo")
                .or_else(|| headers.get("Subscription-Userinfo"))
                .cloned()
                .or_else(|| body_meta(body, "subscription-userinfo")),
        ),
        announce: header_or_body(headers, body, &["announce", "Announce"]),
        support_url: header_or_body(headers, body, &["support-url", "Support-Url", "support-url"]),
        web_page_url: header_or_body(
            headers,
            body,
            &["profile-web-page-url", "Profile-Web-Page-Url"],
        ),
        update_interval_hours: header_or_body(headers, body, &["profile-update-interval", "Profile-Update-Interval"])
            .and_then(|s| s.parse().ok()),
    };

    if let Some(title) = meta.title.clone() {
        meta.title = Some(decode_maybe_base64(&title));
    }
    if let Some(announce) = meta.announce.clone() {
        meta.announce = Some(decode_maybe_base64(&announce));
    }

    let text = decode_subscription_payload(body);
    let mut nodes = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(node) = parse_share_link(line) {
            nodes.push(node);
        } else if line.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                if let Some(node) = parse_json_node(&v) {
                    nodes.push(node);
                }
            }
        }
    }

    // JSON array body
    if nodes.is_empty() {
        if let Ok(arr) = serde_json::from_str::<Vec<Value>>(text.trim()) {
            for v in arr {
                if let Some(node) = parse_json_node(&v) {
                    nodes.push(node);
                }
            }
        }
    }

    (meta, nodes)
}

fn header_or_body(headers: &HashMap<String, String>, body: &str, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = headers.get(*k) {
            if !v.is_empty() {
                return Some(v.clone());
            }
        }
        // case-insensitive header lookup
        for (hk, hv) in headers {
            if hk.eq_ignore_ascii_case(k) && !hv.is_empty() {
                return Some(hv.clone());
            }
        }
    }
    for k in keys {
        if let Some(v) = body_meta(body, k) {
            return Some(v);
        }
    }
    None
}

fn body_meta(body: &str, key: &str) -> Option<String> {
    let needle = format!("#{key}:");
    let needle2 = format!("#{key}=");
    for line in body.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix(&needle).or_else(|| t.strip_prefix(&needle2)) {
            return Some(rest.trim().to_string());
        }
        // also accept without leading #
        let plain = format!("{key}:");
        if let Some(rest) = t.strip_prefix(&plain) {
            if t.starts_with('#') || !t.contains("://") {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

fn parse_userinfo(raw: Option<String>) -> Option<VpnUserInfo> {
    let raw = raw?;
    let mut info = VpnUserInfo::default();
    for part in raw.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            match k.trim() {
                "upload" => info.upload = v.trim().parse().ok(),
                "download" => info.download = v.trim().parse().ok(),
                "total" => info.total = v.trim().parse().ok(),
                "expire" => info.expire = v.trim().parse().ok(),
                _ => {}
            }
        }
    }
    Some(info)
}

fn decode_subscription_payload(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.contains("://") || trimmed.starts_with('{') || trimmed.starts_with('[') || trimmed.starts_with('#') {
        return body.to_string();
    }
    if let Some(decoded) = try_b64(trimmed) {
        if decoded.contains("://") || decoded.starts_with('{') || decoded.starts_with('[') {
            return decoded;
        }
    }
    body.to_string()
}

fn try_b64(s: &str) -> Option<String> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    for eng in [&STANDARD, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = eng.decode(&cleaned) {
            if let Ok(text) = String::from_utf8(bytes) {
                return Some(text);
            }
        }
        // pad
        let mut padded = cleaned.clone();
        while padded.len() % 4 != 0 {
            padded.push('=');
        }
        if let Ok(bytes) = eng.decode(&padded) {
            if let Ok(text) = String::from_utf8(bytes) {
                return Some(text);
            }
        }
    }
    None
}

fn decode_maybe_base64(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("base64:") {
        return try_b64(rest).unwrap_or_else(|| rest.to_string());
    }
    try_b64(t).unwrap_or_else(|| t.to_string())
}

pub fn parse_share_link(link: &str) -> Option<VpnNode> {
    let link = link.trim();
    let lower = link.to_ascii_lowercase();
    if lower.starts_with("vless://") {
        return parse_vless(link);
    }
    if lower.starts_with("vmess://") {
        return parse_vmess(link);
    }
    if lower.starts_with("trojan://") {
        return parse_trojan(link);
    }
    if lower.starts_with("ss://") {
        return parse_ss(link);
    }
    if lower.starts_with("socks://") || lower.starts_with("socks5://") {
        return parse_socks(link);
    }
    if lower.starts_with("hysteria2://") || lower.starts_with("hy2://") {
        return parse_hy2(link);
    }
    None
}

fn new_node(protocol: &str, name: &str, address: &str, port: u16, raw: &str, params: Value) -> VpnNode {
    VpnNode {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        protocol: protocol.to_string(),
        address: address.to_string(),
        port,
        raw: raw.to_string(),
        latency_ms: None,
        subscription_id: None,
        params,
    }
}

fn split_name(fragment: &str) -> String {
    let name = urlencoding_decode(fragment);
    if let Some((title, _)) = name.split_once('?') {
        title.to_string()
    } else {
        name
    }
}

fn urlencoding_decode(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v as char);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
}

fn parse_query(q: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for part in q.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            map.insert(k.to_string(), urlencoding_decode(v));
        } else if !part.is_empty() {
            map.insert(part.to_string(), String::new());
        }
    }
    map
}

fn parse_host_port(authority: &str) -> Option<(String, u16)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, port_part) = rest.split_once("]:")?;
        return Some((host.to_string(), port_part.parse().ok()?));
    }
    let (host, port) = authority.rsplit_once(':')?;
    Some((host.to_string(), port.parse().ok()?))
}

fn parse_vless(link: &str) -> Option<VpnNode> {
    let rest = link.strip_prefix("vless://").or_else(|| link.strip_prefix("VLESS://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "VLESS"));
    let (userinfo, hostport_q) = main.split_once('@')?;
    let (hostport, query) = hostport_q.split_once('?').unwrap_or((hostport_q, ""));
    let (address, port) = parse_host_port(hostport)?;
    let q = parse_query(query);
    let name = {
        let n = split_name(fragment);
        if n.is_empty() {
            format!("{address}:{port}")
        } else {
            n
        }
    };
    let mut params = serde_json::Map::new();
    params.insert("id".into(), Value::String(userinfo.to_string()));
    for (k, v) in q {
        params.insert(k, Value::String(v));
    }
    Some(new_node("vless", &name, &address, port, link, Value::Object(params)))
}

fn parse_vmess(link: &str) -> Option<VpnNode> {
    let rest = link.strip_prefix("vmess://").or_else(|| link.strip_prefix("VMESS://"))?;
    let json_text = try_b64(rest)?;
    let v: Value = serde_json::from_str(&json_text).ok()?;
    let address = v.get("add").and_then(|x| x.as_str())?.to_string();
    let port = v
        .get("port")
        .and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(443) as u16;
    let name = v
        .get("ps")
        .and_then(|x| x.as_str())
        .unwrap_or("VMess")
        .to_string();
    Some(new_node("vmess", &name, &address, port, link, v))
}

fn parse_trojan(link: &str) -> Option<VpnNode> {
    let rest = link.strip_prefix("trojan://").or_else(|| link.strip_prefix("TROJAN://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "Trojan"));
    let (password, hostport_q) = main.split_once('@')?;
    let (hostport, query) = hostport_q.split_once('?').unwrap_or((hostport_q, ""));
    let (address, port) = parse_host_port(hostport)?;
    let q = parse_query(query);
    let name = {
        let n = split_name(fragment);
        if n.is_empty() {
            format!("{address}:{port}")
        } else {
            n
        }
    };
    let mut params = serde_json::Map::new();
    params.insert("password".into(), Value::String(password.to_string()));
    for (k, v) in q {
        params.insert(k, Value::String(v));
    }
    Some(new_node("trojan", &name, &address, port, link, Value::Object(params)))
}

fn parse_ss(link: &str) -> Option<VpnNode> {
    let rest = link.strip_prefix("ss://").or_else(|| link.strip_prefix("SS://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "Shadowsocks"));
    let name = {
        let n = split_name(fragment);
        if n.is_empty() {
            "Shadowsocks".into()
        } else {
            n
        }
    };

    // ss://base64(method:password@host:port) or ss://base64(method:password)@host:port
    if let Some((user_b64, hostport)) = main.split_once('@') {
        let (address, port) = parse_host_port(hostport)?;
        let decoded = try_b64(user_b64).unwrap_or_else(|| user_b64.to_string());
        let (method, password) = decoded.split_once(':')?;
        let params = serde_json::json!({
            "method": method,
            "password": password,
        });
        return Some(new_node("shadowsocks", &name, &address, port, link, params));
    }

    let decoded = try_b64(main)?;
    let (method_pass, hostport) = decoded.split_once('@')?;
    let (method, password) = method_pass.split_once(':')?;
    let (address, port) = parse_host_port(hostport)?;
    let params = serde_json::json!({
        "method": method,
        "password": password,
    });
    Some(new_node("shadowsocks", &name, &address, port, link, params))
}

fn parse_socks(link: &str) -> Option<VpnNode> {
    let rest = link
        .strip_prefix("socks://")
        .or_else(|| link.strip_prefix("socks5://"))
        .or_else(|| link.strip_prefix("SOCKS://"))
        .or_else(|| link.strip_prefix("SOCKS5://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "SOCKS"));
    let name = {
        let n = split_name(fragment);
        if n.is_empty() {
            "SOCKS".into()
        } else {
            n
        }
    };
    parse_socks_authority(main, &name, link)
}

fn parse_socks_authority(main: &str, name: &str, link: &str) -> Option<VpnNode> {
    let main = try_b64(main).unwrap_or_else(|| main.to_string());
    let (user, pass, hostport) = if let Some((creds, hp)) = main.split_once('@') {
        if let Some((u, p)) = creds.split_once(':') {
            (Some(u.to_string()), Some(p.to_string()), hp.to_string())
        } else {
            (Some(creds.to_string()), None, hp.to_string())
        }
    } else {
        (None, None, main)
    };
    let (address, port) = parse_host_port(&hostport)?;
    let params = serde_json::json!({
        "user": user,
        "pass": pass,
    });
    Some(new_node("socks", name, &address, port, link, params))
}

fn parse_hy2(link: &str) -> Option<VpnNode> {
    let rest = link
        .strip_prefix("hysteria2://")
        .or_else(|| link.strip_prefix("hy2://"))
        .or_else(|| link.strip_prefix("HYSTERIA2://"))
        .or_else(|| link.strip_prefix("HY2://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "Hysteria2"));
    let (auth, hostport_q) = if let Some((a, h)) = main.split_once('@') {
        (a.to_string(), h)
    } else {
        (String::new(), main)
    };
    let (hostport, query) = hostport_q.split_once('?').unwrap_or((hostport_q, ""));
    let (address, port) = parse_host_port(hostport)?;
    let q = parse_query(query);
    let name = {
        let n = split_name(fragment);
        if n.is_empty() {
            format!("{address}:{port}")
        } else {
            n
        }
    };
    let mut params = serde_json::Map::new();
    params.insert("auth".into(), Value::String(auth));
    for (k, v) in q {
        params.insert(k, Value::String(v));
    }
    Some(new_node("hysteria2", &name, &address, port, link, Value::Object(params)))
}

fn parse_json_node(v: &Value) -> Option<VpnNode> {
    // Xray outbound style or Clash-ish
    if let Some(proto) = v.get("protocol").and_then(|x| x.as_str()) {
        let name = v
            .get("remarks")
            .or_else(|| v.get("tag"))
            .and_then(|x| x.as_str())
            .unwrap_or(proto)
            .to_string();
        let address = v
            .pointer("/settings/vnext/0/address")
            .or_else(|| v.pointer("/settings/servers/0/address"))
            .and_then(|x| x.as_str())
            .unwrap_or("0.0.0.0")
            .to_string();
        let port = v
            .pointer("/settings/vnext/0/port")
            .or_else(|| v.pointer("/settings/servers/0/port"))
            .and_then(|x| x.as_u64())
            .unwrap_or(443) as u16;
        let raw = serde_json::to_string(v).unwrap_or_default();
        return Some(new_node(proto, &name, &address, port, &raw, v.clone()));
    }
    // share-link inside JSON
    if let Some(link) = v.get("url").and_then(|x| x.as_str()).or_else(|| v.get("link").and_then(|x| x.as_str())) {
        return parse_share_link(link);
    }
    None
}
