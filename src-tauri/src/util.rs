use std::process::Command;

/// Creates a `Command` that never flashes a console window on Windows.
pub fn hidden_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Runs a command and returns (success, combined stdout+stderr).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn run_capture(program: &str, args: &[&str]) -> (bool, String) {
    match hidden_command(program).args(args).output() {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            (out.status.success(), text)
        }
        Err(e) => (false, format!("failed to run {program}: {e}")),
    }
}

/// Async variant of `hidden_command` based on tokio.
pub fn hidden_command_async(program: &str) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    {
        // tokio::process::Command exposes creation_flags without CommandExt.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Best-effort recursive delete. Prefers `remove_dir_all`; PowerShell only
/// if Windows still holds the tree (read-only / locked files).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn force_remove_dir(path: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => return Ok(()),
        Err(_) if !path.exists() => return Ok(()),
        Err(_) => {}
    }
    #[cfg(windows)]
    {
        let script = format!(
            "Remove-Item -LiteralPath '{}' -Recurse -Force -ErrorAction Stop",
            path.display()
        );
        let (ok, out) = run_capture(
            "powershell",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script],
        );
        if ok || !path.exists() {
            return Ok(());
        }
        return Err(format!("failed to remove {}: {}", path.display(), out.trim()));
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_dir_all(path).map_err(|e| e.to_string())
    }
}
