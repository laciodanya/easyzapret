//! Local network identity used to remember which strategy worked on each
//! physical connection. Only a one-way hash is persisted; network names and
//! addresses never leave the machine.

use std::sync::Mutex;

static CURRENT_FINGERPRINT: Mutex<Option<String>> = Mutex::new(None);

pub fn current_fingerprint() -> Option<String> {
    CURRENT_FINGERPRINT.lock().ok()?.clone()
}

pub fn set_current_fingerprint(value: Option<String>) {
    if let Ok(mut current) = CURRENT_FINGERPRINT.lock() {
        *current = value;
    }
}

/// Fingerprints active Windows network profiles from their connection name,
/// adapter alias, default gateway and DNS servers. The PowerShell output is
/// reduced to a SHA-256 digest before Rust receives it.
#[cfg(windows)]
pub async fn fingerprint() -> Option<String> {
    tokio::task::spawn_blocking(|| {
        let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$items = @(Get-NetConnectionProfile | Where-Object {
  $_.IPv4Connectivity -ne 'Disconnected' -or $_.IPv6Connectivity -ne 'Disconnected'
} | ForEach-Object {
  $cfg = Get-NetIPConfiguration -InterfaceIndex $_.InterfaceIndex
  if ($null -ne $cfg) {
    $gateways = @($cfg.IPv4DefaultGateway | ForEach-Object { $_.NextHop } | Sort-Object)
    $dns = @($cfg.DnsServer.ServerAddresses | Sort-Object -Unique)
    [PSCustomObject][ordered]@{
      Name = $_.Name
      InterfaceAlias = $_.InterfaceAlias
      Gateways = $gateways
      DnsServers = $dns
    }
  }
} | Sort-Object Name, InterfaceAlias)
if ($items.Count -eq 0) { exit 2 }
$json = ConvertTo-Json -InputObject @($items) -Compress -Depth 4
$bytes = [Text.Encoding]::UTF8.GetBytes($json)
$sha = [Security.Cryptography.SHA256]::Create()
[Console]::Write(([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant())
"#;
        let powershell = crate::util::windows_system32_program("powershell.exe").ok()?;
        let output = crate::util::hidden_command(&powershell.to_string_lossy())
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_ascii_lowercase();
        (value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())).then_some(value)
    })
    .await
    .ok()
    .flatten()
}

#[cfg(not(windows))]
pub async fn fingerprint() -> Option<String> {
    None
}
