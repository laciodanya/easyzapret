mod compatibility;
mod config;
mod import;
mod storage;
#[cfg(test)]
mod tests;
mod types;

pub use compatibility::filter_argument;
pub use compatibility::required as filter_required;
pub use types::VpnStatus;

use serde_json::Value;
use std::net::{IpAddr, TcpListener};
use std::process::{Child, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use crate::{logs, paths, util, AppState};
use types::*;

pub(crate) static OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Core {
    child: Child,
    #[cfg(windows)]
    _job: std::os::windows::io::OwnedHandle,
}

impl Drop for Core {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct Runtime {
    cores: Vec<Core>,
    active: Option<Store>,
    mode: Option<Mode>,
    error: Option<String>,
    files: Vec<std::path::PathBuf>,
    interface_index: Option<u32>,
}

fn runtime() -> &'static Mutex<Runtime> {
    static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(Runtime::default()))
}

pub fn installed() -> bool {
    paths::vpn_dir().join("sing-box.exe").is_file() && paths::vpn_dir().join("xray.exe").is_file()
}

pub fn tun_running() -> bool {
    let runtime = util::lock_mutex(runtime());
    !runtime.cores.is_empty() && runtime.mode.as_ref() != Some(&Mode::Proxy)
}

pub fn quick_status() -> VpnStatus {
    let runtime = util::lock_mutex(runtime());
    VpnStatus {
        installed: installed(),
        running: !runtime.cores.is_empty(),
        mode: runtime.mode.clone().unwrap_or(Mode::OnlyApps),
        active_profile: runtime
            .active
            .as_ref()
            .and_then(|s| s.selected_profile.clone()),
        error: runtime.error.clone(),
    }
}

fn details(store: &Store) -> VpnDetails {
    VpnDetails {
        status: quick_status(),
        settings: store.settings.clone(),
        selected_profile: store.selected_profile.clone(),
        profiles: store.profiles.iter().map(Profile::info).collect(),
        subscriptions: store
            .subscriptions
            .iter()
            .map(|s| SubscriptionInfo {
                id: s.id.clone(),
                name: s.name.clone(),
                updated_at: s.updated_at.clone(),
                host: reqwest::Url::parse(&s.url)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_owned))
                    .unwrap_or_default(),
            })
            .collect(),
    }
}

fn selected(store: &Store) -> Result<&Profile, String> {
    store
        .profiles
        .iter()
        .find(|p| Some(&p.id) == store.selected_profile.as_ref())
        .ok_or("vpn_select_profile".into())
}

fn runtime_file(suffix: &str) -> std::path::PathBuf {
    paths::vpn_dir().join(format!("runtime-{}-{suffix}.json", std::process::id()))
}

fn write_config(path: &std::path::Path, config: &Value) -> Result<(), String> {
    util::atomic_write(
        path,
        &serde_json::to_vec(config).map_err(|_| "vpn_invalid_json")?,
    )
}

fn check_configuration(store: &Store) -> Result<Vec<std::path::PathBuf>, String> {
    if !installed() {
        return Err("vpn_not_installed".into());
    }
    let profile = selected(store)?;
    let sing = runtime_file("sing-box");
    let xray = runtime_file("xray");
    let mut files = vec![sing.clone()];
    let result = (|| {
        write_config(&sing, &config::build(&store.settings, profile)?)?;
        let executable = paths::vpn_dir().join("sing-box.exe");
        let output = util::hidden_command(&executable.to_string_lossy())
            .args(["check", "-c"])
            .arg(&sing)
            .output()
            .map_err(|_| "vpn_core_check_failed")?;
        if !output.status.success() {
            return Err("vpn_config_invalid".into());
        }
        if profile.engine == "xray" {
            files.push(xray.clone());
            write_config(&xray, &config::xray_config(&store.settings, profile))?;
            let output = util::hidden_command(&paths::vpn_dir().join("xray.exe").to_string_lossy())
                .args(["run", "-test", "-config"])
                .arg(&xray)
                .output()
                .map_err(|_| "vpn_core_check_failed")?;
            if !output.status.success() {
                return Err("vpn_config_invalid".into());
            }
        }
        Ok(files.clone())
    })();
    if result.is_err() {
        for file in files {
            let _ = std::fs::remove_file(file);
        }
    }
    result
}

fn spawn_core(name: &str, path: &std::path::Path) -> Result<Core, String> {
    let executable = paths::vpn_dir().join(name);
    let flag = if name == "xray.exe" { "-config" } else { "-c" };
    let mut child = util::hidden_command(&executable.to_string_lossy())
        .args(["run", flag])
        .arg(path)
        .current_dir(paths::vpn_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "vpn_start_failed")?;
    #[cfg(windows)]
    let job = match attach_job(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    #[cfg(not(windows))]
    let _ = &mut child;
    Ok(Core {
        child,
        #[cfg(windows)]
        _job: job,
    })
}

#[cfg(windows)]
fn attach_job(child: &Child) -> Result<std::os::windows::io::OwnedHandle, String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::JobObjects::*;
    let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw.is_null() {
        return Err("vpn_job_failed".into());
    }
    let job = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let configured = unsafe {
        SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    };
    if configured == 0 || unsafe { AssignProcessToJobObject(raw, child.as_raw_handle()) } == 0 {
        return Err("vpn_job_failed".into());
    }
    Ok(job)
}

async fn server_ips(profile: &Profile) -> Result<Vec<IpAddr>, String> {
    let addresses = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::net::lookup_host((profile.server(), profile.port())),
    )
    .await
    .map_err(|_| "vpn_server_dns_failed")?
    .map_err(|_| "vpn_server_dns_failed")?;
    let mut ips: Vec<_> = addresses.map(|a| a.ip()).collect();
    ips.sort();
    ips.dedup();
    if ips.is_empty() {
        return Err("vpn_server_dns_failed".into());
    }
    Ok(ips)
}

fn start(state: &AppState, store: &Store, ips: &[IpAddr]) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("vpn_windows_only".into());
    }
    let _transition = util::lock_mutex(&crate::NETWORK_TRANSITION);
    let _preparing = compatibility::Preparing::begin();
    if store.settings.mode != Mode::Proxy
        && (state.warp_active.load(std::sync::atomic::Ordering::SeqCst)
            || crate::warp::quick_status().connected)
    {
        return Err("vpn_warp_conflict".into());
    }
    let profile = selected(store)?;
    let files = check_configuration(store)?;
    let mut tunnel_spawned = false;
    let result = (|| {
        let ports = if profile.engine == "xray" {
            vec![store.settings.local_port, store.settings.local_port + 1]
        } else {
            vec![store.settings.local_port]
        };
        // Retain reservations until all ports have been checked.
        let reservations: Result<Vec<_>, _> = ports
            .iter()
            .map(|p| TcpListener::bind(("127.0.0.1", *p)))
            .collect();
        let reservations = reservations.map_err(|_| "vpn_port_in_use")?;
        drop(reservations);
        compatibility::prepare(state, ips, profile.port(), None)?;
        let mut cores = Vec::new();
        if profile.engine == "xray" {
            cores.push(spawn_core("xray.exe", &runtime_file("xray"))?);
        }
        cores.push(spawn_core("sing-box.exe", &runtime_file("sing-box"))?);
        tunnel_spawned = store.settings.mode != Mode::Proxy;
        // A listening proxy and live children confirm that startup completed.
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        loop {
            for core in &mut cores {
                if core
                    .child
                    .try_wait()
                    .map_err(|_| "vpn_start_failed")?
                    .is_some()
                {
                    return Err("vpn_core_exited".into());
                }
            }
            let ready = ports.iter().all(|p| {
                std::net::TcpStream::connect_timeout(
                    &std::net::SocketAddr::from(([127, 0, 0, 1], *p)),
                    Duration::from_millis(100),
                )
                .is_ok()
            });
            if ready {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err("vpn_start_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        let interface_index = if store.settings.mode == Mode::Proxy {
            None
        } else {
            let index = tunnel_interface()?.ok_or("vpn_tunnel_missing")?;
            // The interface exclusion also covers connections opened before TUN started.
            compatibility::prepare(state, ips, profile.port(), Some(index))?;
            Some(index)
        };
        for core in &mut cores {
            if core
                .child
                .try_wait()
                .map_err(|_| "vpn_start_failed")?
                .is_some()
            {
                return Err("vpn_core_exited".into());
            }
        }
        let mut runtime = util::lock_mutex(runtime());
        runtime.cores = cores;
        runtime.mode = Some(store.settings.mode.clone());
        runtime.active = Some(store.clone());
        runtime.files = files.clone();
        runtime.error = None;
        runtime.interface_index = interface_index;
        logs::append("vpn", "VPN started");
        Ok(())
    })();
    if result.is_err() {
        for file in files {
            let _ = std::fs::remove_file(file);
        }
        let _ = compatibility::reset(state, true);
        if tunnel_spawned {
            let _ = cleanup_tunnel_routes(None);
        }
    }
    result
}

pub fn shutdown(state: &AppState, restart_zapret: bool) -> Result<(), String> {
    let _transition = util::lock_mutex(&crate::NETWORK_TRANSITION);
    let (cores, files, interface_index) = {
        let mut runtime = util::lock_mutex(runtime());
        runtime.active = None;
        (
            std::mem::take(&mut runtime.cores),
            std::mem::take(&mut runtime.files),
            runtime.interface_index.take(),
        )
    };
    drop(cores);
    let routes = if interface_index.is_some() {
        cleanup_tunnel_routes(interface_index)
    } else {
        Ok(())
    };
    for file in files {
        let _ = std::fs::remove_file(file);
    }
    compatibility::reset(state, restart_zapret)?;
    routes?;
    logs::append("vpn", "VPN stopped");
    Ok(())
}

#[cfg(windows)]
fn tunnel_interface() -> Result<Option<u32>, String> {
    let script = format!("$ErrorActionPreference='Stop'; $a=Get-NetAdapter -Name '{}' -IncludeHidden -ErrorAction SilentlyContinue; if ($a) {{ [Console]::Write($a.ifIndex) }}",config::TUN_NAME);
    let (ok, out) = util::run_capture(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    );
    if !ok {
        return Err("vpn_tunnel_query_failed".into());
    }
    if out.trim().is_empty() {
        return Ok(None);
    }
    out.trim()
        .parse()
        .map(Some)
        .map_err(|_| "vpn_tunnel_query_failed".into())
}

#[cfg(not(windows))]
fn tunnel_interface() -> Result<Option<u32>, String> {
    Ok(None)
}

#[cfg(windows)]
fn cleanup_tunnel_routes(index: Option<u32>) -> Result<(), String> {
    let index_condition = index
        .map(|index| format!(" -and $_.InterfaceIndex -eq {index}"))
        .unwrap_or_default();
    let script = format!("$ErrorActionPreference='Stop'; Get-NetRoute -ErrorAction SilentlyContinue | Where-Object {{ $_.InterfaceAlias -eq '{}'{index_condition} }} | Remove-NetRoute -Confirm:$false -ErrorAction Stop",config::TUN_NAME);
    let (ok, _) = util::run_capture(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    );
    if ok {
        Ok(())
    } else {
        Err("vpn_cleanup_failed".into())
    }
}

#[cfg(not(windows))]
fn cleanup_tunnel_routes(_index: Option<u32>) -> Result<(), String> {
    Ok(())
}

async fn apply(state: &AppState, store: Store) -> Result<VpnDetails, String> {
    config::validate_settings(&store.settings)?;
    let previous = util::lock_mutex(runtime()).active.clone();
    if let Some(previous) = previous {
        let new_ips = if store.selected_profile.is_some() {
            config::build(&store.settings, selected(&store)?)?;
            server_ips(selected(&store)?).await?
        } else {
            Vec::new()
        };
        // Validate before stopping a working connection.
        if store.selected_profile.is_some() {
            check_configuration(&store)?;
        }
        shutdown(state, true)?;
        let result = (|| {
            if store.selected_profile.is_some() {
                start(state, &store, &new_ips)?;
            }
            storage::save(&store)
        })();
        if let Err(error) = result {
            let _ = shutdown(state, true);
            let restored = match server_ips(selected(&previous)?).await {
                Ok(ips) => start(state, &previous, &ips),
                Err(e) => Err(e),
            };
            if restored.is_err() {
                util::lock_mutex(runtime()).error = Some("vpn_restore_failed".into());
                return Err(format!("{error}; vpn_restore_failed"));
            }
            return Err(format!("{error}; vpn_previous_restored"));
        }
    } else {
        storage::save(&store)?;
    }
    util::lock_mutex(runtime()).mode = Some(store.settings.mode.clone());
    Ok(details(&store))
}

#[tauri::command]
pub async fn vpn_details() -> Result<VpnDetails, String> {
    let _operation = OPERATION.lock().await;
    let store = storage::load()?;
    util::lock_mutex(runtime()).mode = Some(store.settings.mode.clone());
    Ok(details(&store))
}

#[tauri::command]
pub async fn vpn_save_settings(
    state: State<'_, AppState>,
    settings: VpnSettings,
) -> Result<VpnDetails, String> {
    let _operation = OPERATION.lock().await;
    let mut store = storage::load()?;
    store.settings = settings;
    apply(&state, store).await
}

#[tauri::command]
pub async fn vpn_select_profile(
    state: State<'_, AppState>,
    id: String,
) -> Result<VpnDetails, String> {
    let _operation = OPERATION.lock().await;
    let mut store = storage::load()?;
    if !store.profiles.iter().any(|p| p.id == id) {
        return Err("vpn_select_profile".into());
    }
    store.selected_profile = Some(id);
    apply(&state, store).await
}

#[tauri::command]
pub async fn vpn_remove_profile(
    state: State<'_, AppState>,
    id: String,
) -> Result<VpnDetails, String> {
    let _operation = OPERATION.lock().await;
    let mut store = storage::load()?;
    store.profiles.retain(|p| p.id != id);
    if store.selected_profile.as_ref() == Some(&id) {
        store.selected_profile = store.profiles.first().map(|p| p.id.clone());
    }
    apply(&state, store).await
}

#[tauri::command]
pub async fn vpn_profile_json(id: String) -> Result<String, String> {
    let _operation = OPERATION.lock().await;
    let store = storage::load()?;
    let profile = store
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or("vpn_select_profile")?;
    serde_json::to_string_pretty(&profile.outbound).map_err(|_| "vpn_invalid_json".into())
}

#[tauri::command]
pub async fn vpn_update_profile(
    state: State<'_, AppState>,
    id: String,
    name: String,
    json: String,
) -> Result<VpnDetails, String> {
    let _operation = OPERATION.lock().await;
    let mut store = storage::load()?;
    let profile = store
        .profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or("vpn_select_profile")?;
    let mut parsed = import::parse(&json)?;
    if parsed.len() != 1 {
        return Err("vpn_single_profile_required".into());
    }
    let replacement = parsed.remove(0);
    profile.name = name;
    profile.outbound = replacement.outbound;
    profile.engine = replacement.engine;
    import::validate_profile(profile)?;
    apply(&state, store).await
}

async fn fetch_subscription(raw: &str) -> Result<(String, Vec<Profile>), String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "vpn_invalid_subscription")?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err("vpn_invalid_subscription".into());
    }
    let client = reqwest::Client::builder()
        .user_agent("EasyZapret VPN/1.0")
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|_| "vpn_subscription_failed")?;
    let mut response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|_| "vpn_subscription_failed")?;
    if !response.status().is_success() {
        return Err("vpn_subscription_failed".into());
    }
    if response.content_length().unwrap_or_default() > import::MAX_IMPORT_BYTES as u64 {
        return Err("vpn_import_too_large".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "vpn_subscription_failed")?
    {
        if bytes.len() + chunk.len() > import::MAX_IMPORT_BYTES {
            return Err("vpn_import_too_large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(bytes).map_err(|_| "vpn_invalid_subscription")?;
    let profiles = import::parse(&body)?;
    Ok((url.to_string(), profiles))
}

fn replace_subscription_profiles(store: &mut Store, id: &str, mut profiles: Vec<Profile>) {
    let old: Vec<_> = store
        .profiles
        .iter()
        .filter(|p| p.subscription_id.as_deref() == Some(id))
        .cloned()
        .collect();
    let selected = old
        .iter()
        .find(|p| Some(&p.id) == store.selected_profile.as_ref());
    let mut used = std::collections::HashSet::new();
    for profile in &mut profiles {
        profile.subscription_id = Some(id.into());
        if let Some(previous) = old.iter().find(|p| {
            !used.contains(&p.id) && p.name == profile.name && p.server() == profile.server()
        }) {
            profile.id = previous.id.clone();
            used.insert(previous.id.clone());
        }
    }
    if let Some(selected) = selected {
        store.selected_profile = profiles
            .iter()
            .find(|p| p.id == selected.id)
            .or_else(|| profiles.first())
            .map(|p| p.id.clone());
    }
    store
        .profiles
        .retain(|p| p.subscription_id.as_deref() != Some(id));
    store.profiles.extend(profiles);
}

#[tauri::command]
pub async fn vpn_import(state: State<'_, AppState>, input: String) -> Result<VpnDetails, String> {
    let _operation = OPERATION.lock().await;
    let mut store = storage::load()?;
    let input = input.trim();
    let input = if let Some(raw) = input.strip_prefix("happ://add/") {
        percent_encoding::percent_decode_str(raw)
            .decode_utf8_lossy()
            .into_owned()
    } else {
        input.to_owned()
    };
    if input.starts_with("happ://") {
        return Err("vpn_happ_encrypted_unsupported".into());
    }
    if (input.starts_with("https://") || input.starts_with("http://")) && !input.contains('\n') {
        let (url, profiles) = fetch_subscription(&input).await?;
        let existing = store
            .subscriptions
            .iter()
            .find(|s| s.url == url)
            .map(|s| s.id.clone());
        let id = existing.clone().unwrap_or_else(new_id);
        replace_subscription_profiles(&mut store, &id, profiles);
        if existing.is_none() {
            store.subscriptions.push(Subscription {
                id: id.clone(),
                name: reqwest::Url::parse(&url)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_owned))
                    .unwrap_or_else(|| "VPN".into()),
                url,
                updated_at: String::new(),
            });
        }
        if let Some(s) = store.subscriptions.iter_mut().find(|s| s.id == id) {
            s.updated_at = chrono::Utc::now().to_rfc3339();
        }
    } else {
        store.profiles.extend(import::parse(&input)?);
    }
    if store.profiles.len() > 512 {
        return Err("vpn_too_many_profiles".into());
    }
    if store.selected_profile.is_none() {
        store.selected_profile = store.profiles.first().map(|p| p.id.clone());
    }
    apply(&state, store).await
}

#[tauri::command]
pub async fn vpn_refresh_subscription(
    state: State<'_, AppState>,
    id: String,
) -> Result<VpnDetails, String> {
    let _operation = OPERATION.lock().await;
    let mut store = storage::load()?;
    let subscription = store
        .subscriptions
        .iter()
        .find(|s| s.id == id)
        .ok_or("vpn_invalid_subscription")?;
    let (_, profiles) = fetch_subscription(&subscription.url).await?;
    replace_subscription_profiles(&mut store, &id, profiles);
    store
        .subscriptions
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or("vpn_invalid_subscription")?
        .updated_at = chrono::Utc::now().to_rfc3339();
    if store.profiles.len() > 512 {
        return Err("vpn_too_many_profiles".into());
    }
    apply(&state, store).await
}

#[tauri::command]
pub async fn vpn_remove_subscription(
    state: State<'_, AppState>,
    id: String,
) -> Result<VpnDetails, String> {
    let _operation = OPERATION.lock().await;
    let mut store = storage::load()?;
    store.subscriptions.retain(|s| s.id != id);
    store
        .profiles
        .retain(|p| p.subscription_id.as_ref() != Some(&id));
    if !store
        .profiles
        .iter()
        .any(|p| Some(&p.id) == store.selected_profile.as_ref())
    {
        store.selected_profile = store.profiles.first().map(|p| p.id.clone());
    }
    apply(&state, store).await
}

#[tauri::command]
pub async fn vpn_connect(state: State<'_, AppState>) -> Result<(), String> {
    let _operation = OPERATION.lock().await;
    if !util::lock_mutex(runtime()).cores.is_empty() {
        return Ok(());
    }
    let store = storage::load()?;
    let ips = server_ips(selected(&store)?).await?;
    let result = start(&state, &store, &ips);
    if let Err(error) = &result {
        util::lock_mutex(runtime()).error = Some(error.clone());
    }
    result
}

#[tauri::command]
pub async fn vpn_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    let _operation = OPERATION.lock().await;
    shutdown(&state, true)?;
    util::lock_mutex(runtime()).error = None;
    Ok(())
}

#[tauri::command]
pub fn vpn_list_apps() -> Vec<String> {
    let Some(system) = util::process_snapshot() else {
        return Vec::new();
    };
    let mut apps: Vec<_> = system
        .processes()
        .values()
        .filter_map(|p| p.exe())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    apps.sort_by_key(|p| p.to_ascii_lowercase());
    apps.dedup();
    apps
}

#[tauri::command]
pub async fn vpn_test_connection() -> Result<u64, String> {
    let _operation = OPERATION.lock().await;
    let store = util::lock_mutex(runtime())
        .active
        .clone()
        .ok_or("vpn_not_running")?;
    let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{}", store.settings.local_port))
        .map_err(|_| "vpn_test_failed")?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "vpn_test_failed")?;
    let at = std::time::Instant::now();
    let response = client
        .get("https://cp.cloudflare.com/generate_204")
        .send()
        .await
        .map_err(|_| "vpn_test_failed")?;
    if response.status().as_u16() != 204 {
        return Err("vpn_test_failed".into());
    }
    Ok(at.elapsed().as_millis() as u64)
}

#[tauri::command]
pub async fn vpn_pick_app() -> Result<Option<String>, String> {
    #[cfg(not(windows))]
    {
        Err("vpn_windows_only".into())
    }
    #[cfg(windows)]
    {
        tokio::task::spawn_blocking(|| {
            use std::os::windows::ffi::OsStringExt;
            use windows_sys::Win32::UI::Controls::Dialogs::*;
            let mut buffer = vec![0u16; 32768];
            let filter: Vec<u16> = "Windows applications\0*.exe\0\0".encode_utf16().collect();
            let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
            dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
            dialog.lpstrFilter = filter.as_ptr();
            dialog.lpstrFile = buffer.as_mut_ptr();
            dialog.nMaxFile = buffer.len() as u32;
            dialog.Flags =
                OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_DONTADDTORECENT | OFN_NOCHANGEDIR;
            if unsafe { GetOpenFileNameW(&mut dialog) } == 0 {
                return if unsafe { CommDlgExtendedError() } == 0 {
                    Ok(None)
                } else {
                    Err("vpn_file_picker_failed".into())
                };
            }
            let length = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
            Ok(Some(
                std::ffi::OsString::from_wide(&buffer[..length])
                    .to_string_lossy()
                    .into_owned(),
            ))
        })
        .await
        .map_err(|_| "vpn_file_picker_failed")?
    }
}

pub fn start_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let Ok(_operation) = OPERATION.try_lock() else {
                continue;
            };
            let exited = {
                let mut runtime = util::lock_mutex(runtime());
                runtime
                    .cores
                    .iter_mut()
                    .any(|core| !matches!(core.child.try_wait(), Ok(None)))
            };
            if exited {
                let state = app.state::<AppState>();
                let cleanup = shutdown(&state, true);
                util::lock_mutex(runtime()).error = Some(
                    if cleanup.is_ok() {
                        "vpn_core_exited"
                    } else {
                        "vpn_cleanup_failed"
                    }
                    .into(),
                );
                logs::error("vpn", "VPN core exited, managed tunnel stopped");
            } else if tun_running() && crate::warp::quick_status().connected {
                let state = app.state::<AppState>();
                let _ = shutdown(&state, true);
                util::lock_mutex(runtime()).error = Some("vpn_warp_conflict".into());
            }
        }
    });
}

pub async fn auto_connect(app: &AppHandle) {
    let _operation = OPERATION.lock().await;
    let result = async {
        let store = storage::load()?;
        if !store.settings.auto_connect {
            return Ok(());
        }
        let ips = server_ips(selected(&store)?).await?;
        start(&app.state::<AppState>(), &store, &ips)
    }
    .await;
    if let Err(error) = result {
        util::lock_mutex(runtime()).error = Some(error);
        logs::warn("vpn", "VPN autoconnect failed, open VPN settings");
    }
}

pub async fn recover(app: &AppHandle) {
    let _operation = OPERATION.lock().await;
    let filter = std::fs::read_to_string(compatibility::filter_path()).unwrap_or_default();
    if !filter.contains("ifIdx") && !filter.contains("DstPort") {
        return;
    }
    if cleanup_tunnel_routes(None)
        .and_then(|_| compatibility::reset(&app.state::<AppState>(), true))
        .is_err()
    {
        util::lock_mutex(runtime()).error = Some("vpn_cleanup_failed".into());
        logs::warn("vpn", "Recovery of the previous VPN session failed");
    }
}
