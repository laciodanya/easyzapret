use std::process::Command;

#[cfg(windows)]
pub fn windows_directory() -> std::io::Result<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetWindowsDirectoryW;

    let mut buffer = vec![0u16; 32_768];
    let len = unsafe { GetWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
    if len == 0 || len as usize >= buffer.len() {
        return Err(std::io::Error::last_os_error());
    }
    buffer.truncate(len as usize);
    Ok(std::path::PathBuf::from(std::ffi::OsString::from_wide(
        &buffer,
    )))
}

#[cfg(windows)]
pub fn windows_system32_program(name: &str) -> std::io::Result<std::path::PathBuf> {
    let windows = windows_directory()?;
    let filename = if std::path::Path::new(name).extension().is_none() {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let path = if filename.eq_ignore_ascii_case("powershell.exe") {
        windows
            .join("System32/WindowsPowerShell/v1.0")
            .join(filename)
    } else {
        windows.join("System32").join(filename)
    };
    if path.is_file() {
        Ok(path)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "trusted Windows system program not found: {}",
                path.display()
            ),
        ))
    }
}

#[cfg(windows)]
fn resolved_program(program: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(program);
    if path.is_absolute() || path.components().count() > 1 {
        path.to_path_buf()
    } else {
        // Never search the elevated process PATH for a bare executable name.
        windows_system32_program(program).unwrap_or_else(|_| {
            let mut impossible = std::path::PathBuf::from(r"\\?\NUL");
            impossible.push(program);
            impossible
        })
    }
}

/// Creates a `Command` that never flashes a console window on Windows.
pub fn hidden_command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = {
        #[cfg(windows)]
        {
            Command::new(resolved_program(program))
        }
        #[cfg(not(windows))]
        {
            Command::new(program)
        }
    };
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
    let mut cmd = {
        #[cfg(windows)]
        {
            tokio::process::Command::new(resolved_program(program))
        }
        #[cfg(not(windows))]
        {
            tokio::process::Command::new(program)
        }
    };
    #[cfg(windows)]
    {
        // tokio::process::Command exposes creation_flags without CommandExt.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Best-effort recursive delete. On Windows tries PowerShell `Remove-Item
/// -Force` first, which copes better with read-only files than `remove_dir_all`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn force_remove_dir(path: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let script = format!(
            "Remove-Item -LiteralPath '{}' -Recurse -Force -ErrorAction Stop",
            path.display()
        );
        let (ok, out) = run_capture(
            "powershell",
            &[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ],
        );
        if ok {
            return Ok(());
        }
        if !path.exists() {
            return Ok(());
        }
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("{e}; powershell: {}", out.trim()))
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_dir_all(path).map_err(|e| e.to_string())
    }
}
