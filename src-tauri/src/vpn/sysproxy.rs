//! Windows system proxy via WinINet, same approach as v2rayN / Happ:
//! `InternetSetOption(INTERNET_OPTION_PER_CONNECTION_OPTION)` then refresh.
//! No PowerShell, no extra helper EXE (those trip Defender heuristics).

#[cfg(windows)]
use std::mem::{size_of, ManuallyDrop};

#[cfg(windows)]
use crate::logs;

#[cfg_attr(not(windows), allow(dead_code))]
fn proxy_server(http_port: u16, _socks_port: u16) -> String {
    // HTTP(S) only — WinINET `socks=` is SOCKS4 and Chrome ignores it.
    format!("http=127.0.0.1:{http_port};https=127.0.0.1:{http_port}")
}

#[cfg_attr(not(windows), allow(dead_code))]
const BYPASS: &str = "<local>;localhost;127.*;10.*;172.16.*;192.168.*;*.local";

/// Enables system HTTP/HTTPS proxy pointing at local Xray HTTP inbound.
pub fn enable_system_proxy(http_port: u16, socks_port: u16) -> Result<(), String> {
    let server = proxy_server(http_port, socks_port);
    #[cfg(windows)]
    {
        match set_wininet_proxy(Some(&server), Some(BYPASS)) {
            Ok(()) => logs::append("vpn", &format!("System proxy enabled → {server}")),
            Err(e) => {
                logs::append("vpn", &format!("WinINet API failed ({e}), using registry"));
                registry_fallback(true, Some(&server))?;
                notify_wininet();
            }
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = (http_port, socks_port, server);
        Ok(())
    }
}

pub fn disable_system_proxy() {
    #[cfg(windows)]
    {
        if let Err(e) = set_wininet_proxy(None, None) {
            logs::append("vpn", &format!("WinINet disable failed ({e}), using registry"));
            let _ = registry_fallback(false, None);
            notify_wininet();
        } else {
            logs::append("vpn", "System proxy disabled");
        }
    }
}

#[cfg(windows)]
fn registry_fallback(enable: bool, server: Option<&str>) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            KEY_READ | KEY_WRITE,
        )
        .map_err(|e| e.to_string())?;
    key.set_value("ProxyEnable", &(if enable { 1u32 } else { 0u32 }))
        .map_err(|e| e.to_string())?;
    let _ = key.set_value("AutoDetect", &0u32);
    let _ = key.delete_value("AutoConfigURL");
    if enable {
        if let Some(s) = server {
            key.set_value("ProxyServer", &s).map_err(|e| e.to_string())?;
        }
        let _ = key.set_value("ProxyOverride", &BYPASS);
    }
    Ok(())
}

#[cfg(windows)]
fn notify_wininet() {
    use std::ptr::null;
    use windows_sys::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_PROXY_SETTINGS_CHANGED, INTERNET_OPTION_REFRESH,
        INTERNET_OPTION_SETTINGS_CHANGED,
    };
    unsafe {
        let _ = InternetSetOptionW(null(), INTERNET_OPTION_SETTINGS_CHANGED, null(), 0);
        let _ = InternetSetOptionW(null(), INTERNET_OPTION_PROXY_SETTINGS_CHANGED, null(), 0);
        let _ = InternetSetOptionW(null(), INTERNET_OPTION_REFRESH, null(), 0);
    }
}

#[cfg(windows)]
fn set_wininet_proxy(server: Option<&str>, bypass: Option<&str>) -> Result<(), String> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_PER_CONNECTION_OPTION,
        INTERNET_OPTION_PROXY_SETTINGS_CHANGED, INTERNET_OPTION_REFRESH,
        INTERNET_PER_CONN_FLAGS, INTERNET_PER_CONN_OPTIONW, INTERNET_PER_CONN_OPTIONW_0,
        INTERNET_PER_CONN_OPTION_LISTW, INTERNET_PER_CONN_PROXY_BYPASS,
        INTERNET_PER_CONN_PROXY_SERVER, PROXY_TYPE_DIRECT, PROXY_TYPE_PROXY,
    };

    let mut server_w: Vec<u16> = server
        .unwrap_or("")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut bypass_w: Vec<u16> = bypass
        .unwrap_or("")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let flags = if server.is_some() {
        PROXY_TYPE_DIRECT | PROXY_TYPE_PROXY
    } else {
        PROXY_TYPE_DIRECT
    };

    let mut opts: Vec<INTERNET_PER_CONN_OPTIONW> = Vec::with_capacity(3);
    opts.push(INTERNET_PER_CONN_OPTIONW {
        dwOption: INTERNET_PER_CONN_FLAGS,
        Value: INTERNET_PER_CONN_OPTIONW_0 { dwValue: flags },
    });
    if server.is_some() {
        opts.push(INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_SERVER,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                pszValue: server_w.as_mut_ptr(),
            },
        });
        opts.push(INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_BYPASS,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                pszValue: bypass_w.as_mut_ptr(),
            },
        });
    }

    let mut opts = ManuallyDrop::new(opts);
    let list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        pszConnection: null_mut(),
        dwOptionCount: opts.len() as u32,
        dwOptionError: 0,
        pOptions: opts.as_mut_ptr(),
    };

    unsafe {
        let ok = InternetSetOptionW(
            null(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            (&list as *const INTERNET_PER_CONN_OPTION_LISTW).cast(),
            size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        );
        if ok == 0 {
            ManuallyDrop::drop(&mut opts);
            return Err(format!("InternetSetOption {}", std::io::Error::last_os_error()));
        }
        let _ = InternetSetOptionW(null(), INTERNET_OPTION_PROXY_SETTINGS_CHANGED, null(), 0);
        let _ = InternetSetOptionW(null(), INTERNET_OPTION_REFRESH, null(), 0);
        ManuallyDrop::drop(&mut opts);
    }
    let _ = (server_w, bypass_w);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::proxy_server;

    #[test]
    fn proxy_server_is_http_https() {
        let s = proxy_server(10809, 10808);
        assert!(s.contains("http=127.0.0.1:10809"));
        assert!(s.contains("https=127.0.0.1:10809"));
        assert!(!s.contains("socks="));
    }
}
