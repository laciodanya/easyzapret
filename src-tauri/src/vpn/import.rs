use base64::{engine::general_purpose, Engine};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::types::{new_id, Profile};

pub const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;

fn decode(value: &str) -> Result<String, String> {
    let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = [
        general_purpose::STANDARD,
        general_purpose::STANDARD_NO_PAD,
        general_purpose::URL_SAFE,
        general_purpose::URL_SAFE_NO_PAD,
    ]
    .iter()
    .find_map(|engine| engine.decode(&compact).ok())
    .ok_or("vpn_invalid_base64")?;
    String::from_utf8(bytes).map_err(|_| "vpn_invalid_base64".into())
}

fn unescape(value: &str) -> String {
    percent_encoding::percent_decode_str(value)
        .decode_utf8_lossy()
        .into_owned()
}

fn profile(name: String, engine: &str, mut outbound: Value) -> Result<Profile, String> {
    if !outbound.is_object() {
        return Err("vpn_invalid_json".into());
    }
    outbound["tag"] = json!("proxy");
    let result = Profile {
        id: new_id(),
        name,
        engine: engine.into(),
        outbound,
        subscription_id: None,
    };
    validate_profile(&result)?;
    Ok(result)
}

/// Subscription data can describe servers, but cannot take control of the
/// application's listeners, routes, file paths, or executable selection.
fn reject_external_resources(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key.ends_with("_path")
                    || key.ends_with("File")
                    || key.ends_with("file")
                    || matches!(
                        key.as_str(),
                        "detour"
                            | "proxySettings"
                            | "sendThrough"
                            | "bind_interface"
                            | "routing_mark"
                            | "netns"
                            | "dialerProxy"
                            | "certificate_path"
                            | "key_path"
                    )
                {
                    return Err("vpn_external_resource_in_profile".into());
                }
                reject_external_resources(child)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_external_resources(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn validate_profile(profile: &Profile) -> Result<(), String> {
    reject_external_resources(&profile.outbound)?;
    let protocol = if profile.engine == "sing-box" {
        profile.outbound["type"].as_str().unwrap_or_default()
    } else if profile.engine == "xray" {
        profile.outbound["protocol"].as_str().unwrap_or_default()
    } else {
        return Err("vpn_unsupported_engine".into());
    };
    if !matches!(
        protocol,
        "vless"
            | "vmess"
            | "trojan"
            | "shadowsocks"
            | "socks"
            | "http"
            | "hysteria2"
            | "tuic"
            | "anytls"
    ) {
        return Err("vpn_unsupported_protocol".into());
    }
    if profile.engine == "xray" {
        let credential = match protocol {
            "vless" | "vmess" => profile.outbound.pointer("/settings/vnext/0/users/0/id"),
            "trojan" | "shadowsocks" => profile.outbound.pointer("/settings/servers/0/password"),
            _ => None,
        };
        if matches!(protocol, "vless" | "vmess" | "trojan" | "shadowsocks")
            && !credential
                .and_then(Value::as_str)
                .is_some_and(|v| !v.is_empty() && v != "null")
        {
            return Err("vpn_invalid_link".into());
        }
    }
    let port = if profile.engine == "sing-box" {
        profile.outbound.get("server_port")
    } else {
        profile
            .outbound
            .pointer("/settings/vnext/0/port")
            .or_else(|| profile.outbound.pointer("/settings/servers/0/port"))
    };
    if !port
        .and_then(Value::as_u64)
        .is_some_and(|p| (1..=65535).contains(&p))
    {
        return Err("vpn_invalid_port".into());
    }
    if profile.name.is_empty()
        || profile.name.len() > 256
        || profile.server().is_empty()
        || profile.server().len() > 253
        || profile.port() == 0
    {
        return Err("vpn_invalid_profile".into());
    }
    Ok(())
}

fn xray_stream(q: &HashMap<String, String>) -> Result<Value, String> {
    let get = |key: &str, default: &str| q.get(key).cloned().unwrap_or_else(|| default.into());
    let network = get("type", "tcp");
    let security = get("security", "none");
    let mut stream = json!({"network": network, "security": security});
    let sni = get("sni", &get("peer", ""));
    match security.as_str() {
        "tls" => {
            stream["tlsSettings"] = json!({"serverName":sni,
                "allowInsecure":get("allowInsecure", "0") == "1",
                "fingerprint":get("fp", "chrome")});
            if let Some(alpn) = q.get("alpn") {
                stream["tlsSettings"]["alpn"] = json!(alpn.split(',').collect::<Vec<_>>());
            }
        }
        "reality" => {
            stream["realitySettings"] = json!({"serverName":sni,
                "fingerprint":get("fp", "chrome"), "publicKey":get("pbk", ""),
                "shortId":get("sid", ""), "spiderX":get("spx", "/")});
        }
        "none" => {}
        _ => return Err("vpn_unsupported_security".into()),
    }
    let host = get("host", "");
    let path = get("path", "/");
    match network.as_str() {
        "tcp" | "raw" => {
            if get("headerType", "none") == "http" {
                stream["tcpSettings"] = json!({"header":{"type":"http",
                    "request":{"path":[path],"headers":{"Host":[host]}}}});
            }
        }
        "ws" => {
            stream["wsSettings"] = json!({"path":path,"headers":{"Host":host}});
        }
        "grpc" => {
            stream["grpcSettings"] = json!({"serviceName":get("serviceName", ""),
                "multiMode":get("mode", "gun") == "multi"});
        }
        "httpupgrade" => {
            stream["httpupgradeSettings"] = json!({"host":host,"path":path});
        }
        "xhttp" | "splithttp" => {
            stream["network"] = json!("xhttp");
            stream["xhttpSettings"] = json!({"host":host,"path":path,"mode":get("mode","auto")});
            if let Some(extra) = q.get("extra") {
                stream["xhttpSettings"]["extra"] =
                    serde_json::from_str(extra).map_err(|_| "vpn_invalid_xhttp_extra")?;
            }
        }
        "kcp" => {
            stream["kcpSettings"] = json!({"seed":get("seed",""),
                "header":{"type":get("headerType","none")}});
        }
        _ => return Err("vpn_unsupported_transport".into()),
    }
    Ok(stream)
}

fn vmess(raw: &str) -> Result<Profile, String> {
    let value: Value = serde_json::from_str(&decode(raw)?).map_err(|_| "vpn_invalid_vmess")?;
    let string = |key: &str| {
        value[key]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value[key].to_string())
    };
    let server = string("add");
    let port = string("port")
        .parse::<u16>()
        .map_err(|_| "vpn_invalid_port")?;
    let mut query = HashMap::new();
    for (from, to) in [
        ("net", "type"),
        ("tls", "security"),
        ("sni", "sni"),
        ("host", "host"),
        ("path", "path"),
        ("fp", "fp"),
        ("alpn", "alpn"),
        ("type", "headerType"),
    ] {
        if let Some(v) = value[from].as_str().filter(|v| !v.is_empty()) {
            query.insert(to.into(), v.into());
        }
    }
    if query.get("type").map(String::as_str) == Some("grpc") {
        query.insert("serviceName".into(), string("path"));
    }
    let aid = string("aid").parse::<u32>().unwrap_or_default();
    let name = value["ps"]
        .as_str()
        .filter(|v| !v.is_empty())
        .unwrap_or(&server)
        .to_owned();
    profile(
        name,
        "xray",
        json!({"protocol":"vmess", "settings":{"vnext":[{
        "address":server,"port":port,"users":[{"id":string("id"),"alterId":aid,
        "security":value["scy"].as_str().unwrap_or("auto")}]}]}, "streamSettings":xray_stream(&query)?}),
    )
}

fn link(raw: &str) -> Result<Profile, String> {
    if let Some(value) = raw.strip_prefix("vmess://") {
        return vmess(value);
    }
    let mut normalized = raw.to_owned();
    // SIP002 also permits Base64 encoding of the entire method:password@host:port.
    if let Some(value) = raw.strip_prefix("ss://") {
        let authority = value.split(['#', '?']).next().unwrap_or_default();
        if !authority.contains('@') {
            normalized = format!("ss://{}{}", decode(authority)?, &value[authority.len()..]);
        }
    }
    let url = reqwest::Url::parse(&normalized).map_err(|_| "vpn_invalid_link")?;
    let server = url
        .host_str()
        .ok_or("vpn_invalid_link")?
        .trim_matches(['[', ']'])
        .to_owned();
    let port = url.port().unwrap_or(443);
    let user = unescape(url.username());
    let password = url.password().map(unescape).unwrap_or_default();
    let name = url
        .fragment()
        .map(unescape)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| server.clone());
    let mut q: HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let protocol = url.scheme();
    let outbound = match protocol {
        "vless" => json!({"protocol":"vless","settings":{"vnext":[{
            "address":server,"port":port,"users":[{"id":user,
            "encryption":q.get("encryption").map(String::as_str).unwrap_or("none"),
            "flow":q.get("flow").map(String::as_str).unwrap_or("")}]}]},"streamSettings":xray_stream(&q)?}),
        "trojan" => {
            q.entry("security".into()).or_insert_with(|| "tls".into());
            json!({"protocol":"trojan","settings":{"servers":[{"address":server,
                "port":port,"password":user}]},"streamSettings":xray_stream(&q)?})
        }
        "ss" => {
            if q.contains_key("plugin") {
                return Err("vpn_shadowsocks_plugin_unsupported".into());
            }
            let credentials = if password.is_empty() {
                decode(&user)?
            } else {
                format!("{user}:{password}")
            };
            let (method, secret) = credentials.split_once(':').ok_or("vpn_invalid_link")?;
            json!({"protocol":"shadowsocks","settings":{"servers":[{"address":server,
                "port":port,"method":method,"password":secret}]}})
        }
        "socks" | "socks5" | "http" | "https" => {
            let mut node = json!({"address":server,"port":port});
            if !user.is_empty() {
                node["users"] = json!([{"user":user,"pass":password}]);
            }
            let mut out = json!({"protocol":if protocol.starts_with("socks") {"socks"} else {"http"},
                "settings":{"servers":[node]}});
            if protocol == "https" {
                q.insert("security".into(), "tls".into());
                out["streamSettings"] = xray_stream(&q)?;
            }
            out
        }
        "hysteria2" | "hy2" | "tuic" | "anytls" => {
            let get = |k: &str, default: &str| q.get(k).cloned().unwrap_or_else(|| default.into());
            let mut out = json!({"type":if protocol == "hy2" {"hysteria2"} else {protocol},
                "server":server,"server_port":port,"tls":{"enabled":true,
                "server_name":get("sni",&server),"insecure":get("insecure","0") == "1"}});
            if protocol == "tuic" {
                out["uuid"] = json!(user);
                out["password"] = json!(password);
                out["congestion_control"] = json!(get("congestion_control", "cubic"));
                out["udp_relay_mode"] = json!(get("udp_relay_mode", "native"));
                out["tls"]["alpn"] = json!(get("alpn", "h3").split(',').collect::<Vec<_>>());
            } else {
                out["password"] = json!(if password.is_empty() {
                    user
                } else {
                    format!("{user}:{password}")
                });
                if let Some(obfs) = q.get("obfs") {
                    out["obfs"] = json!({"type":obfs,"password":get("obfs-password","")});
                }
            }
            return profile(name, "sing-box", out);
        }
        _ => return Err("vpn_unsupported_protocol".into()),
    };
    profile(name, "xray", outbound)
}

pub fn parse(input: &str) -> Result<Vec<Profile>, String> {
    if input.len() > MAX_IMPORT_BYTES {
        return Err("vpn_import_too_large".into());
    }
    let mut body = input.trim().trim_start_matches('\u{feff}').to_owned();
    if !body.starts_with(['{', '[']) && !body.contains("://") {
        body = decode(&body)?;
    }
    let mut profiles = Vec::new();
    if body.starts_with(['{', '[']) {
        let root: Value = serde_json::from_str(&body).map_err(|_| "vpn_invalid_json")?;
        let nodes = root
            .get("outbounds")
            .and_then(Value::as_array)
            .or_else(|| root.as_array())
            .cloned()
            .unwrap_or_else(|| vec![root]);
        for node in nodes {
            let kind = node
                .get("type")
                .or_else(|| node.get("protocol"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            if matches!(
                kind,
                "direct" | "freedom" | "block" | "blackhole" | "dns" | "selector" | "urltest"
            ) {
                continue;
            }
            let engine = if node.get("protocol").is_some() {
                "xray"
            } else {
                "sing-box"
            };
            let name = node["tag"]
                .as_str()
                .filter(|n| !n.is_empty())
                .unwrap_or("VPN")
                .to_owned();
            profiles.push(profile(name, engine, node)?);
        }
    } else {
        for (index, line) in body.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Errors identify a line without copying its credentials.
            profiles.push(link(line).map_err(|e| format!("{e} (line {})", index + 1))?);
            if profiles.len() > 256 {
                return Err("vpn_too_many_profiles".into());
            }
        }
    }
    if profiles.is_empty() {
        return Err("vpn_no_profiles".into());
    }
    if profiles.len() > 256 {
        return Err("vpn_too_many_profiles".into());
    }
    Ok(profiles)
}
