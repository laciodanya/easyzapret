//! Build Xray-core JSON config from a selected node + VPN settings.

use serde_json::{json, Map, Value};

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
            "sniffing": sniffing(settings)
        }),
        json!({
            "tag": "http-in",
            "port": settings.http_port,
            "listen": "127.0.0.1",
            "protocol": "http",
            "settings": { "allowTransparent": false },
            "sniffing": sniffing(settings)
        }),
    ];

    if settings.mode == "tun" {
        inbounds.push(json!({
            "tag": "tun-in",
            "protocol": "tun",
            "settings": {
                "name": "xray0",
                "desc": "EasyZapret",
                "mtu": 1500,
                "gateway": ["10.10.0.1/24"],
                "dns": [settings.dns.clone()],
                "autoSystemRoutingTable": ["0.0.0.0/0"],
                "autoOutboundsInterface": "auto"
            },
            "sniffing": sniffing(settings)
        }));
    }

    let mut routing_rules = Vec::new();
    if settings.routing_enabled && (settings.bypass_private || settings.bypass_lan) {
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

    let mut outbounds = vec![
        outbound,
        json!({ "tag": "direct", "protocol": "freedom", "settings": { "domainStrategy": "UseIP" } }),
        json!({ "tag": "block", "protocol": "blackhole", "settings": {} }),
    ];

    if settings.fragmentation {
        outbounds.insert(
            1,
            json!({
                "tag": "frag",
                "protocol": "freedom",
                "settings": {
                    "fragment": {
                        "packets": settings.fragmentation_packets,
                        "length": settings.fragmentation_length,
                        "interval": settings.fragmentation_interval
                    }
                }
            }),
        );
        if let Some(proxy) = outbounds.first_mut().and_then(|v| v.as_object_mut()) {
            let stream = proxy
                .entry("streamSettings")
                .or_insert_with(|| json!({}))
                .as_object_mut();
            if let Some(stream) = stream {
                let sockopt = stream
                    .entry("sockopt")
                    .or_insert_with(|| json!({}))
                    .as_object_mut();
                if let Some(sockopt) = sockopt {
                    sockopt.insert("dialerProxy".into(), json!("frag"));
                }
            }
        }
    }

    Ok(json!({
        "log": { "loglevel": "warning" },
        "dns": {
            "servers": [ settings.dns, "localhost" ],
            "queryStrategy": "UseIP"
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "routing": {
            "domainStrategy": "IPIfNonMatch",
            "rules": routing_rules
        }
    }))
}

fn sniffing(settings: &VpnSettings) -> Value {
    json!({
        "enabled": settings.sniffing,
        "destOverride": ["http", "tls", "quic"],
        "routeOnly": true
    })
}

fn build_outbound(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
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
        "shadowsocks" => build_ss(node, settings)?,
        "socks" => build_socks(node)?,
        "wireguard" => build_wireguard(node)?,
        "hysteria2" => return Err("hysteria2_needs_singbox".into()),
        other => return Err(format!("unsupported_protocol:{other}")),
    };

    let network = normalize_network(
        &pstr(&node.params, &["type", "net", "network"]).unwrap_or_else(|| "tcp".into()),
    );
    let security = stream_security(node);
    let mux_ok = settings.mux_enabled
        && security != "reality"
        && !matches!(network.as_str(), "grpc" | "xhttp");
    if mux_ok {
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

fn pstr(params: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = params.get(*key) {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            } else if let Some(n) = v.as_i64() {
                return Some(n.to_string());
            }
        }
    }
    None
}

fn normalize_network(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "ws" | "websocket" => "ws".into(),
        "grpc" | "gun" => "grpc".into(),
        "h2" | "http" => "h2".into(),
        "xhttp" | "splithttp" => "xhttp".into(),
        "httpupgrade" => "httpupgrade".into(),
        "kcp" | "mkcp" => "kcp".into(),
        "quic" => "quic".into(),
        _ => "tcp".into(),
    }
}

fn normalize_security(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "reality" => "reality".into(),
        "tls" | "1" | "true" | "xtls" => "tls".into(),
        _ => "none".into(),
    }
}

fn stream_security(node: &VpnNode) -> String {
    let params = &node.params;
    let network = normalize_network(
        &pstr(params, &["type", "net", "network"]).unwrap_or_else(|| "tcp".into()),
    );
    if pstr(params, &["pbk", "publickey", "publicKey"]).is_some() {
        return "reality".into();
    }
    let raw = pstr(params, &["security", "tls"]).unwrap_or_default();
    let lowered = raw.to_ascii_lowercase();
    if lowered.is_empty() || matches!(lowered.as_str(), "auto" | "aes-128-gcm" | "chacha20-poly1305" | "zero")
    {
        return if network == "tcp" { "none".into() } else { "tls".into() };
    }
    normalize_security(&raw)
}

fn build_stream(node: &VpnNode, settings: &VpnSettings) -> Value {
    let params = &node.params;
    let network = normalize_network(
        &pstr(params, &["type", "net", "network"]).unwrap_or_else(|| "tcp".into()),
    );
    let security = stream_security(node);

    let mut stream = Map::new();
    stream.insert("network".into(), json!(network));
    stream.insert("security".into(), json!(security));

    match network.as_str() {
        "ws" => {
            let mut ws = Map::new();
            if let Some(path) = pstr(params, &["path"]) {
                ws.insert("path".into(), json!(path));
            }
            let mut headers = Map::new();
            if let Some(host) = pstr(params, &["host", "sni"]) {
                headers.insert("Host".into(), json!(host));
            }
            if !headers.is_empty() {
                ws.insert("headers".into(), Value::Object(headers));
            }
            stream.insert("wsSettings".into(), Value::Object(ws));
        }
        "grpc" => {
            let mut grpc = Map::new();
            if let Some(svc) = pstr(params, &["servicename", "serviceName", "service_name"]) {
                grpc.insert("serviceName".into(), json!(svc));
            }
            if let Some(auth) = pstr(params, &["authority"]) {
                grpc.insert("authority".into(), json!(auth));
            }
            if pstr(params, &["mode"]).as_deref() == Some("multi") {
                grpc.insert("multiMode".into(), json!(true));
            }
            stream.insert("grpcSettings".into(), Value::Object(grpc));
        }
        "h2" => {
            let mut h2 = Map::new();
            if let Some(path) = pstr(params, &["path"]) {
                h2.insert("path".into(), json!(path));
            }
            if let Some(host) = pstr(params, &["host"]) {
                h2.insert("host".into(), json!([host]));
            }
            stream.insert("httpSettings".into(), Value::Object(h2));
        }
        "xhttp" => {
            let mut xh = Map::new();
            if let Some(path) = pstr(params, &["path"]) {
                xh.insert("path".into(), json!(path));
            }
            if let Some(host) = pstr(params, &["host"]) {
                xh.insert("host".into(), json!(host));
            }
            if let Some(mode) = pstr(params, &["mode"]) {
                xh.insert("mode".into(), json!(mode));
            }
            if let Some(extra) = pstr(params, &["extra"]) {
                if let Ok(v) = serde_json::from_str::<Value>(&extra) {
                    xh.insert("extra".into(), v);
                }
            }
            stream.insert("xhttpSettings".into(), Value::Object(xh));
        }
        "httpupgrade" => {
            let mut hu = Map::new();
            if let Some(path) = pstr(params, &["path"]) {
                hu.insert("path".into(), json!(path));
            }
            if let Some(host) = pstr(params, &["host"]) {
                hu.insert("host".into(), json!(host));
            }
            stream.insert("httpupgradeSettings".into(), Value::Object(hu));
        }
        "tcp" => {
            let header = pstr(params, &["headertype", "headerType"]).unwrap_or_default();
            if header.eq_ignore_ascii_case("http") {
                let host = pstr(params, &["host"]).unwrap_or_else(|| node.address.clone());
                let path = pstr(params, &["path"]).unwrap_or_else(|| "/".into());
                stream.insert(
                    "tcpSettings".into(),
                    json!({
                        "header": {
                            "type": "http",
                            "request": {
                                "path": [path],
                                "headers": { "Host": [host] }
                            }
                        }
                    }),
                );
            }
        }
        _ => {}
    }

    if security == "tls" {
        let mut tls = Map::new();
        let sni = pstr(params, &["sni", "host"]).unwrap_or_else(|| node.address.clone());
        tls.insert("serverName".into(), json!(sni));
        let insecure = settings.allow_insecure
            || matches!(
                pstr(params, &["insecure", "allowinsecure", "allowInsecure"]).as_deref(),
                Some("1" | "true")
            );
        tls.insert("allowInsecure".into(), json!(insecure));
        if let Some(fp) = pstr(params, &["fp", "fingerprint"]) {
            tls.insert("fingerprint".into(), json!(fp));
        }
        if let Some(alpn) = pstr(params, &["alpn"]) {
            let list: Vec<&str> = alpn.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
            if !list.is_empty() {
                tls.insert("alpn".into(), json!(list));
            }
        }
        stream.insert("tlsSettings".into(), Value::Object(tls));
    } else if security == "reality" {
        let mut reality = Map::new();
        reality.insert("show".into(), json!(false));
        let sni = pstr(params, &["sni"]).unwrap_or_else(|| node.address.clone());
        reality.insert("serverName".into(), json!(sni));
        if let Some(fp) = pstr(params, &["fp", "fingerprint"]) {
            reality.insert("fingerprint".into(), json!(fp));
        }
        if let Some(pbk) = pstr(params, &["pbk", "publickey", "publicKey"]) {
            reality.insert("publicKey".into(), json!(pbk));
        }
        if let Some(sid) = pstr(params, &["sid", "shortid", "shortId"]) {
            reality.insert("shortId".into(), json!(sid));
        }
        if let Some(spx) = pstr(params, &["spx", "spiderx", "spiderX"]) {
            reality.insert("spiderX".into(), json!(spx));
        }
        if let Some(pqv) = pstr(params, &["pqv"]) {
            reality.insert("mldsa65Verify".into(), json!(pqv));
        }
        stream.insert("realitySettings".into(), Value::Object(reality));
    }

    Value::Object(stream)
}

fn build_vless(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
    let id = pstr(&node.params, &["id", "uuid"]).ok_or("vless missing id")?;
    let encryption = pstr(&node.params, &["encryption"]).unwrap_or_else(|| "none".into());
    let mut user = Map::new();
    user.insert("id".into(), json!(id));
    user.insert("encryption".into(), json!(encryption));
    let network = normalize_network(
        &pstr(&node.params, &["type", "net", "network"]).unwrap_or_else(|| "tcp".into()),
    );
    if let Some(flow) = pstr(&node.params, &["flow"]) {
        // xtls-rprx-vision is TCP/XHTTP only — grpc/ws drop it or the tunnel is dead.
        if matches!(network.as_str(), "tcp" | "raw" | "xhttp") {
            user.insert("flow".into(), json!(flow));
        }
    }
    if let Some(enc) = pstr(&node.params, &["packetencoding", "packetEncoding"]) {
        user.insert("packetEncoding".into(), json!(enc));
    }
    Ok(json!({
        "protocol": "vless",
        "settings": {
            "vnext": [{
                "address": node.address,
                "port": node.port,
                "users": [user]
            }]
        },
        "streamSettings": build_stream(node, settings)
    }))
}

fn build_vmess(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
    let id = pstr(&node.params, &["id"]).ok_or("vmess missing id")?;
    let aid = node
        .params
        .get("aid")
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0);
    let security = pstr(&node.params, &["scy", "security"]).unwrap_or_else(|| "auto".into());
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
    let password = pstr(&node.params, &["password"]).ok_or("trojan missing password")?;
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

fn build_ss(node: &VpnNode, settings: &VpnSettings) -> Result<Value, String> {
    let method = pstr(&node.params, &["method"]).ok_or("ss missing method")?;
    let password = pstr(&node.params, &["password"]).ok_or("ss missing password")?;
    let mut outbound = json!({
        "protocol": "shadowsocks",
        "settings": {
            "servers": [{
                "address": node.address,
                "port": node.port,
                "method": method,
                "password": password
            }]
        }
    });
    if pstr(&node.params, &["type", "plugin"]).is_some() {
        if let Some(obj) = outbound.as_object_mut() {
            obj.insert("streamSettings".into(), build_stream(node, settings));
        }
    }
    Ok(outbound)
}

fn build_socks(node: &VpnNode) -> Result<Value, String> {
    let user = pstr(&node.params, &["user"]);
    let pass = pstr(&node.params, &["pass"]);
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

fn build_wireguard(node: &VpnNode) -> Result<Value, String> {
    let secret = pstr(&node.params, &["secretkey", "secretKey", "privatekey"]).ok_or("wg missing key")?;
    let public = pstr(&node.params, &["publickey", "publicKey"]).ok_or("wg missing publickey")?;
    let address = pstr(&node.params, &["address"]).unwrap_or_else(|| "10.0.0.2/32".into());
    Ok(json!({
        "protocol": "wireguard",
        "settings": {
            "secretKey": secret,
            "address": [address],
            "peers": [{
                "publicKey": public,
                "endpoint": format!("{}:{}", node.address, node.port)
            }]
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::parse::parse_share_link;

    #[test]
    fn grpc_reality_outbound_uses_grpc_and_reality_settings() {
        let link = "vless://11111111-1111-1111-1111-111111111111@de.example.com:443?encryption=none&security=reality&sni=www.microsoft.com&fp=chrome&pbk=PUBLICKEY&sid=abcd1234&type=grpc&serviceName=Tune&flow=xtls-rprx-vision#DE";
        let node = parse_share_link(link).unwrap();
        let settings = VpnSettings::default();
        let cfg = build_config(&node, &settings).unwrap();
        let proxy = &cfg["outbounds"][0];
        assert_eq!(proxy["protocol"], "vless");
        assert_eq!(proxy["streamSettings"]["network"], "grpc");
        assert_eq!(proxy["streamSettings"]["security"], "reality");
        assert_eq!(proxy["streamSettings"]["grpcSettings"]["serviceName"], "Tune");
        assert_eq!(
            proxy["streamSettings"]["realitySettings"]["publicKey"],
            "PUBLICKEY"
        );
        assert!(
            proxy["settings"]["vnext"][0]["users"][0]
                .get("flow")
                .is_none(),
            "vision flow is invalid on grpc"
        );
        assert!(cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["protocol"] == "tun"));
    }

    #[test]
    fn tcp_reality_keeps_vision_flow() {
        let link = "vless://11111111-1111-1111-1111-111111111111@de.example.com:443?encryption=none&security=reality&sni=www.microsoft.com&fp=chrome&pbk=PUBLICKEY&sid=abcd1234&type=tcp&flow=xtls-rprx-vision#DE";
        let node = parse_share_link(link).unwrap();
        let settings = VpnSettings {
            mode: "system-proxy".into(),
            ..VpnSettings::default()
        };
        let cfg = build_config(&node, &settings).unwrap();
        assert_eq!(
            cfg["outbounds"][0]["settings"]["vnext"][0]["users"][0]["flow"],
            "xtls-rprx-vision"
        );
        assert_eq!(cfg["outbounds"][0]["streamSettings"]["security"], "reality");
    }

    #[test]
    fn empty_flow_omitted() {
        let link = "vless://11111111-1111-1111-1111-111111111111@host.example:443?encryption=none&security=tls&type=ws&path=%2F&host=host.example#WS";
        let node = parse_share_link(link).unwrap();
        let settings = VpnSettings {
            mode: "system-proxy".into(),
            ..VpnSettings::default()
        };
        let cfg = build_config(&node, &settings).unwrap();
        let user = &cfg["outbounds"][0]["settings"]["vnext"][0]["users"][0];
        assert!(user.get("flow").is_none(), "{user}");
        assert_eq!(cfg["outbounds"][0]["streamSettings"]["network"], "ws");
        assert!(cfg["inbounds"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["protocol"] != "tun"));
    }
}
