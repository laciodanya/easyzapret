//! Low-level network probes built on the `curl` that ships with Windows 10/11.
//!
//! Shared by the autopilot, the home-screen service check and the quick
//! service tests, so every part of the app judges "does it work" the same way.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::paths;

pub fn curl_bin() -> &'static str {
    if cfg!(windows) {
        "curl.exe"
    } else {
        "curl"
    }
}

pub fn devnull() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

/// Size of the random body used by [`bulk`]; large enough to cross the
/// 16–20 KB point where many DPI systems freeze a connection.
pub const BULK_BYTES: usize = 65_536;

#[derive(Debug, Clone)]
pub struct HttpProbe {
    /// A real HTTP response arrived (any status). Blocked, reset or timed-out
    /// connections report `false`.
    pub ok: bool,
    pub code: u16,
    pub millis: u64,
    /// First line curl printed to stderr, empty on success.
    pub error: String,
}

/// HEAD request that only cares whether the TLS/HTTP exchange completes.
/// Status codes such as 403/404/421 still prove the host is reachable, which
/// is what matters for censorship checks.
pub async fn http(url: &str, timeout_secs: u64) -> HttpProbe {
    let started = Instant::now();
    let secs = timeout_secs.to_string();
    let mut cmd = crate::util::hidden_command_async(curl_bin());
    cmd.args([
        "-I",
        "-s",
        "--show-error",
        "--connect-timeout",
        &secs,
        "-m",
        &secs,
        "-o",
        devnull(),
        "-w",
        "%{http_code}",
        "--http1.1",
        url,
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let result = tokio::time::timeout(Duration::from_secs(timeout_secs + 3), cmd.output()).await;
    let millis = started.elapsed().as_millis() as u64;
    let Ok(Ok(out)) = result else {
        return HttpProbe {
            ok: false,
            code: 0,
            millis,
            error: "timeout".into(),
        };
    };

    let code: u16 = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    let error = String::from_utf8_lossy(&out.stderr)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches("curl: ")
        .to_string();
    HttpProbe {
        ok: out.status.success() && code > 0,
        code,
        millis,
        error,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkState {
    Ok,
    /// Upload started but the connection froze — the classic DPI throttle.
    Blocked,
    Failed,
}

#[derive(Debug, Clone)]
pub struct BulkProbe {
    pub state: BulkState,
    pub millis: u64,
}

/// Creates (once) the random payload file used for bulk probes.
pub fn ensure_payload() -> Option<PathBuf> {
    use rand::RngCore;
    let path = paths::tmp_dir().join("probe_payload.bin");
    if std::fs::metadata(&path)
        .map(|m| m.len() as usize == BULK_BYTES)
        .unwrap_or(false)
    {
        return Some(path);
    }
    std::fs::create_dir_all(paths::tmp_dir()).ok()?;
    let mut payload = vec![0u8; BULK_BYTES];
    rand::thread_rng().fill_bytes(&mut payload);
    std::fs::write(&path, payload).ok()?;
    Some(path)
}

/// POSTs a 64 KB random body to `https://host` and watches for a frozen
/// connection (bytes sent, nothing received until the timeout).
pub async fn bulk(host: &str, payload: &Path, timeout_secs: u64) -> BulkProbe {
    let secs = timeout_secs.to_string();
    let mut cmd = crate::util::hidden_command_async(curl_bin());
    cmd.args([
        "--range",
        &format!("0-{}", BULK_BYTES - 1),
        "-m",
        &secs,
        "--connect-timeout",
        &secs,
        "-w",
        "%{http_code} %{size_upload} %{size_download} %{time_total}",
        "-o",
        devnull(),
        "-X",
        "POST",
        "--data-binary",
        &format!("@{}", payload.display()),
        "-s",
    ])
    .arg(format!("https://{host}"))
    .stdout(Stdio::piped())
    .stderr(Stdio::null());

    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(timeout_secs + 4), cmd.output()).await;
    let millis = started.elapsed().as_millis() as u64;
    let Ok(Ok(out)) = result else {
        return BulkProbe {
            state: BulkState::Failed,
            millis,
        };
    };

    let exit = out.status.code().unwrap_or(-1);
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() != 4 {
        return BulkProbe {
            state: BulkState::Failed,
            millis,
        };
    }
    let up: i64 = parts[1].parse().unwrap_or(0);
    let down: i64 = parts[2].parse().unwrap_or(0);
    let time: f64 = parts[3].parse().unwrap_or(-1.0);

    let state = if up > 0 && down == 0 && time >= timeout_secs as f64 && exit != 0 {
        BulkState::Blocked
    } else if exit == 0 {
        BulkState::Ok
    } else {
        BulkState::Failed
    };
    BulkProbe { state, millis }
}

/// Plain-HTTP reachability of neutral hosts that are never blocked for
/// censorship reasons. Used to tell "my bypass broke" from "my internet is down".
pub async fn internet_reachable() -> bool {
    const NEUTRAL: &[&str] = &[
        "http://www.msftconnecttest.com/connecttest.txt",
        "http://connectivitycheck.gstatic.com/generate_204",
        "https://ya.ru",
    ];
    let probes = futures_util::future::join_all(NEUTRAL.iter().map(|u| http(u, 4))).await;
    probes.iter().any(|p| p.ok)
}
