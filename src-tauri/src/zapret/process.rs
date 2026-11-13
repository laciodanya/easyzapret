use std::io::BufRead;
use std::process::Stdio;

use sysinfo::{Process, ProcessesToUpdate, System};
use tauri::State;

use crate::zapret::strategy;
use crate::{logs, paths, AppState};

fn normalized_exe(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn is_managed_winws(process: &Process) -> bool {
    if !process
        .name()
        .to_string_lossy()
        .eq_ignore_ascii_case("winws.exe")
    {
        return false;
    }
    process
        .exe()
        .map(|exe| {
            normalized_exe(exe) == normalized_exe(&paths::zapret_bin_dir().join("winws.exe"))
        })
        .unwrap_or(false)
}

pub fn winws_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes().values().any(is_managed_winws)
}

pub fn kill_all_winws() {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    for p in sys.processes().values() {
        if is_managed_winws(p) {
            p.kill();
        }
    }
}

/// Force-terminates every winws.exe instance. On Windows `taskkill /F` is
/// more reliable than sysinfo::Process::kill for processes holding WinDivert
/// and cygwin DLLs in zapret/bin.
pub fn force_kill_winws() {
    #[cfg(windows)]
    {
        for _ in 0..4 {
            let mut sys = System::new();
            sys.refresh_processes(ProcessesToUpdate::All, true);
            for process in sys.processes().values().filter(|p| is_managed_winws(p)) {
                let pid = process.pid().as_u32().to_string();
                let _ = crate::util::run_capture("taskkill", &["/PID", &pid, "/T", "/F"]);
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
            if !winws_running() {
                return;
            }
        }
    }
    kill_all_winws();
}

/// Blocks until winws.exe is gone or `max_wait` elapses.
pub fn wait_for_winws_exit(max_wait: std::time::Duration) {
    let deadline = std::time::Instant::now() + max_wait;
    while std::time::Instant::now() < deadline {
        if !winws_running() {
            return;
        }
        force_kill_winws();
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
}

/// Starts winws.exe with the arguments extracted from the selected strategy
/// bat. Mirrors running `general*.bat` manually, minus the console window.
pub fn start_strategy(state: &AppState, bat_name: &str) -> Result<(), String> {
    let exe = paths::zapret_bin_dir().join("winws.exe");
    if !exe.exists() {
        return Err("winws.exe not found — install zapret first".into());
    }
    if winws_running() {
        return Err("already_running".into());
    }
    // service.bat refuses to run the standalone bat while the service is active.
    let svc = super::service::query_service_state("zapret");
    if svc.as_deref() == Some("RUNNING") {
        return Err("service_running".into());
    }

    // winws references *-user.txt lists that the release does not ship.
    strategy::ensure_user_lists().map_err(|e| format!("failed to create user lists: {e}"))?;

    let args = strategy::winws_args(bat_name)?;
    logs::append("zapret", &format!("Starting strategy: {bat_name}"));
    logs::append("zapret", &format!("winws args: {}", args.join(" ")));

    let mut cmd = crate::util::hidden_command(&exe.to_string_lossy());
    cmd.args(&args)
        .current_dir(paths::zapret_bin_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        logs::error("zapret", &format!("failed to start winws.exe: {e}"));
        format!("failed to start winws.exe: {e}")
    })?;
    logs::append("zapret", &format!("winws.exe started (pid {})", child.id()));

    // Pipe winws output into zapret.log for the in-app log viewer.
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout)
                .lines()
                .map_while(Result::ok)
            {
                logs::append_raw("zapret", &line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr)
                .lines()
                .map_while(Result::ok)
            {
                logs::append_raw("zapret", &line);
            }
        });
    }

    *state.zapret_child.lock().unwrap() = Some(child);
    *state.current_strategy.lock().unwrap() = Some(bat_name.to_string());

    // winws exits within a second or two when its args or the driver are wrong.
    // Record that fact so a log shared on GitHub explains "it just turns off".
    let watch = bat_name.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(4));
        if !winws_running() {
            logs::error(
                "zapret",
                &format!("winws.exe is not running 4s after start of '{watch}' — it exited early (check the output above)"),
            );
        }
    });
    Ok(())
}

pub fn stop(state: &AppState) {
    if let Some(mut child) = state.zapret_child.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    // Also stop instances started outside of the app (parity with taskkill).
    kill_all_winws();
    *state.current_strategy.lock().unwrap() = None;
    logs::append("zapret", "winws.exe stopped");
}

#[tauri::command]
pub fn start_zapret(state: State<'_, AppState>, strategy_file: String) -> Result<(), String> {
    if super::service::query_service_state("zapret").is_some() {
        return super::service::start_installed_service();
    }
    start_strategy(&state, &strategy_file)
}

#[tauri::command]
pub fn stop_zapret(state: State<'_, AppState>) -> Result<(), String> {
    if super::service::query_service_state("zapret").is_some() {
        super::service::stop_installed_service(&state)
    } else {
        stop(&state);
        Ok(())
    }
}
