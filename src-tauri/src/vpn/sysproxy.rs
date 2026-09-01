//! Windows system proxy helpers (Internet Settings registry).

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
use crate::logs;

#[cfg(windows)]
fn inet_settings() -> Result<RegKey, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Internet Settings",
        KEY_READ | KEY_WRITE,
    )
    .map_err(|e| e.to_string())
}

/// Enables system HTTP/HTTPS/SOCKS proxy pointing at local xray inbounds.
pub fn enable_system_proxy(http_port: u16, socks_port: u16) -> Result<(), String> {
    #[cfg(windows)]
    {
        let key = inet_settings()?;
        let server = format!("127.0.0.1:{http_port};socks=127.0.0.1:{socks_port}");
        key.set_value("ProxyEnable", &1u32)
            .map_err(|e| e.to_string())?;
        key.set_value("ProxyServer", &server)
            .map_err(|e| e.to_string())?;
        let _ = key.set_value(
            "ProxyOverride",
            &"<local>;localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.2*;172.3*;192.168.*;*.local",
        );
        logs::append("vpn", &format!("System proxy enabled → {server}"));
        notify_proxy_change();
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = (http_port, socks_port);
        Ok(())
    }
}

pub fn disable_system_proxy() {
    #[cfg(windows)]
    {
        if let Ok(key) = inet_settings() {
            let _ = key.set_value("ProxyEnable", &0u32);
            logs::append("vpn", "System proxy disabled");
            notify_proxy_change();
        }
    }
}

#[cfg(windows)]
fn notify_proxy_change() {
    let _ = crate::util::run_capture(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "Add-Type -Namespace WinINet -Name Native -MemberDefinition '[DllImport(\"wininet.dll\", SetLastError=true)] public static extern bool InternetSetOption(IntPtr h, int o, IntPtr b, int l);'; [WinINet.Native]::InternetSetOption([IntPtr]::Zero,39,[IntPtr]::Zero,0); [WinINet.Native]::InternetSetOption([IntPtr]::Zero,37,[IntPtr]::Zero,0) | Out-Null",
        ],
    );
}
