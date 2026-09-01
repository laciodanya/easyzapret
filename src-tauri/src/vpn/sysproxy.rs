//! Windows system proxy (WinINET + WinHTTP) so browsers actually use Xray.

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
use crate::logs;
use crate::paths;
#[cfg(windows)]
use crate::util;

#[cfg(windows)]
fn inet_settings() -> Result<RegKey, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Internet Settings",
        KEY_READ | KEY_WRITE,
    )
    .map_err(|e| e.to_string())
}

#[cfg_attr(not(windows), allow(dead_code))]
fn proxy_server(http_port: u16, socks_port: u16) -> String {
    format!("http=127.0.0.1:{http_port};https=127.0.0.1:{http_port};socks=127.0.0.1:{socks_port}")
}

fn write_pac(http_port: u16, socks_port: u16) -> Result<std::path::PathBuf, String> {
    paths::ensure_dirs().map_err(|e| e.to_string())?;
    let path = paths::vpn_dir().join("proxy.pac");
    let pac = format!(
        r#"function FindProxyForURL(url, host) {{
  if (isPlainHostName(host) || host === "127.0.0.1" || host === "localhost") return "DIRECT";
  if (shExpMatch(host, "*.local")) return "DIRECT";
  if (isInNet(host, "10.0.0.0", "255.0.0.0")) return "DIRECT";
  if (isInNet(host, "172.16.0.0", "255.240.0.0")) return "DIRECT";
  if (isInNet(host, "192.168.0.0", "255.255.0.0")) return "DIRECT";
  if (isInNet(host, "169.254.0.0", "255.255.0.0")) return "DIRECT";
  return "PROXY 127.0.0.1:{http}; SOCKS5 127.0.0.1:{socks}; DIRECT";
}}
"#,
        http = http_port,
        socks = socks_port
    );
    std::fs::write(&path, pac).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Enables system HTTP/HTTPS/SOCKS proxy pointing at local xray inbounds.
pub fn enable_system_proxy(http_port: u16, socks_port: u16) -> Result<(), String> {
    #[cfg(windows)]
    {
        let key = inet_settings()?;
        let server = proxy_server(http_port, socks_port);
        key.set_value("ProxyEnable", &1u32)
            .map_err(|e| e.to_string())?;
        key.set_value("ProxyServer", &server)
            .map_err(|e| e.to_string())?;
        let _ = key.set_value("AutoDetect", &0u32);
        let _ = key.delete_value("AutoConfigURL");
        let _ = key.set_value(
            "ProxyOverride",
            &"<local>;localhost;127.*;10.*;172.16.*;192.168.*;*.local",
        );
        let _ = write_pac(http_port, socks_port);
        logs::append("vpn", &format!("System proxy enabled → {server}"));
        notify_proxy_change();
        let _ = util::run_capture("netsh", &["winhttp", "import", "proxy", "source=ie"]);
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = write_pac(http_port, socks_port);
        let _ = (http_port, socks_port);
        Ok(())
    }
}

pub fn disable_system_proxy() {
    #[cfg(windows)]
    {
        if let Ok(key) = inet_settings() {
            let _ = key.set_value("ProxyEnable", &0u32);
            let _ = key.set_value("AutoDetect", &0u32);
            let _ = key.delete_value("AutoConfigURL");
            logs::append("vpn", "System proxy disabled");
            notify_proxy_change();
        }
        let _ = util::run_capture("netsh", &["winhttp", "reset", "proxy"]);
    }
}

#[cfg(windows)]
fn notify_proxy_change() {
    let _ = util::run_capture(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Add-Type -Namespace WinINet -Name Native -MemberDefinition '[DllImport(\"wininet.dll\", SetLastError=true)] public static extern bool InternetSetOption(IntPtr h, int o, IntPtr b, int l);'; [WinINet.Native]::InternetSetOption([IntPtr]::Zero,39,[IntPtr]::Zero,0); [WinINet.Native]::InternetSetOption([IntPtr]::Zero,37,[IntPtr]::Zero,0) | Out-Null",
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::proxy_server;

    #[test]
    fn proxy_server_uses_per_scheme_syntax() {
        let s = proxy_server(10809, 10808);
        assert!(s.contains("http=127.0.0.1:10809"));
        assert!(s.contains("https=127.0.0.1:10809"));
        assert!(s.contains("socks=127.0.0.1:10808"));
        assert!(!s.starts_with("127.0.0.1:10809;socks="));
    }
}
