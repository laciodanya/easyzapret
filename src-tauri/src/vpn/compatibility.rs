use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};

use super::config::{TUN_V4, TUN_V6};

static PREPARING: AtomicBool = AtomicBool::new(false);

pub struct Preparing;

impl Preparing {
    pub fn begin() -> Self {
        PREPARING.store(true, Ordering::SeqCst);
        Self
    }
}

impl Drop for Preparing {
    fn drop(&mut self) {
        PREPARING.store(false, Ordering::SeqCst);
    }
}

pub fn required() -> bool {
    PREPARING.load(Ordering::SeqCst) || super::quick_status().running
}
use crate::{paths, util, AppState};

pub fn filter_path() -> std::path::PathBuf {
    paths::vpn_dir().join("winws-exclude.txt")
}

pub fn filter_argument() -> Option<String> {
    filter_path()
        .is_file()
        .then(|| format!("--wf-raw-part=@{}", filter_path().display()))
}

fn filter(servers: &[IpAddr], port: u16, interface_index: Option<u32>) -> String {
    let mut parts = vec![
        format!("not (ip and (ip.SrcAddr == {TUN_V4} or ip.DstAddr == {TUN_V4}))"),
        format!("not (ipv6 and (ipv6.SrcAddr == {TUN_V6} or ipv6.DstAddr == {TUN_V6}))"),
    ];
    if let Some(index) = interface_index {
        parts.push(format!("ifIdx != {index}"));
    }
    for ip in servers {
        let family = if ip.is_ipv4() { "ip" } else { "ipv6" };
        parts.push(format!("not ({family} and (({family}.DstAddr == {ip} and ((tcp and tcp.DstPort == {port}) or (udp and udp.DstPort == {port}))) or ({family}.SrcAddr == {ip} and ((tcp and tcp.SrcPort == {port}) or (udp and udp.SrcPort == {port})))))"));
    }
    parts.join(" and ")
}

fn restart_zapret(state: &AppState) -> Result<(), String> {
    if crate::zapret::service::query_service_state("zapret").as_deref() == Some("RUNNING") {
        crate::zapret::service::stop_installed_service(state)?;
        crate::zapret::service::start_installed_service()
    } else if crate::zapret::process::winws_running() {
        let strategy = util::lock_mutex(&state.current_strategy)
            .clone()
            .ok_or("vpn_external_zapret_restart_required")?;
        crate::zapret::process::stop(state);
        crate::zapret::process::start_strategy(state, &strategy)
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn patch_installed_service() -> Result<(), String> {
    use winreg::{enums::*, RegKey};
    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let service = match root.open_subkey("SYSTEM\\CurrentControlSet\\Services\\zapret") {
        Ok(key) => key,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("vpn_zapret_service_read_failed".into()),
    };
    let command: String = service
        .get_value("ImagePath")
        .map_err(|_| "vpn_zapret_service_read_failed")?;
    if command.contains("--wf-raw=") {
        return Err("vpn_zapret_custom_filter".into());
    }
    let argument = filter_argument().ok_or("vpn_filter_missing")?;
    if command.contains(&argument)
        || command.contains(&format!("--wf-raw-part=\"@{}\"", filter_path().display()))
    {
        return Ok(());
    }
    let argument = crate::zapret::strategy::service_args_string(&[argument]);
    let (ok, _) = util::run_capture(
        "sc",
        &[
            "config",
            "zapret",
            "binPath=",
            &format!("{command} {argument}"),
        ],
    );
    if ok {
        Ok(())
    } else {
        Err("vpn_zapret_service_update_failed".into())
    }
}

#[cfg(not(windows))]
fn patch_installed_service() -> Result<(), String> {
    Ok(())
}

pub fn prepare(
    state: &AppState,
    servers: &[IpAddr],
    port: u16,
    interface_index: Option<u32>,
) -> Result<(), String> {
    util::atomic_write(
        &filter_path(),
        filter(servers, port, interface_index).as_bytes(),
    )?;
    patch_installed_service()?;
    restart_zapret(state)
}

pub fn reset(state: &AppState, restart: bool) -> Result<(), String> {
    if !filter_path().exists() {
        return Ok(());
    }
    util::atomic_write(&filter_path(), filter(&[], 0, None).as_bytes())?;
    if restart {
        restart_zapret(state)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excludes_tunnel_and_only_the_remote_endpoint_ports() {
        let value = filter(
            &[
                "203.0.113.5".parse().unwrap(),
                "2001:db8::1".parse().unwrap(),
            ],
            443,
            Some(42),
        );
        assert!(value.contains("ip.SrcAddr == 172.29.231.1"));
        assert!(value.contains("ipv6.SrcAddr == fdfe:ea:5a::1"));
        assert!(value.contains("tcp.DstPort == 443"));
        assert!(value.contains("udp.SrcPort == 443"));
        assert!(value.contains("ifIdx != 42"));
        assert!(!filter(&[], 0, None).contains("203.0.113.5"));
    }
}
