//! Subscription and share-link parsers (vless / vmess / trojan / ss / socks / hy2 / wireguard).

use std::collections::HashMap;

use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
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
                .cloned()
                .or_else(|| header_ci(headers, "subscription-userinfo"))
                .or_else(|| body_meta(body, "subscription-userinfo")),
        ),
        announce: header_or_body(headers, body, &["announce", "Announce"]),
        support_url: header_or_body(headers, body, &["support-url", "Support-Url"]),
        web_page_url: header_or_body(headers, body, &["profile-web-page-url", "Profile-Web-Page-Url"]),
        update_interval_hours: header_or_body(
            headers,
            body,
            &["profile-update-interval", "Profile-Update-Interval"],
        )
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

fn header_ci(headers: &HashMap<String, String>, key: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.clone())
        .filter(|v| !v.is_empty())
}

fn header_or_body(headers: &HashMap<String, String>, body: &str, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = headers.get(*k).cloned().filter(|s| !s.is_empty()) {
            return Some(v);
        }
        if let Some(v) = header_ci(headers, k) {
            return Some(v);
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
    if trimmed.contains("://") || trimmed.starts_with('{') || trimmed.starts_with('[') || trimmed.starts_with('#')
    {
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
    for eng in [&STANDARD, &URL_SAFE_NO_PAD, &URL_SAFE] {
        let mut padded = cleaned.clone();
        while padded.len() % 4 != 0 {
            padded.push('=');
        }
        if let Ok(bytes) = eng.decode(&padded) {
            return Some(String::from_utf8_lossy(&bytes).into_owned());
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
    if lower.starts_with("wireguard://") {
        return parse_wireguard(link);
    }
    None
}

fn new_node(protocol: &str, name: &str, address: &str, port: u16, raw: &str, params: Value) -> VpnNode {
    VpnNode {
        id: uuid::Uuid::new_v4().to_string(),
        name: clean_display_name(name),
        protocol: protocol.to_string(),
        address: address.to_string(),
        port,
        raw: raw.to_string(),
        latency_ms: None,
        subscription_id: None,
        params,
    }
}

pub fn clean_display_name(raw: &str) -> String {
    let decoded = repair_utf8_mojibake(&percent_decode_utf8(raw));
    let title = decoded.split('?').next().unwrap_or(&decoded);
    let title = title.replace(['\u{feff}', '\u{200b}'], "");
    title.trim().to_string()
}

/// If UTF-8 was decoded as Latin-1 (`ðŸ‡©` instead of 🇩🇪), restore it.
pub fn repair_utf8_mojibake(s: &str) -> String {
    if !s.chars().any(|c| ('\u{0080}'..='\u{00FF}').contains(&c)) {
        return s.to_string();
    }
    let bytes: Vec<u8> = s.chars().map(|c| (c as u32).min(0xff) as u8).collect();
    match String::from_utf8(bytes) {
        Ok(fixed) if fixed != s => fixed,
        _ => s.to_string(),
    }
}

/// Decode percent-encoding as UTF-8 bytes (flags, Cyrillic, CJK).
pub fn percent_decode_utf8(s: &str) -> String {
    let raw = s.as_bytes();
    let mut bytes = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let Ok(hex) = std::str::from_utf8(&raw[i + 1..i + 3]) {
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    bytes.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        bytes.push(if raw[i] == b'+' { b' ' } else { raw[i] });
        i += 1;
    }
    String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn parse_query(q: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for part in q.split('&') {
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = part.split_once('=') {
            map.insert(k.to_ascii_lowercase(), percent_decode_utf8(v));
        } else {
            map.insert(part.to_ascii_lowercase(), String::new());
        }
    }
    map
}

fn parse_host_port(authority: &str) -> Option<(String, u16)> {
    let authority = percent_decode_utf8(authority);
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, port_part) = rest.split_once("]:")?;
        return Some((host.to_string(), port_part.parse().ok()?));
    }
    let (host, port) = authority.rsplit_once(':')?;
    Some((host.to_string(), port.parse().ok()?))
}

fn parse_vless(link: &str) -> Option<VpnNode> {
    let rest = link
        .strip_prefix("vless://")
        .or_else(|| link.strip_prefix("VLESS://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "VLESS"));
    let (userinfo, hostport_q) = main.split_once('@')?;
    let (hostport, query) = hostport_q.split_once('?').unwrap_or((hostport_q, ""));
    let (address, port) = parse_host_port(hostport)?;
    let q = parse_query(query);
    let name = {
        let n = clean_display_name(fragment);
        if n.is_empty() {
            format!("{address}:{port}")
        } else {
            n
        }
    };
    let mut params = serde_json::Map::new();
    params.insert("id".into(), Value::String(percent_decode_utf8(userinfo)));
    for (k, v) in q {
        params.insert(k, Value::String(v));
    }
    Some(new_node(
        "vless",
        &name,
        &address,
        port,
        link,
        Value::Object(params),
    ))
}

fn parse_vmess(link: &str) -> Option<VpnNode> {
    let rest = link
        .strip_prefix("vmess://")
        .or_else(|| link.strip_prefix("VMESS://"))?;
    let (payload, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let json_text = try_b64(payload)?;
    let v: Value = serde_json::from_str(&json_text).ok()?;
    let address = v.get("add").and_then(|x| x.as_str())?.to_string();
    let port = v
        .get("port")
        .and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(443) as u16;
    let name = if !fragment.is_empty() {
        clean_display_name(fragment)
    } else {
        v.get("ps")
            .and_then(|x| x.as_str())
            .map(clean_display_name)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "VMess".into())
    };
    Some(new_node("vmess", &name, &address, port, link, v))
}

fn parse_trojan(link: &str) -> Option<VpnNode> {
    let rest = link
        .strip_prefix("trojan://")
        .or_else(|| link.strip_prefix("TROJAN://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "Trojan"));
    let (password, hostport_q) = main.split_once('@')?;
    let (hostport, query) = hostport_q.split_once('?').unwrap_or((hostport_q, ""));
    let (address, port) = parse_host_port(hostport)?;
    let q = parse_query(query);
    let name = {
        let n = clean_display_name(fragment);
        if n.is_empty() {
            format!("{address}:{port}")
        } else {
            n
        }
    };
    let mut params = serde_json::Map::new();
    params.insert("password".into(), Value::String(percent_decode_utf8(password)));
    for (k, v) in q {
        params.insert(k, Value::String(v));
    }
    Some(new_node(
        "trojan",
        &name,
        &address,
        port,
        link,
        Value::Object(params),
    ))
}

fn parse_ss(link: &str) -> Option<VpnNode> {
    let rest = link.strip_prefix("ss://").or_else(|| link.strip_prefix("SS://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "Shadowsocks"));
    let name = {
        let n = clean_display_name(fragment);
        if n.is_empty() {
            "Shadowsocks".into()
        } else {
            n
        }
    };

    if let Some((user_b64, hostport_q)) = main.split_once('@') {
        let (hostport, query) = hostport_q.split_once('?').unwrap_or((hostport_q, ""));
        let (address, port) = parse_host_port(hostport)?;
        let decoded = try_b64(user_b64).unwrap_or_else(|| percent_decode_utf8(user_b64));
        let (method, password) = decoded.split_once(':')?;
        let mut params = serde_json::Map::new();
        params.insert("method".into(), Value::String(method.to_string()));
        params.insert("password".into(), Value::String(password.to_string()));
        for (k, v) in parse_query(query) {
            params.insert(k, Value::String(v));
        }
        return Some(new_node(
            "shadowsocks",
            &name,
            &address,
            port,
            link,
            Value::Object(params),
        ));
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
        let n = clean_display_name(fragment);
        if n.is_empty() {
            "SOCKS".into()
        } else {
            n
        }
    };
    parse_socks_authority(main, &name, link)
}

fn parse_socks_authority(main: &str, name: &str, link: &str) -> Option<VpnNode> {
    let main = try_b64(main).unwrap_or_else(|| percent_decode_utf8(main));
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
        (percent_decode_utf8(a), h)
    } else {
        (String::new(), main)
    };
    let (hostport, query) = hostport_q.split_once('?').unwrap_or((hostport_q, ""));
    let (address, port) = parse_host_port(hostport)?;
    let q = parse_query(query);
    let name = {
        let n = clean_display_name(fragment);
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
    Some(new_node(
        "hysteria2",
        &name,
        &address,
        port,
        link,
        Value::Object(params),
    ))
}

fn parse_wireguard(link: &str) -> Option<VpnNode> {
    let rest = link
        .strip_prefix("wireguard://")
        .or_else(|| link.strip_prefix("WIREGUARD://"))?;
    let (main, fragment) = rest.split_once('#').unwrap_or((rest, "WireGuard"));
    let (secret, hostport_q) = main.split_once('@')?;
    let (hostport, query) = hostport_q.split_once('?').unwrap_or((hostport_q, ""));
    let (address, port) = parse_host_port(hostport)?;
    let q = parse_query(query);
    let name = {
        let n = clean_display_name(fragment);
        if n.is_empty() {
            format!("{address}:{port}")
        } else {
            n
        }
    };
    let mut params = serde_json::Map::new();
    params.insert("secretKey".into(), Value::String(percent_decode_utf8(secret)));
    for (k, v) in q {
        params.insert(k, Value::String(v));
    }
    Some(new_node(
        "wireguard",
        &name,
        &address,
        port,
        link,
        Value::Object(params),
    ))
}

fn parse_json_node(v: &Value) -> Option<VpnNode> {
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
    if let Some(link) = v
        .get("url")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("link").and_then(|x| x.as_str()))
    {
        return parse_share_link(link);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_keeps_flag_emoji() {
        // 🇩🇪 as UTF-8 percent-encoding
        let encoded = "%F0%9F%87%A9%F0%9F%87%AA%20Germany";
        let out = percent_decode_utf8(encoded);
        assert!(out.contains("Germany"), "{out}");
        assert!(out.contains('\u{1f1e9}'), "missing regional indicator: {out:?}");
        assert!(!out.contains('ð'), "mojibake in {out:?}");
    }

    #[test]
    fn vless_grpc_reality_parses() {
        let link = "vless://11111111-1111-1111-1111-111111111111@de.example.com:443?encryption=none&security=reality&sni=www.microsoft.com&fp=chrome&pbk=PUBLICKEY&sid=abcd1234&type=grpc&serviceName=grpc&flow=xtls-rprx-vision#%F0%9F%87%A9%20DE-1";
        let node = parse_share_link(link).expect("parse");
        assert_eq!(node.protocol, "vless");
        assert_eq!(node.address, "de.example.com");
        assert_eq!(node.port, 443);
        assert_eq!(node.params["type"], "grpc");
        assert_eq!(node.params["security"], "reality");
        assert_eq!(node.params["servicename"], "grpc");
        assert!(node.name.contains("DE-1"), "{}", node.name);
        assert!(!node.name.contains('ð'));
    }

    #[test]
    fn happ_server_description_stripped() {
        let name = clean_display_name("Netherlands?serverDescription=SGFwcA==");
        assert_eq!(name, "Netherlands");
    }

    #[test]
    fn repairs_latin1_mojibake_flags() {
        let bytes = [0xF0, 0x9F, 0x87, 0xA9, 0xF0, 0x9F, 0x87, 0xAA, b' ', b'D', b'E'];
        let mojibake: String = bytes.iter().map(|&b| char::from(b)).collect();
        assert!(mojibake.contains('ð'));
        let fixed = repair_utf8_mojibake(&mojibake);
        assert!(fixed.contains('\u{1f1e9}'), "{fixed:?}");
        assert!(fixed.contains("DE"));
        assert!(!fixed.contains('ð'));
    }
}
