//! Xray-core process lifecycle.

use std::fs::File;
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::{logs, paths, util};

static VPN_CHILD: Mutex<Option<Child>> = Mutex::new(None);

pub fn is_core_installed() -> bool {
    paths::vpn_core_exe().exists()
}

pub fn wintun_available() -> bool {
    paths::vpn_core_dir().join("wintun.dll").exists()
}

fn xray_log_path() -> std::path::PathBuf {
    paths::logs_dir().join("xray.log")
}

pub fn is_running() -> bool {
    let mut guard = VPN_CHILD.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                false
            }
            Ok(None) => true,
            Err(_) => {
                *guard = None;
                false
            }
        }
    } else {
        false
    }
}

pub fn start(config_json: &str) -> Result<(), String> {
    stop();
    paths::ensure_dirs().map_err(|e| e.to_string())?;
    let exe = paths::vpn_core_exe();
    if !exe.exists() {
        return Err("vpn_core_not_installed".into());
    }
    let cfg_path = paths::vpn_runtime_config();
    std::fs::write(&cfg_path, config_json).map_err(|e| e.to_string())?;

    let log_path = xray_log_path();
    let log_file = File::create(&log_path).map_err(|e| e.to_string())?;
    let err_file = log_file.try_clone().map_err(|e| e.to_string())?;

    let mut cmd = util::hidden_command(exe.to_str().unwrap_or("xray"));
    cmd.arg("run")
        .arg("-c")
        .arg(&cfg_path)
        .current_dir(paths::vpn_core_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file));

    let child = cmd.spawn().map_err(|e| format!("failed to start xray: {e}"))?;
    *VPN_CHILD.lock().unwrap() = Some(child);

    std::thread::sleep(Duration::from_millis(1500));
    if !is_running() {
        logs::append("vpn", &format!("Xray failed: {}", last_log_tail(10)));
        return Err("vpn_core_exited".into());
    }
    logs::append("vpn", "Xray core started");
    Ok(())
}

pub fn inbound_listening(port: u16) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration as StdDuration;
    let Ok(mut addrs) = ("127.0.0.1", port).to_socket_addrs() else {
        return false;
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, StdDuration::from_millis(400)).is_ok()
}

pub fn last_log_tail(n: usize) -> String {
    let tail = std::fs::read_to_string(xray_log_path()).unwrap_or_default();
    tail.lines()
        .rev()
        .take(n)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
}

/// True when the Wintun adapter exists *and* the default route points at it.
/// An adapter without 0.0.0.0/0 means old Xray / failed autoSystemRoutingTable
/// — traffic never enters the tunnel.
pub fn tun_ready() -> bool {
    #[cfg(windows)]
    {
        std::thread::sleep(Duration::from_millis(2000));
        let (_, ifaces) = util::run_capture("netsh", &["interface", "show", "interface"]);
        let ifaces = ifaces.to_ascii_lowercase();
        if !(ifaces.contains("xray0") || ifaces.contains("wintun")) {
            return false;
        }
        let (_, routes) = util::run_capture("netsh", &["interface", "ipv4", "show", "route"]);
        routes.to_ascii_lowercase().lines().any(|line| {
            line.contains("0.0.0.0/0") && (line.contains("xray0") || line.contains("wintun"))
        })
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn stop() {
    let mut guard = VPN_CHILD.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        logs::append("vpn", "Xray core stopped");
    }
}
