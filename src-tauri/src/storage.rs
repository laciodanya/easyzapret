//! Secure one-time migration for the fixed `C:\\EasyZapret` runtime path.
//!
//! The application runs elevated and writes executables below a predictable
//! path. Repairing an already existing directory in place is not sufficient:
//! an unprivileged process may have opened `WRITE_DAC` before elevation and
//! use that old handle after the ACL check. Instead we build a fresh directory
//! with its final DACL, move the old object aside, and move the new object into
//! its place. A Windows file ID in HKLM detects later identity replacement.

#[cfg(not(windows))]
use crate::AppState;

#[derive(Debug, Clone)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct RestorePlan {
    preserve_components: bool,
    service_owned: bool,
    service_was_running: bool,
    service_start_type: u32,
    manual_strategy: Option<String>,
    tg_was_running: bool,
    backup_path: Option<String>,
}

#[cfg(not(windows))]
pub fn prepare() -> std::io::Result<Option<RestorePlan>> {
    Ok(None)
}

#[cfg(not(windows))]
pub fn restore_after_prepare(_state: &AppState, _plan: Option<RestorePlan>) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
mod windows {
    use std::collections::HashSet;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    use rand::{rngs::OsRng, RngCore};
    use windows_sys::Win32::Foundation::{
        CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
        SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        AclSizeInformation, GetAce, GetAclInformation, IsWellKnownSid, WinBuiltinAdministratorsSid,
        WinLocalSystemSid, ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION,
        DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
        SECURITY_ATTRIBUTES,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FileRenameInfo, GetFileInformationByHandle, SetFileInformationByHandle,
        BY_HANDLE_FILE_INFORMATION, DELETE, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_READ_DATA,
        FILE_RENAME_INFO, FILE_SHARE_READ, FILE_SHARE_WRITE, READ_CONTROL,
    };
    use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, KEY_WRITE};
    use winreg::RegKey;

    use super::RestorePlan;
    use crate::{logs, paths, settings, tg_proxy, util, zapret, AppState};

    const REGISTRY_PATH: &str = r"SOFTWARE\EasyZapret\Storage";
    const STORAGE_SCHEMA: u32 = 1;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;
    const MAX_LIST_BYTES: u64 = 2 * 1024 * 1024;
    const EDITABLE_LISTS: &[&str] = &[
        "list-general-user.txt",
        "list-exclude-user.txt",
        "ipset-all.txt",
        "ipset-exclude-user.txt",
    ];

    static ROOT_GUARD: OnceLock<File> = OnceLock::new();

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct FileIdentity {
        volume_serial: u32,
        file_index: u64,
    }

    struct LocalDescriptor(PSECURITY_DESCRIPTOR);

    impl LocalDescriptor {
        fn from_sddl(sddl: &str) -> std::io::Result<Self> {
            let wide: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
            let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            if unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    wide.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self(descriptor))
        }
    }

    impl Drop for LocalDescriptor {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
    }

    struct MigrationMutex {
        handle: HANDLE,
        owned: bool,
    }

    impl MigrationMutex {
        fn acquire() -> std::io::Result<Self> {
            let descriptor = LocalDescriptor::from_sddl("O:BAG:BAD:P(A;;GA;;;SY)(A;;GA;;;BA)")?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.0,
                bInheritHandle: 0,
            };
            let name: Vec<u16> = "Global\\EasyZapret.StorageMigration.v1"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let handle = unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let wait = unsafe { WaitForSingleObject(handle, 60_000) };
            if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
                unsafe {
                    CloseHandle(handle);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "timed out waiting for another EasyZapret storage migration",
                ));
            }
            Ok(Self {
                handle,
                owned: true,
            })
        }
    }

    impl Drop for MigrationMutex {
        fn drop(&mut self) {
            unsafe {
                if self.owned {
                    ReleaseMutex(self.handle);
                }
                CloseHandle(self.handle);
            }
        }
    }

    fn registry_key() -> std::io::Result<RegKey> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let (key, _) =
            hklm.create_subkey_with_flags(REGISTRY_PATH, KEY_READ | KEY_WRITE | KEY_WOW64_64KEY)?;
        Ok(key)
    }

    fn random_suffix() -> String {
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let mut result = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write as _;
            let _ = write!(&mut result, "{byte:02x}");
        }
        result
    }

    fn create_secure_directory(path: &Path) -> std::io::Result<()> {
        use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;

        let descriptor = LocalDescriptor::from_sddl("O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)")?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { CreateDirectoryW(wide.as_ptr(), &attributes) } == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn create_stage() -> std::io::Result<PathBuf> {
        for _ in 0..16 {
            let path = PathBuf::from(format!(r"C:\EasyZapret.stage-{}", random_suffix()));
            match create_secure_directory(&path) {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique secure staging directory",
        ))
    }

    fn alternate_target() -> std::io::Result<PathBuf> {
        for _ in 0..16 {
            let path = PathBuf::from(format!(r"C:\EasyZapret.data-{}", random_suffix()));
            if !path_entry_exists(&path)? {
                return Ok(path);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique protected EasyZapret data path",
        ))
    }

    fn path_entry_exists(path: &Path) -> std::io::Result<bool> {
        match std::fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn is_access_denied(error: &std::io::Error) -> bool {
        error.raw_os_error() == Some(5)
            || error.kind() == std::io::ErrorKind::PermissionDenied
                && error.raw_os_error().is_none()
    }

    fn open_object(path: &Path, access: u32, share: u32) -> std::io::Result<File> {
        std::fs::OpenOptions::new()
            .access_mode(access)
            .share_mode(share)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }

    fn open_root_for_migration(path: &Path) -> std::io::Result<File> {
        open_object(
            path,
            DELETE | FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ,
        )
    }

    fn open_child_for_read(path: &Path) -> std::io::Result<File> {
        open_object(
            path,
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ,
        )
    }

    fn is_reparse(file: &File) -> std::io::Result<bool> {
        Ok(file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }

    fn file_identity_from_handle(file: &File) -> std::io::Result<FileIdentity> {
        let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, info.as_mut_ptr()) }
            == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let info = unsafe { info.assume_init() };
        Ok(FileIdentity {
            volume_serial: info.dwVolumeSerialNumber,
            file_index: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
        })
    }

    fn file_identity(path: &Path) -> std::io::Result<FileIdentity> {
        let file = open_object(
            path,
            FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
        )?;
        if is_reparse(&file)? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{} is a reparse point", path.display()),
            ));
        }
        file_identity_from_handle(&file)
    }

    fn path_security_is_trusted(file: &File) -> std::io::Result<bool> {
        let mut owner: PSID = std::ptr::null_mut();
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle() as HANDLE,
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                &mut dacl,
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(status as i32));
        }

        let result = (|| {
            if owner.is_null() {
                return Ok(false);
            }
            let sid_is_trusted = |sid: PSID| {
                !sid.is_null()
                    && (unsafe { IsWellKnownSid(sid, WinBuiltinAdministratorsSid) } != 0
                        || unsafe { IsWellKnownSid(sid, WinLocalSystemSid) } != 0)
            };
            if !sid_is_trusted(owner) || dacl.is_null() {
                return Ok(false);
            }

            let mut acl_info = ACL_SIZE_INFORMATION {
                AceCount: 0,
                AclBytesInUse: 0,
                AclBytesFree: 0,
            };
            if unsafe {
                GetAclInformation(
                    dacl,
                    (&mut acl_info as *mut ACL_SIZE_INFORMATION).cast(),
                    std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                    AclSizeInformation,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error());
            }

            const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
            const ACCESS_ALLOWED_OBJECT_ACE_TYPE: u8 = 5;
            const ACCESS_ALLOWED_CALLBACK_ACE_TYPE: u8 = 9;
            const ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE: u8 = 11;
            const DANGEROUS_ACCESS: u32 = 0x1000_0000
                | 0x4000_0000
                | 0x0001_0000
                | 0x0004_0000
                | 0x0008_0000
                | 0x0000_0002
                | 0x0000_0004
                | 0x0000_0010
                | 0x0000_0040
                | 0x0000_0100;

            for index in 0..acl_info.AceCount {
                let mut raw_ace = std::ptr::null_mut();
                if unsafe { GetAce(dacl, index, &mut raw_ace) } == 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let header = unsafe { &*(raw_ace.cast::<ACE_HEADER>()) };
                match header.AceType {
                    ACCESS_ALLOWED_ACE_TYPE | ACCESS_ALLOWED_CALLBACK_ACE_TYPE => {
                        let ace = unsafe { &*(raw_ace.cast::<ACCESS_ALLOWED_ACE>()) };
                        if ace.Mask & DANGEROUS_ACCESS != 0 {
                            let sid = std::ptr::addr_of!(ace.SidStart) as PSID;
                            if !sid_is_trusted(sid) {
                                return Ok(false);
                            }
                        }
                    }
                    ACCESS_ALLOWED_OBJECT_ACE_TYPE | ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE => {
                        let mask =
                            unsafe { raw_ace.cast::<u8>().add(4).cast::<u32>().read_unaligned() };
                        if mask & DANGEROUS_ACCESS != 0 {
                            return Ok(false);
                        }
                    }
                    _ => {}
                }
            }
            Ok(true)
        })();

        if !descriptor.is_null() {
            unsafe {
                LocalFree(descriptor);
            }
        }
        result
    }

    fn inspect_children(
        path: &Path,
        trusted: &mut bool,
        guards: &mut Vec<File>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let child = entry.path();
            let guard = open_child_for_read(&child)?;
            let reparse = is_reparse(&guard)?;
            if reparse || !path_security_is_trusted(&guard)? {
                *trusted = false;
            }
            let is_dir = guard.metadata()?.is_dir();
            guards.push(guard);
            if is_dir && !reparse {
                inspect_children(&child, trusted, guards)?;
            }
        }
        Ok(())
    }

    fn inspect_and_lock_tree(root: &Path, root_guard: &File) -> std::io::Result<(bool, Vec<File>)> {
        let root_reparse = is_reparse(root_guard)?;
        let mut trusted = !root_reparse && path_security_is_trusted(root_guard)?;
        let mut guards = Vec::new();
        if !root_reparse {
            inspect_children(root, &mut trusted, &mut guards)?;
        }
        Ok((trusted, guards))
    }

    fn copy_tree(source: &Path, destination: &Path, root_level: bool) -> std::io::Result<()> {
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            if root_level
                && entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("tmp")
            {
                continue;
            }
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("refusing to copy reparse point: {}", source_path.display()),
                ));
            }
            if kind.is_dir() {
                std::fs::create_dir(&destination_path)?;
                copy_tree(&source_path, &destination_path, false)?;
            } else if kind.is_file() {
                std::fs::copy(&source_path, &destination_path)?;
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unsupported file type: {}", source_path.display()),
                ));
            }
        }
        Ok(())
    }

    fn read_locked_file(path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
        let mut file = open_child_for_read(path)?;
        if is_reparse(&file)? || !file.metadata()?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("refusing non-regular file: {}", path.display()),
            ));
        }
        let size = file.metadata()?.len();
        if size > max_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{} is larger than the migration limit", path.display()),
            ));
        }
        let mut bytes = Vec::with_capacity(size as usize);
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    fn valid_strategy_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 160
            && !name.contains(['/', '\\', ':'])
            && !name.starts_with('.')
            && Path::new(name)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("bat"))
                .unwrap_or(false)
    }

    fn sanitize_settings(mut value: settings::Settings) -> settings::Settings {
        if !matches!(value.language.as_deref(), None | Some("ru") | Some("en")) {
            value.language = None;
        }
        if !matches!(value.theme.as_str(), "light" | "dark" | "system") {
            value.theme = "system".into();
        }
        if !value
            .selected_strategy
            .as_deref()
            .map(valid_strategy_name)
            .unwrap_or(true)
        {
            value.selected_strategy = None;
        }
        if !matches!(
            value.autopilot.policy.as_str(),
            "stability" | "speed" | "gaming"
        ) {
            value.autopilot.policy = "stability".into();
        }
        if !matches!(
            value.autopilot.switch_mode.as_str(),
            "rotate_policy" | "rotate_custom" | "best_by_tests"
        ) {
            value.autopilot.switch_mode = "rotate_policy".into();
        }
        value.autopilot.interval_minutes = value.autopilot.interval_minutes.clamp(1, 1440);
        value.autopilot.max_test_strategies = value.autopilot.max_test_strategies.clamp(1, 32);
        value.autopilot.min_health_percent = value.autopilot.min_health_percent.clamp(1, 100);
        value.autopilot.max_switches_per_hour = value.autopilot.max_switches_per_hour.clamp(1, 60);
        let mut seen = HashSet::new();
        value.autopilot.allowed_strategies.retain(|strategy| {
            valid_strategy_name(strategy) && seen.insert(strategy.to_ascii_lowercase())
        });
        value.autopilot.allowed_strategies.truncate(64);
        if value
            .last_seen_changelog_version
            .as_ref()
            .map(|version| version.len() > 64)
            .unwrap_or(false)
        {
            value.last_seen_changelog_version = None;
        }

        // An unsafe legacy tree may have been prepared by another local user.
        // Preserve harmless preferences and lists, but never execute components
        // or startup actions selected by that untrusted configuration.
        value.zapret_version = None;
        value.tg_version = None;
        value.autopilot.enabled = false;
        value.autostart.auto_start_zapret = false;
        value.autostart.auto_start_warp = false;
        value.autostart.auto_start_tg = false;
        value
    }

    fn copy_selective_legacy(source: &Path, destination: &Path) -> std::io::Result<()> {
        let config = [source.join("config.json"), source.join("config.json.bak")]
            .into_iter()
            .find_map(|path| {
                read_locked_file(&path, MAX_SETTINGS_BYTES)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<settings::Settings>(&bytes).ok())
            });
        if let Some(config) = config {
            let config = sanitize_settings(config);
            let bytes = serde_json::to_vec_pretty(&config)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination.join("config.json"))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }

        let zapret = source.join("zapret");
        let lists = zapret.join("lists");
        let zapret_guard = match open_child_for_read(&zapret) {
            Ok(guard) if !is_reparse(&guard)? && guard.metadata()?.is_dir() => guard,
            _ => return Ok(()),
        };
        let lists_guard = match open_child_for_read(&lists) {
            Ok(guard) if !is_reparse(&guard)? && guard.metadata()?.is_dir() => guard,
            _ => return Ok(()),
        };
        let _parent_guards = (zapret_guard, lists_guard);

        let destination_lists = destination.join("zapret").join("lists");
        std::fs::create_dir_all(&destination_lists)?;
        for name in EDITABLE_LISTS {
            if let Ok(bytes) = read_locked_file(&lists.join(name), MAX_LIST_BYTES) {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(destination_lists.join(name))?;
                file.write_all(&bytes)?;
                file.sync_all()?;
            }
        }
        Ok(())
    }

    fn rename_by_handle(file: &File, destination: &Path) -> std::io::Result<()> {
        let name: Vec<u16> = destination.as_os_str().encode_wide().collect();
        let name_offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
        let size = name_offset + name.len() * std::mem::size_of::<u16>();
        // FILE_RENAME_INFO is pointer-aligned and ends in a flexible UTF-16
        // array. A Vec<u8> would not guarantee sufficient alignment for the
        // struct cast, so allocate machine words and pass only the used bytes.
        let word_count = size.div_ceil(std::mem::size_of::<usize>());
        let mut buffer = vec![0usize; word_count];
        let bytes = buffer.as_mut_ptr().cast::<u8>();
        let info = bytes.cast::<FILE_RENAME_INFO>();
        unsafe {
            (*info).Anonymous.Flags = 0;
            (*info).RootDirectory = std::ptr::null_mut();
            (*info).FileNameLength = (name.len() * std::mem::size_of::<u16>()) as u32;
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                bytes.add(name_offset).cast::<u16>(),
                name.len(),
            );
        }
        if unsafe {
            SetFileInformationByHandle(
                file.as_raw_handle() as HANDLE,
                FileRenameInfo,
                buffer.as_ptr().cast(),
                size as u32,
            )
        } == 0
        {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn write_identity(key: &RegKey, identity: FileIdentity) -> std::io::Result<()> {
        key.set_value("VolumeSerial", &identity.volume_serial)?;
        key.set_value("FileIndex", &identity.file_index)?;
        Ok(())
    }

    fn read_identity(key: &RegKey) -> std::io::Result<FileIdentity> {
        Ok(FileIdentity {
            volume_serial: key.get_value("VolumeSerial")?,
            file_index: key.get_value("FileIndex")?,
        })
    }

    fn write_restore_plan(key: &RegKey, plan: &RestorePlan) -> std::io::Result<()> {
        key.set_value("RestorePending", &1u32)?;
        key.set_value("PreserveComponents", &(plan.preserve_components as u32))?;
        key.set_value("ServiceOwned", &(plan.service_owned as u32))?;
        key.set_value("ServiceWasRunning", &(plan.service_was_running as u32))?;
        key.set_value("ServiceStartType", &plan.service_start_type)?;
        key.set_value(
            "ManualStrategy",
            &plan.manual_strategy.clone().unwrap_or_default(),
        )?;
        key.set_value("TgWasRunning", &(plan.tg_was_running as u32))?;
        key.set_value(
            "RuntimeBackupPath",
            &plan.backup_path.clone().unwrap_or_default(),
        )?;
        Ok(())
    }

    fn read_restore_plan(key: &RegKey) -> std::io::Result<Option<RestorePlan>> {
        if key.get_value::<u32, _>("RestorePending").unwrap_or(0) != 1 {
            return Ok(None);
        }
        let manual: String = key.get_value("ManualStrategy").unwrap_or_default();
        let backup: String = key.get_value("RuntimeBackupPath").unwrap_or_default();
        Ok(Some(RestorePlan {
            preserve_components: key.get_value::<u32, _>("PreserveComponents").unwrap_or(0) == 1,
            service_owned: key.get_value::<u32, _>("ServiceOwned").unwrap_or(0) == 1,
            service_was_running: key.get_value::<u32, _>("ServiceWasRunning").unwrap_or(0) == 1,
            service_start_type: key.get_value("ServiceStartType").unwrap_or(3),
            manual_strategy: (!manual.is_empty()).then_some(manual),
            tg_was_running: key.get_value::<u32, _>("TgWasRunning").unwrap_or(0) == 1,
            backup_path: (!backup.is_empty()).then_some(backup),
        }))
    }

    fn write_pending(
        key: &RegKey,
        identity: FileIdentity,
        stage: &Path,
        target: &Path,
        backup: &Path,
        plan: &RestorePlan,
    ) -> std::io::Result<()> {
        key.set_value("Schema", &STORAGE_SCHEMA)?;
        key.set_value("State", &"pending")?;
        write_identity(key, identity)?;
        key.set_value("StagePath", &stage.to_string_lossy().to_string())?;
        key.set_value("TargetPath", &target.to_string_lossy().to_string())?;
        key.set_value("BackupPath", &backup.to_string_lossy().to_string())?;
        write_restore_plan(key, plan)
    }

    fn finalize_active(
        key: &RegKey,
        identity: FileIdentity,
        active_path: &Path,
        backup: Option<&Path>,
    ) -> std::io::Result<()> {
        key.set_value("Schema", &STORAGE_SCHEMA)?;
        key.set_value("State", &"active")?;
        write_identity(key, identity)?;
        key.set_value("ActivePath", &active_path.to_string_lossy().to_string())?;
        if let Some(backup) = backup {
            key.set_value("LastBackupPath", &backup.to_string_lossy().to_string())?;
        }
        for name in ["StagePath", "TargetPath", "BackupPath"] {
            let _ = key.delete_value(name);
        }
        Ok(())
    }

    fn clear_uncommitted_marker(key: &RegKey) {
        for name in [
            "Schema",
            "State",
            "VolumeSerial",
            "FileIndex",
            "ActivePath",
            "StagePath",
            "TargetPath",
            "BackupPath",
            "RestorePending",
            "PreserveComponents",
            "ServiceOwned",
            "ServiceWasRunning",
            "ServiceStartType",
            "ManualStrategy",
            "TgWasRunning",
            "RuntimeBackupPath",
        ] {
            let _ = key.delete_value(name);
        }
    }

    fn complete_restore_marker() -> std::io::Result<()> {
        let key = registry_key()?;
        key.set_value("RestorePending", &0u32)?;
        for name in [
            "PreserveComponents",
            "ServiceOwned",
            "ServiceWasRunning",
            "ServiceStartType",
            "ManualStrategy",
            "TgWasRunning",
            "RuntimeBackupPath",
        ] {
            let _ = key.delete_value(name);
        }
        Ok(())
    }

    fn marker_state(key: &RegKey) -> Option<String> {
        if key.get_value::<u32, _>("Schema").ok()? != STORAGE_SCHEMA {
            return None;
        }
        key.get_value("State").ok()
    }

    fn is_expected_random_path(path: &Path, kind: &str) -> bool {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let expected_prefix = format!("EasyZapret.{kind}-");
        path.parent() == Some(Path::new(r"C:\"))
            && name.starts_with(&expected_prefix)
            && name[expected_prefix.len()..].len() == 32
            && name[expected_prefix.len()..]
                .chars()
                .all(|character| character.is_ascii_hexdigit())
    }

    fn is_allowed_data_path(path: &Path) -> bool {
        path == paths::legacy_data_dir() || is_expected_random_path(path, "data")
    }

    fn install_root_guard(root: &Path, expected: FileIdentity) -> std::io::Result<()> {
        let guard = open_object(
            root,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
        )?;
        if is_reparse(&guard)?
            || file_identity_from_handle(&guard)? != expected
            || !path_security_is_trusted(&guard)?
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "{} identity or ACL does not match the protected storage marker",
                    root.display()
                ),
            ));
        }
        ROOT_GUARD.set(guard).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "EasyZapret storage was prepared twice in one process",
            )
        })
    }

    fn service_registry_key() -> Option<RegKey> {
        RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(
                r"SYSTEM\CurrentControlSet\Services\zapret",
                KEY_READ | KEY_WOW64_64KEY,
            )
            .ok()
    }

    fn managed_service_at(data_root: &Path) -> bool {
        let Some(key) = service_registry_key() else {
            return false;
        };
        let Ok(image) = key.get_value::<String, _>("ImagePath") else {
            return false;
        };
        let expected = data_root
            .join("zapret")
            .join("bin")
            .join("winws.exe")
            .to_string_lossy()
            .replace('/', "\\")
            .to_ascii_lowercase();
        let image = image.trim().replace('/', "\\").to_ascii_lowercase();
        image.starts_with(&format!("\"{expected}\""))
            || image == expected
            || image.starts_with(&format!("{expected} "))
    }

    fn service_start_type() -> u32 {
        service_registry_key()
            .and_then(|key| key.get_value("Start").ok())
            .unwrap_or(3)
    }

    fn set_service_start_type(start_type: u32) -> Result<(), String> {
        let value = match start_type {
            2 => "auto",
            3 => "demand",
            4 => "disabled",
            _ => "demand",
        };
        let (ok, output) = util::run_capture("sc", &["config", "zapret", "start=", value]);
        if ok {
            Ok(())
        } else {
            Err(format!(
                "failed to set zapret service start type to {value}: {}",
                output.trim()
            ))
        }
    }

    fn stage_selected_strategy(stage: &Path) -> Option<String> {
        let bytes = std::fs::read(stage.join("config.json")).ok()?;
        let settings: settings::Settings = serde_json::from_slice(&bytes).ok()?;
        settings
            .selected_strategy
            .filter(|name| valid_strategy_name(name))
    }

    fn capture_restore_plan(
        stage: &Path,
        preserve_components: bool,
        backup: Option<&Path>,
        service_root: &Path,
    ) -> RestorePlan {
        let service_owned = managed_service_at(service_root);
        let service_was_running = service_owned
            && zapret::service::query_service_state("zapret").as_deref() == Some("RUNNING");
        let manual_strategy =
            if preserve_components && !service_owned && zapret::process::winws_running() {
                stage_selected_strategy(stage).or_else(|| Some("general.bat".into()))
            } else {
                None
            };
        RestorePlan {
            preserve_components,
            service_owned,
            service_was_running,
            service_start_type: if service_owned {
                service_start_type()
            } else {
                3
            },
            manual_strategy,
            tg_was_running: preserve_components && tg_proxy::proxy_running(),
            backup_path: backup.map(|path| path.to_string_lossy().to_string()),
        }
    }

    fn stop_for_migration(plan: &RestorePlan) -> Result<(), String> {
        if plan.service_owned {
            set_service_start_type(4)?;
            let _ = util::run_capture("net", &["stop", "zapret"]);
            let _ = util::run_capture("sc", &["stop", "zapret"]);
        }
        zapret::process::force_kill_winws();
        for service in ["WinDivert", "WinDivert14"] {
            let _ = util::run_capture("net", &["stop", service]);
            let _ = util::run_capture("sc", &["stop", service]);
        }
        zapret::process::wait_for_winws_exit(Duration::from_secs(6));
        if zapret::process::winws_running() {
            return Err("winws.exe did not stop for the storage migration".into());
        }

        tg_proxy::kill_all();
        let deadline = Instant::now() + Duration::from_secs(5);
        while tg_proxy::proxy_running() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(200));
        }
        if tg_proxy::proxy_running() {
            return Err("TgWsProxy did not stop for the storage migration".into());
        }
        Ok(())
    }

    fn restore_runtime(state: &AppState, plan: &RestorePlan) -> Result<(), String> {
        if !plan.preserve_components {
            if plan.service_owned {
                let _ = util::run_capture("sc", &["delete", "zapret"]);
            }
            for service in ["WinDivert", "WinDivert14"] {
                let _ = util::run_capture("sc", &["delete", service]);
            }
            return Ok(());
        }

        if plan.service_owned {
            if paths::zapret_bin_dir().join("winws.exe").is_file() {
                set_service_start_type(plan.service_start_type)?;
                if plan.service_was_running {
                    zapret::service::start_installed_service()?;
                }
            } else {
                let (deleted, output) = util::run_capture("sc", &["delete", "zapret"]);
                if !deleted && zapret::service::query_service_state("zapret").is_some() {
                    return Err(format!(
                        "failed to remove orphaned zapret service: {}",
                        output.trim()
                    ));
                }
            }
        } else if let Some(strategy) = &plan.manual_strategy {
            zapret::process::start_strategy(state, strategy)?;
        }

        if plan.tg_was_running {
            tg_proxy::start_tg()?;
        }
        Ok(())
    }

    fn activate_empty_storage_at(
        key: &RegKey,
        target: &Path,
        service_root: &Path,
    ) -> std::io::Result<Option<RestorePlan>> {
        if !is_allowed_data_path(target) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "refusing unexpected EasyZapret data path: {}",
                    target.display()
                ),
            ));
        }
        let stage = create_stage()?;
        let stage_guard = open_root_for_migration(&stage)?;
        if is_reparse(&stage_guard)? || !path_security_is_trusted(&stage_guard)? {
            drop(stage_guard);
            let _ = std::fs::remove_dir_all(&stage);
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "new EasyZapret staging directory did not keep its protected ACL",
            ));
        }
        let expected = file_identity_from_handle(&stage_guard)?;
        let backup = PathBuf::from(format!(r"C:\EasyZapret.backup-{}", random_suffix()));
        let plan = capture_restore_plan(&stage, false, None, service_root);
        write_pending(key, expected, &stage, target, &backup, &plan)?;

        if let Err(error) = stop_for_migration(&plan) {
            clear_uncommitted_marker(key);
            drop(stage_guard);
            let _ = std::fs::remove_dir_all(&stage);
            return Err(std::io::Error::other(error));
        }
        if path_entry_exists(target)? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "protected EasyZapret target appeared during activation: {}",
                    target.display()
                ),
            ));
        }
        rename_by_handle(&stage_guard, target)?;
        if file_identity(target)? != expected {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "protected EasyZapret storage changed identity during activation",
            ));
        }
        paths::set_windows_data_dir(target.to_path_buf())?;
        finalize_active(key, expected, target, None)?;
        drop(stage_guard);
        install_root_guard(target, expected)?;
        Ok(Some(plan))
    }

    fn recover_pending(key: &RegKey) -> std::io::Result<Option<RestorePlan>> {
        let expected = read_identity(key)?;
        let stage = PathBuf::from(key.get_value::<String, _>("StagePath")?);
        let target = PathBuf::from(key.get_value::<String, _>("TargetPath")?);
        let backup = PathBuf::from(key.get_value::<String, _>("BackupPath")?);
        if !is_expected_random_path(&stage, "stage")
            || !is_expected_random_path(&backup, "backup")
            || !is_allowed_data_path(&target)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid pending storage migration paths in HKLM",
            ));
        }
        let plan = read_restore_plan(key)?.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "pending storage migration has no runtime restore plan",
            )
        })?;
        let root = target;

        if path_entry_exists(&root)? && file_identity(&root).ok() == Some(expected) {
            paths::set_windows_data_dir(root.clone())?;
            finalize_active(
                key,
                expected,
                &root,
                path_entry_exists(&backup)?.then_some(backup.as_path()),
            )?;
            install_root_guard(&root, expected)?;
            return Ok(Some(plan));
        }

        if !path_entry_exists(&stage)? || file_identity(&stage).ok() != Some(expected) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "cannot recover the pending EasyZapret migration: the secured staging directory is missing or changed",
            ));
        }
        let stage_guard = open_root_for_migration(&stage)?;
        if is_reparse(&stage_guard)? || !path_security_is_trusted(&stage_guard)? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "cannot recover the pending EasyZapret migration: staging ACL is unsafe",
            ));
        }

        if path_entry_exists(&root)? && path_entry_exists(&backup)? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "cannot recover the pending EasyZapret migration: both legacy and backup paths exist",
            ));
        }
        stop_for_migration(&plan).map_err(std::io::Error::other)?;
        if path_entry_exists(&root)? {
            let root_guard = open_root_for_migration(&root)?;
            rename_by_handle(&root_guard, &backup)?;
        }
        rename_by_handle(&stage_guard, &root)?;
        if file_identity(&root)? != expected {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "secured EasyZapret directory changed identity during recovery",
            ));
        }
        paths::set_windows_data_dir(root.clone())?;
        finalize_active(
            key,
            expected,
            &root,
            path_entry_exists(&backup)?.then_some(backup.as_path()),
        )?;
        drop(stage_guard);
        install_root_guard(&root, expected)?;
        Ok(Some(plan))
    }

    fn migrate_existing_or_create(key: &RegKey) -> std::io::Result<Option<RestorePlan>> {
        let root = paths::legacy_data_dir();
        let source_guard = if path_entry_exists(&root)? {
            match open_root_for_migration(&root) {
                Ok(guard) => Some(guard),
                Err(error) if is_access_denied(&error) => {
                    let alternate = alternate_target()?;
                    return activate_empty_storage_at(key, &alternate, &root);
                }
                Err(error) => {
                    return Err(std::io::Error::new(
                        error.kind(),
                        format!(
                            "cannot lock C:\\EasyZapret for secure migration; close other programs using it: {error}"
                        ),
                    ));
                }
            }
        } else {
            None
        };

        let (trusted_source, source_guards) = if let Some(source_guard) = &source_guard {
            match inspect_and_lock_tree(&root, source_guard) {
                Ok(result) => result,
                Err(error) if is_access_denied(&error) => {
                    let alternate = alternate_target()?;
                    return activate_empty_storage_at(key, &alternate, &root);
                }
                Err(error) => return Err(error),
            }
        } else {
            (true, Vec::new())
        };

        let stage = create_stage()?;
        let copy_result = if source_guard.is_some() {
            if trusted_source {
                copy_tree(&root, &stage, true)
            } else if source_guard
                .as_ref()
                .map(is_reparse)
                .transpose()?
                .unwrap_or(false)
            {
                Ok(())
            } else {
                copy_selective_legacy(&root, &stage)
            }
        } else {
            Ok(())
        };
        if let Err(error) = copy_result {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(error);
        }

        // Verify every destination object inherited the protected ACL. No ACL
        // is ever repaired in place; a failed check discards this private stage.
        let stage_guard = open_root_for_migration(&stage)?;
        let (stage_trusted, stage_guards) = inspect_and_lock_tree(&stage, &stage_guard)?;
        if !stage_trusted {
            drop(stage_guards);
            drop(stage_guard);
            let _ = std::fs::remove_dir_all(&stage);
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "new EasyZapret staging tree did not inherit the protected ACL",
            ));
        }
        drop(stage_guards);
        let expected = file_identity_from_handle(&stage_guard)?;
        let backup = PathBuf::from(format!(r"C:\EasyZapret.backup-{}", random_suffix()));
        let preserve_components = source_guard.is_some() && trusted_source;
        let plan = capture_restore_plan(
            &stage,
            preserve_components,
            source_guard.as_ref().map(|_| backup.as_path()),
            &root,
        );
        write_pending(key, expected, &stage, &root, &backup, &plan)?;

        // Existing objects were locked while copied. They no longer influence
        // the secured stage, and keeping descendant handles open would prevent
        // Windows from renaming the legacy directory into quarantine.
        drop(source_guards);

        if let Err(error) = stop_for_migration(&plan) {
            if plan.service_owned && plan.preserve_components {
                let _ = set_service_start_type(plan.service_start_type);
                if plan.service_was_running {
                    let _ = zapret::service::start_installed_service();
                }
            }
            clear_uncommitted_marker(key);
            drop(stage_guard);
            let _ = std::fs::remove_dir_all(&stage);
            return Err(std::io::Error::other(error));
        }

        if let Some(source_guard) = &source_guard {
            if let Err(error) = rename_by_handle(source_guard, &backup) {
                if plan.service_owned && plan.preserve_components {
                    let _ = set_service_start_type(plan.service_start_type);
                    if plan.service_was_running {
                        let _ = zapret::service::start_installed_service();
                    }
                }
                clear_uncommitted_marker(key);
                drop(stage_guard);
                let _ = std::fs::remove_dir_all(&stage);
                return Err(error);
            }
        }

        if let Err(error) = rename_by_handle(&stage_guard, &root) {
            // Keep the pending marker and both exact paths for crash recovery.
            // If possible, put the legacy object back so the machine is not
            // left without C:\EasyZapret while reporting the failure.
            if source_guard.is_some() && !path_entry_exists(&root).unwrap_or(true) {
                if let Ok(backup_guard) = open_root_for_migration(&backup) {
                    let _ = rename_by_handle(&backup_guard, &root);
                }
            }
            return Err(std::io::Error::new(
                error.kind(),
                format!("failed to activate protected EasyZapret storage: {error}"),
            ));
        }

        if file_identity(&root)? != expected {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "protected EasyZapret storage changed identity during activation",
            ));
        }
        finalize_active(
            key,
            expected,
            &root,
            source_guard.as_ref().map(|_| backup.as_path()),
        )?;
        drop(stage_guard);
        drop(source_guard);
        paths::set_windows_data_dir(root.clone())?;
        install_root_guard(&root, expected)?;
        Ok(Some(plan))
    }

    pub fn prepare() -> std::io::Result<Option<RestorePlan>> {
        let _mutex = MigrationMutex::acquire()?;
        let key = registry_key()?;

        match marker_state(&key).as_deref() {
            Some("active") => {
                let root = key
                    .get_value::<String, _>("ActivePath")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| paths::legacy_data_dir());
                if !is_allowed_data_path(&root) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "invalid EasyZapret data path stored in HKLM: {}",
                            root.display()
                        ),
                    ));
                }
                let expected = read_identity(&key)?;
                if !path_entry_exists(&root)? {
                    // A protected root can only be removed by an administrator.
                    // Recreate the same registered path through a random,
                    // atomically protected staging object.
                    return activate_empty_storage_at(&key, &root, &root);
                }
                let actual = file_identity(&root)?;
                if actual != expected {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!(
                            "{} was replaced after it was secured. Move the unexpected directory aside before starting EasyZapret.",
                            root.display()
                        ),
                    ));
                }
                paths::set_windows_data_dir(root.clone())?;
                install_root_guard(&root, expected)?;
                read_restore_plan(&key)
            }
            Some("pending") => recover_pending(&key),
            _ => migrate_existing_or_create(&key),
        }
    }

    pub fn restore_after_prepare(
        state: &AppState,
        plan: Option<RestorePlan>,
    ) -> Result<(), String> {
        let Some(plan) = plan else {
            return Ok(());
        };
        let backup_note = plan
            .backup_path
            .as_deref()
            .map(|path| format!("; legacy backup: {path}"))
            .unwrap_or_default();
        logs::append(
            "app",
            &format!(
                "secure storage migration activated (components preserved: {}){backup_note}",
                plan.preserve_components
            ),
        );
        restore_runtime(state, &plan)?;
        complete_restore_marker().map_err(|error| error.to_string())?;
        logs::append("app", "runtime state restored after storage migration");
        Ok(())
    }
}

#[cfg(windows)]
pub use windows::{prepare, restore_after_prepare};
