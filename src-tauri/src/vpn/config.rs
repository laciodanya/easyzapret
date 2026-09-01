//! Build Xray-core JSON config from a selected node + VPN settings.

use serde_json::{json, Value};

use super::store::{VpnNode, VpnSettings};

pub fn build_config(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
    let outbound = build_outbound(node, settings)?;

    let mut inbounds = vec![
        json!({
            "tag": "socks-in",
            "port": settings.socks_port,
            "listen": "127.0.0.1",
            "protocol": "socks",
            "settings": { "udp": true, "auth": "noauth" },
            "sniffing": {
                "enabled": settings.sniffing,
                "destOverride": ["http", "tls", "quic"]
            }
        }),
        json!({
            "tag": "http-in",
            "port": settings.http_port,
            "listen": "127.0.0.1",
            "protocol": "http",
            "settings": {},
            "sniffing": {
                "enabled": settings.sniffing,
                "destOverride": ["http", "tls", "quic"]
            }
        }),
    ];

    // Keep clippy quiet if we later add TUN inbounds.
    let _ = &mut inbounds;

    let mut routing_rules = Vec::new();
    if settings.routing_enabled {
        if settings.bypass_private || settings.bypass_lan {
            routing_rules.push(json!({
                "type": "field",
                "outboundTag": "direct",
                "ip": [
                    "0.0.0.0/8",
                    "10.0.0.0/8",
                    "127.0.0.0/8",
                    "169.254.0.0/16",
                    "172.16.0.0/12",
                    "192.168.0.0/16",
                    "::1/128",
                    "fc00::/7",
                    "fe80::/10"
                ]
            }));
        }
        routing_rules.push(json!({
            "type": "field",
            "outboundTag": "proxy",
            "network": "tcp,udp"
        }));
    }

    let config = json!({
        "log": { "loglevel": "warning" },
        "dns": {
            "servers": [ settings.dns, "localhost" ]
        },
        "inbounds": inbounds,
        "outbounds": [
            outbound,
            { "tag": "direct", "protocol": "freedom", "settings": {} },
            { "tag": "block", "protocol": "blackhole", "settings": {} }
        ],
        "routing": {
            "domainStrategy": "AsIs",
            "rules": routing_rules
        }
    });
    Ok(config)
}

fn build_outbound(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
    // Full xray outbound JSON already?
    if node.params.get("protocol").and_then(|v| v.as_str()).is_some()
        && node.params.get("settings").is_some()
    {
        let mut ob = node.params.clone();
        if let Some(obj) = ob.as_object_mut() {
            obj.insert("tag".into(), json!("proxy"));
        }
        return Ok(ob);
    }

    let mut outbound = match node.protocol.as_str() {
        "vless" => build_vless(node, settings)?,
        "vmess" => build_vmess(node, settings)?,
        "trojan" => build_trojan(node, settings)?,
        "shadowsocks" => build_ss(node)?,
        "socks" => build_socks(node)?,
        "hysteria2" => {
            return Err("hysteria2_needs_singbox".into());
        }
        other => return Err(format!("unsupported_protocol:{other}")),
    };

    if settings.mux_enabled {
        if let Some(obj) = outbound.as_object_mut() {
            obj.insert(
                "mux".into(),
                json!({
                    "enabled": true,
                    "concurrency": settings.mux_concurrency
                }),
            );
        }
    }

    if let Some(obj) = outbound.as_object_mut() {
        obj.insert("tag".into(), json!("proxy"));
    }
    Ok(outbound)
}

fn pstr(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
}

fn build_stream(node: &VpnNode, settings: &VpnSettings) -> Value {
    let params = &node.params;
    let network = pstr(params, "type")
        .or_else(|| pstr(params, "net"))
        .unwrap_or_else(|| "tcp".into());
    let security = pstr(params, "security")
        .or_else(|| pstr(params, "tls"))
        .unwrap_or_else(|| {
            if network == "ws" || network == "grpc" || network == "h2" || network == "httpupgrade" || network == "xhttp" {
                "tls".into()
            } else {
                "none".into()
            }
        });

    let mut stream = serde_json::Map::new();
    stream.insert("network".into(), json!(network));
    stream.insert("security".into(), json!(if security == "reality" || security == "tls" { security.clone() } else if security == "1" || security == "true" { "tls".into() } else { "none".into() }));

    match network.as_str() {
        "ws" => {
            let mut ws = serde_json::Map::new();
            if let Some(path) = pstr(params, "path") {
                ws.insert("path".into(), json!(path));
            }
            let mut headers = serde_json::Map::new();
            if let Some(host) = pstr(params, "host") {
                headers.insert("Host".into(), json!(host));
            }
            if !headers.is_empty() {
                ws.insert("headers".into(), Value::Object(headers));
            }
            stream.insert("wsSettings".into(), Value::Object(ws));
        }
        "grpc" => {
            let mut grpc = serde_json::Map::new();
            if let Some(svc) = pstr(params, "serviceName").or_else(|| pstr(params, "serviceName")) {
                grpc.insert("serviceName".into(), json!(svc));
            }
            stream.insert("grpcSettings".into(), Value::Object(grpc));
        }
        "h2" | "http" => {
            let mut h2 = serde_json::Map::new();
            if let Some(path) = pstr(params, "path") {
                h2.insert("path".into(), json!(path));
            }
            if let Some(host) = pstr(params, "host") {
                h2.insert("host".into(), json!([host]));
            }
            stream.insert("httpSettings".into(), Value::Object(h2));
        }
        "xhttp" | "splithttp" => {
            let mut xh = serde_json::Map::new();
            if let Some(path) = pstr(params, "path") {
                xh.insert("path".into(), json!(path));
            }
            if let Some(host) = pstr(params, "host") {
                xh.insert("host".into(), json!(host));
            }
            if let Some(mode) = pstr(params, "mode") {
                xh.insert("mode".into(), json!(mode));
            }
            stream.insert("xhttpSettings".into(), Value::Object(xh));
        }
        "httpupgrade" => {
            let mut hu = serde_json::Map::new();
            if let Some(path) = pstr(params, "path") {
                hu.insert("path".into(), json!(path));
            }
            if let Some(host) = pstr(params, "host") {
                hu.insert("host".into(), json!(host));
            }
            stream.insert("httpupgradeSettings".into(), Value::Object(hu));
        }
        _ => {}
    }

    let sec = stream.get("security").and_then(|v| v.as_str()).unwrap_or("none");
    if sec == "tls" {
        let mut tls = serde_json::Map::new();
        let sni = pstr(params, "sni")
            .or_else(|| pstr(params, "host"))
            .unwrap_or_else(|| node.address.clone());
        tls.insert("serverName".into(), json!(sni));
        tls.insert(
            "allowInsecure".into(),
            json!(settings.allow_insecure || pstr(params, "insecure").as_deref() == Some("1")),
        );
        if let Some(fp) = pstr(params, "fp").or_else(|| pstr(params, "fingerprint")) {
            tls.insert("fingerprint".into(), json!(fp));
        }
        if let Some(alpn) = pstr(params, "alpn") {
            let list: Vec<&str> = alpn.split(',').collect();
            tls.insert("alpn".into(), json!(list));
        }
        stream.insert("tlsSettings".into(), Value::Object(tls));
    } else if sec == "reality" {
        let mut reality = serde_json::Map::new();
        let sni = pstr(params, "sni").unwrap_or_else(|| node.address.clone());
        reality.insert("serverName".into(), json!(sni));
        if let Some(fp) = pstr(params, "fp") {
            reality.insert("fingerprint".into(), json!(fp));
        }
        if let Some(pbk) = pstr(params, "pbk") {
            reality.insert("publicKey".into(), json!(pbk));
        }
        if let Some(sid) = pstr(params, "sid") {
            reality.insert("shortId".into(), json!(sid));
        }
        if let Some(spx) = pstr(params, "spx") {
            reality.insert("spiderX".into(), json!(spx));
        }
        stream.insert("realitySettings".into(), Value::Object(reality));
    }

    if settings.fragmentation {
        let mut sockopt = serde_json::Map::new();
        sockopt.insert(
            "dialerProxy".into(),
            json!(""),
        );
        // Xray fragment via sockopt
        sockopt.insert(
            "tcpMaxSeg".into(),
            json!(1400),
        );
        let mut fragment = serde_json::Map::new();
        fragment.insert("packets".into(), json!(settings.fragmentation_packets));
        fragment.insert("length".into(), json!(settings.fragmentation_length));
        fragment.insert("interval".into(), json!(settings.fragmentation_interval));
        // Newer xray uses freedom dialer; keep sockopt marker for future
        stream.insert("sockopt".into(), Value::Object(sockopt));
        let _ = fragment;
    }

    Value::Object(stream)
}

fn build_vless(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
    let id = pstr(&node.params, "id").ok_or("vless missing id")?;
    let flow = pstr(&node.params, "flow").unwrap_or_default();
    let encryption = pstr(&node.params, "encryption").unwrap_or_else(|| "none".into());
    Ok(json!({
        "protocol": "vless",
        "settings": {
            "vnext": [{
                "address": node.address,
                "port": node.port,
                "users": [{
                    "id": id,
                    "encryption": encryption,
                    "flow": flow
                }]
            }]
        },
        "streamSettings": build_stream(node, settings)
    }))
}

fn build_vmess(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
    let id = pstr(&node.params, "id").ok_or("vmess missing id")?;
    let aid = node
        .params
        .get("aid")
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0);
    let security = pstr(&node.params, "scy").unwrap_or_else(|| "auto".into());
    Ok(json!({
        "protocol": "vmess",
        "settings": {
            "vnext": [{
                "address": node.address,
                "port": node.port,
                "users": [{
                    "id": id,
                    "alterId": aid,
                    "security": security
                }]
            }]
        },
        "streamSettings": build_stream(node, settings)
    }))
}

fn build_trojan(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
    let password = pstr(&node.params, "password").ok_or("trojan missing password")?;
    Ok(json!({
        "protocol": "trojan",
        "settings": {
            "servers": [{
                "address": node.address,
                "port": node.port,
                "password": password
            }]
        },
        "streamSettings": build_stream(node, settings)
    }))
}

fn build_ss(node: &VpnNode) -> Result<Value, String> {
    let method = pstr(&node.params, "method").ok_or("ss missing method")?;
    let password = pstr(&node.params, "password").ok_or("ss missing password")?;
    Ok(json!({
        "protocol": "shadowsocks",
        "settings": {
            "servers": [{
                "address": node.address,
                "port": node.port,
                "method": method,
                "password": password
            }]
        }
    }))
}

fn build_socks(node: &VpnNode) -> Result<Value, String> {
    let user = pstr(&node.params, "user");
    let pass = pstr(&node.params, "pass");
    let mut server = json!({
        "address": node.address,
        "port": node.port
    });
    if let (Some(u), Some(p)) = (user, pass) {
        server.as_object_mut().unwrap().insert(
            "users".into(),
            json!([{ "user": u, "pass": p }]),
        );
    }
    Ok(json!({
        "protocol": "socks",
        "settings": { "servers": [server] }
    }))
}
