use super::types::Store;
use crate::util;

fn file() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(crate::paths::data_dir)
        .join("EasyZapret")
        .join("vpn.dat")
}

pub fn load() -> Result<Store, String> {
    let bytes = match std::fs::read(file()) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Store::default()),
        Err(_) => return Err("vpn_storage_read_failed".into()),
    };
    let bytes = decrypt(&bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| "vpn_storage_invalid".into())
}

pub fn save(store: &Store) -> Result<(), String> {
    let bytes = serde_json::to_vec(store).map_err(|_| "vpn_storage_invalid")?;
    util::atomic_write(&file(), &encrypt(&bytes)?).map_err(|_| "vpn_storage_write_failed".into())
}

#[cfg(windows)]
fn crypt(bytes: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        if protect {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err("vpn_storage_decryption_failed".into());
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(result)
}

#[cfg(windows)]
fn encrypt(bytes: &[u8]) -> Result<Vec<u8>, String> {
    crypt(bytes, true)
}
#[cfg(windows)]
fn decrypt(bytes: &[u8]) -> Result<Vec<u8>, String> {
    crypt(bytes, false)
}

// Development builds run without Windows DPAPI. VPN connections remain Windows-only.
#[cfg(not(windows))]
fn encrypt(bytes: &[u8]) -> Result<Vec<u8>, String> {
    Ok(bytes.to_vec())
}
#[cfg(not(windows))]
fn decrypt(bytes: &[u8]) -> Result<Vec<u8>, String> {
    Ok(bytes.to_vec())
}
