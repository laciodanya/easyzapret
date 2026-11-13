use std::path::PathBuf;

#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
static WINDOWS_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

#[cfg(windows)]
pub(crate) fn legacy_data_dir() -> PathBuf {
    PathBuf::from("C:\\EasyZapret")
}

#[cfg(windows)]
pub(crate) fn set_windows_data_dir(path: PathBuf) -> std::io::Result<()> {
    if let Some(current) = WINDOWS_DATA_DIR.get() {
        return if current == &path {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "EasyZapret data directory is already fixed at {}",
                    current.display()
                ),
            ))
        };
    }
    WINDOWS_DATA_DIR.set(path).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "EasyZapret data directory was initialized concurrently",
        )
    })
}

/// Fixed data directory. Everything EasyZapret manages lives here so that the
/// path never contains cyrillic or special characters (zapret requirement).
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        WINDOWS_DATA_DIR
            .get()
            .cloned()
            .unwrap_or_else(legacy_data_dir)
    }
    #[cfg(not(windows))]
    {
        // Non-Windows builds exist only for development.
        dirs::home_dir().unwrap_or_default().join("EasyZapret")
    }
}

pub fn zapret_dir() -> PathBuf {
    data_dir().join("zapret")
}

pub fn zapret_bin_dir() -> PathBuf {
    zapret_dir().join("bin")
}

pub fn zapret_lists_dir() -> PathBuf {
    zapret_dir().join("lists")
}

pub fn zapret_utils_dir() -> PathBuf {
    zapret_dir().join("utils")
}

pub fn tg_dir() -> PathBuf {
    data_dir().join("tg-ws-proxy")
}

pub fn tg_exe() -> PathBuf {
    tg_dir().join("TgWsProxy_windows.exe")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn tmp_dir() -> PathBuf {
    data_dir().join("tmp")
}

pub fn settings_file() -> PathBuf {
    data_dir().join("config.json")
}

#[cfg(windows)]
fn reject_reparse_point(path: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("refusing to use reparse point: {}", path.display()),
                ));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok(())
}

/// Creates ordinary subdirectories after `storage::prepare` has replaced or
/// verified the protected root. The root itself is deliberately never created
/// here on Windows: fixing a predictable elevated directory after creation is
/// vulnerable to handles opened before the ACL change.
pub fn ensure_dirs() -> std::io::Result<()> {
    let root = data_dir();
    #[cfg(not(windows))]
    std::fs::create_dir_all(&root)?;
    #[cfg(windows)]
    {
        reject_reparse_point(&root)?;
        if !root.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "protected EasyZapret storage was not prepared",
            ));
        }
    }

    for dir in [zapret_dir(), tg_dir(), logs_dir(), tmp_dir()] {
        #[cfg(windows)]
        reject_reparse_point(&dir)?;
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
