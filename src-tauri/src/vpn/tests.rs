use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;

const VLESS: &str = "vless://d342d11e-d424-4583-b36e-524ab1f0afa4@vpn.example.com:443?security=reality&sni=example.com&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=1234&fp=chrome&type=xhttp&path=%2Fvpn&mode=auto#Test";

#[test]
fn imports_reality_xhttp_without_losing_transport_parameters() {
    let profiles = import::parse(VLESS).unwrap();
    let p = &profiles[0];
    assert_eq!(p.engine, "xray");
    assert_eq!(p.outbound["streamSettings"]["network"], "xhttp");
    assert_eq!(
        p.outbound["streamSettings"]["xhttpSettings"]["path"],
        "/vpn"
    );
    assert_eq!(
        p.outbound["streamSettings"]["realitySettings"]["serverName"],
        "example.com"
    );
    assert_eq!(p.name, "Test");
    assert_eq!(p.port(), 443);
}

#[test]
fn imports_base64_subscription_vmess_and_sip002_credentials() {
    let vmess = STANDARD.encode(
        json!({"v":"2","ps":"VMess","add":"vpn.example.com","port":"443",
        "id":"d342d11e-d424-4583-b36e-524ab1f0afa4","aid":"0","net":"ws","tls":"tls",
        "host":"example.com","path":"/ws"})
        .to_string(),
    );
    let ss = STANDARD.encode("aes-128-gcm:p@ss:word");
    let body = format!(
        "#profile-title: Test\n{VLESS}\nvmess://{vmess}\nss://{ss}@vpn.example.com:8388#SS"
    );
    let nodes = import::parse(&STANDARD.encode(body)).unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[1].outbound["streamSettings"]["network"], "ws");
    assert_eq!(
        nodes[2].outbound["settings"]["servers"][0]["password"],
        "p@ss:word"
    );
}

#[test]
fn malformed_subscription_is_atomic_and_errors_do_not_contain_keys() {
    let error = import::parse(&format!("{VLESS}\nvless://SUPER_SECRET_TOKEN")).unwrap_err();
    assert!(!error.contains("SUPER_SECRET_TOKEN"));
    assert!(error.contains("line 2"));
}

#[test]
fn imports_ipv6_hysteria_and_rejects_external_resources() {
    let nodes = import::parse(
        "hy2://password@[2001:db8::1]:443?sni=example.com&obfs=salamander&obfs-password=test#IPv6",
    )
    .unwrap();
    assert_eq!(nodes[0].server(), "2001:db8::1");
    assert_eq!(nodes[0].outbound["obfs"]["password"], "test");
    let malicious = json!({"type":"trojan","server":"example.com","server_port":443,
        "password":"secret","tls":{"enabled":true,"certificate_path":"C:/private.pem"}});
    assert_eq!(
        import::parse(&malicious.to_string()).unwrap_err(),
        "vpn_external_resource_in_profile"
    );
}

#[test]
fn only_selected_apps_gate_precedes_custom_proxy_rules() {
    let profile = import::parse(VLESS).unwrap().remove(0);
    let settings = VpnSettings {
        apps: vec!["Discord.exe".into(), r"C:\Apps\Browser.exe".into()],
        rules: vec![RouteRule {
            kind: "domain_suffix".into(),
            values: vec!["example.com".into()],
            outbound: "proxy".into(),
        }],
        ..Default::default()
    };
    let config = config::build(&settings, &profile).unwrap();
    let rules = config["route"]["rules"].as_array().unwrap();
    let gate = rules.iter().position(|r| r["type"] == "logical").unwrap();
    let custom = rules
        .iter()
        .position(|r| r.get("domain_suffix").is_some())
        .unwrap();
    assert!(gate < custom);
    assert_eq!(rules[gate]["invert"], true);
    assert_eq!(rules[gate]["mode"], "or");
    assert_eq!(rules[gate]["outbound"], "direct");
    assert_eq!(config["inbounds"][1]["strict_route"], false);
    assert_eq!(config["dns"]["servers"][1]["detour"], "direct");
}

#[test]
fn bypass_mode_and_empty_selection_are_explicit() {
    let profile = import::parse(VLESS).unwrap().remove(0);
    assert_eq!(
        config::build(&VpnSettings::default(), &profile).unwrap_err(),
        "vpn_select_apps"
    );
    let settings = VpnSettings {
        mode: Mode::BypassApps,
        apps: vec!["Game.exe".into()],
        ..Default::default()
    };
    let value = config::build(&settings, &profile).unwrap();
    let rules = value["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.iter().find(|r| r["type"] == "logical").unwrap()["invert"],
        false
    );
    let settings = VpnSettings {
        mode: Mode::Proxy,
        ..Default::default()
    };
    assert_eq!(
        config::build(&settings, &profile).unwrap()["inbounds"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn subscription_refresh_keeps_selected_server_and_replaces_removed_nodes() {
    let mut profiles = import::parse(VLESS).unwrap();
    profiles[0].subscription_id = Some("sub".into());
    let id = profiles[0].id.clone();
    let mut store = Store {
        profiles,
        selected_profile: Some(id.clone()),
        ..Default::default()
    };
    replace_subscription_profiles(
        &mut store,
        "sub",
        import::parse(&VLESS.replace("sid=1234", "sid=abcd")).unwrap(),
    );
    assert_eq!(store.selected_profile.as_deref(), Some(id.as_str()));
    assert_eq!(store.profiles.len(), 1);
    assert_eq!(
        store.profiles[0].outbound["streamSettings"]["realitySettings"]["shortId"],
        "abcd"
    );
}

#[test]
fn generated_configs_pass_real_core_validation_when_available() {
    let Ok(sing) = std::env::var("EASYZAPRET_TEST_SINGBOX") else {
        return;
    };
    let xray = std::env::var("EASYZAPRET_TEST_XRAY").unwrap();
    let folder = std::env::temp_dir().join(format!("easyzapret-vpn-test-{}", new_id()));
    std::fs::create_dir(&folder).unwrap();
    for key in [
        VLESS,
        "trojan://test@vpn.example.com:443?sni=example.com#Trojan",
        "hy2://test@vpn.example.com:443?sni=example.com#HY2",
    ] {
        let profile = import::parse(key).unwrap().remove(0);
        for mode in [Mode::All, Mode::OnlyApps, Mode::BypassApps, Mode::Proxy] {
            let settings = VpnSettings {
                mode,
                apps: vec!["Discord.exe".into()],
                ..Default::default()
            };
            let file = folder.join("sing.json");
            write_config(&file, &config::build(&settings, &profile).unwrap()).unwrap();
            let output = std::process::Command::new(&sing)
                .args(["check", "-c"])
                .arg(file)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if profile.engine == "xray" {
                let file = folder.join("xray.json");
                write_config(&file, &config::xray_config(&settings, &profile)).unwrap();
                let output = std::process::Command::new(&xray)
                    .args(["run", "-test", "-config"])
                    .arg(file)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stdout)
                );
            }
        }
    }
    std::fs::remove_dir_all(folder).unwrap();
}
