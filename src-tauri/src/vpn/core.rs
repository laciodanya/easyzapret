//! Xray-core process lifecycle.

use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::{logs, paths, util};

static VPN_CHILD: Mutex<Option<Child>> = Mutex::new(None);

pub fn is_core_installed() -> bool {
    paths::vpn_core_exe().exists()
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
        // Fallback: look for stray xray we own by config path in cmdline is hard;
        // just report based on our child handle.
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

    let mut cmd = util::hidden_command(exe.to_str().unwrap_or("xray"));
    cmd.arg("run")
        .arg("-c")
        .arg(&cfg_path)
        .current_dir(paths::vpn_core_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd.spawn().map_err(|e| format!("failed to start xray: {e}"))?;
    *VPN_CHILD.lock().unwrap() = Some(child);

    // Brief settle — if process dies immediately, surface failure.
    std::thread::sleep(Duration::from_millis(400));
    if !is_running() {
        return Err("vpn_core_exited".into());
    }
    logs::append("vpn", "Xray core started");
    Ok(())
}

pub fn stop() {
    let mut guard = VPN_CHILD.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        logs::append("vpn", "Xray core stopped");
    }
}
