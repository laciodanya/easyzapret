use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{path::BaseDirectory, AppHandle, Emitter, Manager};

use crate::{logs, paths};

#[derive(Deserialize)]
struct Asset {
    name: String,
    version: String,
    archive: String,
    url: String,
    size: u64,
    sha256: String,
    files: Vec<String>,
}

static ASSETS: LazyLock<Vec<Asset>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../vpn-assets.json")).expect("invalid VPN asset catalog")
});

#[derive(Clone, Serialize)]
struct Progress<'a> {
    phase: &'a str,
    asset: &'a str,
    downloaded: u64,
    total: u64,
}

fn progress(app: &AppHandle, phase: &str, asset: &Asset, downloaded: u64) {
    let _ = app.emit(
        "vpn-install-progress",
        Progress {
            phase,
            asset: &asset.name,
            downloaded,
            total: asset.size,
        },
    );
}

#[derive(Debug)]
struct InstallError {
    code: &'static str,
    detail: String,
    retry: bool,
}

impl InstallError {
    fn new(code: &'static str, detail: impl ToString) -> Self {
        Self {
            code,
            detail: detail.to_string(),
            retry: false,
        }
    }

    fn network(error: reqwest::Error) -> Self {
        Self {
            code: if error.is_timeout() {
                "vpn_install_timeout"
            } else {
                "vpn_install_network"
            },
            // Redirect URLs can contain signed query parameters.
            detail: error.without_url().to_string(),
            retry: true,
        }
    }

    fn disk(error: io::Error) -> Self {
        Self::new("vpn_install_disk", error)
    }
}

type InstallResult<T> = Result<T, InstallError>;

struct VerifiedArchive<'a> {
    asset: &'a Asset,
    file: File,
    size: u64,
    hash: Sha256,
}

impl<'a> VerifiedArchive<'a> {
    fn create(asset: &'a Asset, destination: &Path) -> InstallResult<Self> {
        let file = File::create(destination).map_err(InstallError::disk)?;
        Ok(Self {
            asset,
            file,
            size: 0,
            hash: Sha256::new(),
        })
    }

    fn write(&mut self, chunk: &[u8]) -> InstallResult<()> {
        self.size += chunk.len() as u64;
        if self.size > self.asset.size {
            return Err(InstallError::new(
                "vpn_install_integrity",
                "archive exceeds pinned size",
            ));
        }
        self.file.write_all(chunk).map_err(InstallError::disk)?;
        self.hash.update(chunk);
        Ok(())
    }

    fn finish(self) -> InstallResult<()> {
        if self.size != self.asset.size {
            return Err(InstallError::new(
                "vpn_install_incomplete",
                format!("received {} of {} bytes", self.size, self.asset.size),
            ));
        }
        if format!("{:x}", self.hash.finalize()) != self.asset.sha256 {
            return Err(InstallError::new(
                "vpn_install_integrity",
                "SHA-256 mismatch",
            ));
        }
        self.file.sync_all().map_err(InstallError::disk)
    }
}

fn copy_bundled(asset: &Asset, source: &Path, destination: &Path) -> InstallResult<()> {
    let mut source = io::BufReader::new(File::open(source).map_err(InstallError::disk)?);
    let mut archive = VerifiedArchive::create(asset, destination)?;
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = source.read(&mut buffer).map_err(InstallError::disk)?;
        if read == 0 {
            return archive.finish();
        }
        archive.write(&buffer[..read])?;
    }
}

async fn download(
    app: &AppHandle,
    asset: &Asset,
    destination: &Path,
    direct: bool,
) -> InstallResult<()> {
    let builder = reqwest::Client::builder()
        .https_only(true)
        .user_agent(concat!("EasyZapret/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(100))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && super::trusted_github_host(attempt.url()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }));
    let client = if direct { builder.no_proxy() } else { builder }
        .build()
        .map_err(InstallError::network)?;
    let mut response = client
        .get(&asset.url)
        .send()
        .await
        .map_err(InstallError::network)?;
    let status = response.status();
    if !status.is_success() {
        return Err(InstallError {
            code: "vpn_install_http",
            detail: format!("GitHub HTTP {status}"),
            retry: status.as_u16() == 403 || status.as_u16() == 429 || status.is_server_error(),
        });
    }
    if response
        .content_length()
        .is_some_and(|size| size != asset.size)
    {
        return Err(InstallError::new(
            "vpn_install_integrity",
            "unexpected Content-Length",
        ));
    }
    let mut archive = VerifiedArchive::create(asset, destination)?;
    let mut last_update = Instant::now();
    while let Some(chunk) = response.chunk().await.map_err(InstallError::network)? {
        archive.write(&chunk)?;
        if last_update.elapsed() >= Duration::from_millis(150) {
            progress(app, "downloading", asset, archive.size);
            last_update = Instant::now();
        }
    }
    archive.finish().map_err(|mut error| {
        error.retry = error.code == "vpn_install_incomplete";
        error
    })
}

fn unpack(asset: &Asset, archive: &Path, staging: &Path, prepared: &Path) -> InstallResult<()> {
    let extracted = staging.join(&asset.name);
    super::extract_zip(archive, &extracted)
        .map_err(|error| InstallError::new("vpn_install_extract", error))?;
    for name in &asset.files {
        let destination = if name == "LICENSE" {
            prepared.join(format!("{}.LICENSE", asset.name))
        } else {
            prepared.join(name)
        };
        let source = extracted.join(name);
        if !source.is_file() {
            return Err(InstallError::new(
                "vpn_install_extract",
                format!("missing {name}"),
            ));
        }
        fs::rename(source, destination).map_err(InstallError::disk)?;
    }
    Ok(())
}

fn replace(prepared: &Path, target: &Path, backup: &Path) -> InstallResult<()> {
    // Older EasyZapret versions included this path in the zapret service command.
    let filter = target.join("winws-exclude.txt");
    if filter.is_file() {
        fs::copy(filter, prepared.join("winws-exclude.txt")).map_err(InstallError::disk)?;
    }
    let had_previous = target.exists();
    if had_previous {
        fs::rename(target, backup).map_err(InstallError::disk)?;
    }
    if let Err(error) = fs::rename(prepared, target) {
        if had_previous {
            fs::rename(backup, target).map_err(|restore| {
                InstallError::new(
                    "vpn_install_rollback",
                    format!("{error}; rollback: {restore}"),
                )
            })?;
        }
        return Err(InstallError::disk(error));
    }
    if had_previous {
        let _ = fs::remove_dir_all(backup);
    }
    Ok(())
}

pub(super) async fn install(app: &AppHandle) -> Result<String, String> {
    if !cfg!(windows) {
        return Err("vpn_windows_only".into());
    }
    let _operation = crate::vpn::OPERATION.try_lock().map_err(|_| "vpn_busy")?;
    if crate::vpn::quick_status().running {
        return Err("vpn_disconnect_to_install".into());
    }
    // Staging and backup are siblings of the protected destination on the same volume.
    let staging = paths::data_dir().join(format!(".vpn-install-{}", rand::random::<u64>()));
    let backup = paths::data_dir().join(format!(".vpn-backup-{}", rand::random::<u64>()));
    let result = install_inner(app, &staging, &backup).await;
    let _ = fs::remove_dir_all(&staging);
    result.map_err(|error| {
        logs::error("vpn", &format!("{}: {}", error.code, error.detail));
        error.code.into()
    })
}

async fn install_inner(app: &AppHandle, staging: &Path, backup: &Path) -> InstallResult<String> {
    fs::create_dir(staging).map_err(InstallError::disk)?;
    let prepared = staging.join("prepared");
    fs::create_dir(&prepared).map_err(InstallError::disk)?;
    for asset in ASSETS.iter() {
        let archive = staging.join(&asset.archive);
        let bundled = app
            .path()
            .resolve(
                Path::new("vpn-components").join(&asset.archive),
                BaseDirectory::Resource,
            )
            .ok()
            .filter(|path| path.is_file());
        if let Some(bundled) = bundled {
            progress(app, "verifying", asset, 0);
            copy_bundled(asset, &bundled, &archive)?;
        } else {
            progress(app, "downloading", asset, 0);
            if let Err(error) = download(app, asset, &archive, false).await {
                if !error.retry {
                    return Err(error);
                }
                logs::warn(
                    "vpn",
                    &format!("Download of {} will retry: {}", asset.name, error.detail),
                );
                progress(app, "retrying", asset, 0);
                tokio::time::sleep(Duration::from_secs(1)).await;
                download(app, asset, &archive, true).await?;
            }
        }
        progress(app, "extracting", asset, asset.size);
        unpack(asset, &archive, staging, &prepared)?;
        fs::remove_file(&archive).map_err(InstallError::disk)?;
    }
    for asset in ASSETS.iter() {
        if asset
            .files
            .iter()
            .filter(|file| file.as_str() != "LICENSE")
            .any(|file| !prepared.join(file).is_file())
        {
            return Err(InstallError::new(
                "vpn_install_disk",
                format!("{} files disappeared during installation", asset.name),
            ));
        }
    }
    let target = paths::vpn_dir();
    replace(&prepared, &target, backup)?;
    let versions = ASSETS
        .iter()
        .map(|asset| format!("{} {}", asset.name, asset.version))
        .collect::<Vec<_>>()
        .join(" / ");
    logs::append("vpn", &format!("Installed verified {versions}"));
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_truncated_and_modified_archives() {
        let path = std::env::temp_dir().join(format!("easyzapret-test-{}", rand::random::<u64>()));
        let mut asset = Asset {
            name: "test".into(),
            version: "".into(),
            archive: "".into(),
            url: "".into(),
            size: 3,
            sha256: format!("{:x}", Sha256::digest(b"abc")),
            files: vec![],
        };
        let mut archive = VerifiedArchive::create(&asset, &path).unwrap();
        archive.write(b"ab").unwrap();
        assert_eq!(archive.finish().unwrap_err().code, "vpn_install_incomplete");
        let mut archive = VerifiedArchive::create(&asset, &path).unwrap();
        archive.write(b"bad").unwrap();
        assert_eq!(archive.finish().unwrap_err().code, "vpn_install_integrity");
        asset.size = 4;
        let mut archive = VerifiedArchive::create(&asset, &path).unwrap();
        assert_eq!(
            archive.write(b"oversized").unwrap_err().code,
            "vpn_install_integrity"
        );
        drop(archive);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn installs_verified_release_archives_when_available() {
        let Some(resources) = std::env::var_os("EASYZAPRET_TEST_VPN_ARCHIVES") else {
            return;
        };
        let resources = std::path::PathBuf::from(resources);
        let staging =
            std::env::temp_dir().join(format!("easyzapret-test-{}", rand::random::<u64>()));
        let prepared = staging.join("prepared");
        fs::create_dir_all(&prepared).unwrap();
        for asset in ASSETS.iter() {
            let copy = staging.join(&asset.archive);
            copy_bundled(asset, &resources.join(&asset.archive), &copy).unwrap();
            unpack(asset, &copy, &staging, &prepared).unwrap();
        }
        let target = staging.join("installed");
        replace(&prepared, &target, &staging.join("backup")).unwrap();
        assert!(target.join("sing-box.exe").is_file());
        assert!(target.join("xray.exe").is_file());
        if cfg!(windows) {
            for (exe, version) in [("sing-box.exe", "1.12.12"), ("xray.exe", "25.10.15")] {
                let output = std::process::Command::new(target.join(exe))
                    .arg("version")
                    .output()
                    .unwrap();
                assert!(output.status.success());
                assert!(String::from_utf8_lossy(&output.stdout).contains(version));
            }
        }
        fs::remove_dir_all(staging).unwrap();
    }
}
