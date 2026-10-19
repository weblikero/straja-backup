//! Update handoff runs from a copy outside the installation directory.
//! No taskkill, forced service stop, shell scripts, or replacement of backup data.
use crate::{
    protocol::{self, Request, Response},
    updates::{self, PreparedUpdate},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::windows::{fs::OpenOptionsExt, process::CommandExt};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::Threading::*,
    UI::{Shell::*, WindowsAndMessaging::*},
};

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}
fn ws(s: &str) -> Vec<u16> {
    wide(s.as_ref())
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

fn mutex_name() -> Result<Vec<u16>> {
    Ok(ws(&format!(
        "Local\\StrajaUpdate-{}",
        crate::security::user_sid()?
    )))
}
pub fn running() -> bool {
    let Ok(name) = mutex_name() else {
        return false;
    };
    let handle = unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
    !Handle(handle).0.is_null()
}
fn lock() -> Result<Handle> {
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, mutex_name()?.as_ptr()) };
    ensure!(!handle.is_null(), "Cannot create update lock");
    let guard = Handle(handle);
    ensure!(
        unsafe { GetLastError() } != ERROR_ALREADY_EXISTS,
        "Another update is running"
    );
    Ok(guard)
}
pub fn notify(message: &str) {
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            ws(message).as_ptr(),
            ws("Straja Backup").as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Agent {
    token: String,
    data_dir: PathBuf,
    service_mode: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Job {
    handoff: String,
    prepared: PreparedUpdate,
    target: PathBuf,
    parent_pid: u32,
    current_hash: String,
    agent: Option<Agent>,
    all_users: bool,
    language: String,
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
        _ => false,
    }
}
fn hash_file(file: &mut File) -> Result<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn verified_installer(prepared: &PreparedUpdate) -> Result<File> {
    let asset = prepared
        .update
        .installer
        .as_ref()
        .context("Missing installer metadata")?;
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x00200000)
        .open(&prepared.path)?;
    let meta = file.metadata()?;
    crate::plain::not_link(&meta, &prepared.path)?;
    ensure!(
        meta.is_file() && meta.len() == asset.size && meta.len() <= updates::MAX_INSTALLER,
        "Invalid installer size"
    );
    ensure!(
        hash_file(&mut file)? == asset.sha256,
        "Installer SHA-256 verification failed"
    );
    Ok(file) // Hold a read-only sharing lock through process launch and installer exit.
}

fn all_users(target: &Path) -> bool {
    use windows_sys::Win32::System::Registry::*;
    let key = ws(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{D76D401F-7D3B-4B8A-9BF6-3491DE7C802D}_is1",
    );
    let mut buffer = [0u16; 32768];
    let mut bytes = (buffer.len() * 2) as u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            ws("InstallLocation").as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    ok == 0
        && same_path(
            target,
            Path::new(&String::from_utf16_lossy(
                &buffer[..(bytes as usize / 2).saturating_sub(1)],
            )),
        )
}
fn language_name() -> &'static str {
    use crate::i18n::Language::*;
    match crate::i18n::current() {
        English => "english",
        German => "german",
        Spanish => "spanish",
        Portuguese => "portuguese",
        French => "french",
        Italian => "italian",
        Romanian => "romanian",
    }
}

pub async fn launch(prepared: PreparedUpdate) -> Result<()> {
    ensure!(!running(), "Another update is running");
    let exe = std::env::current_exe()?;
    let target = exe
        .parent()
        .context("Installation folder missing")?
        .to_path_buf();
    let folder = prepared
        .path
        .parent()
        .context("Download folder missing")?
        .to_owned();
    ensure!(
        same_path(
            folder.parent().context("Invalid download folder")?,
            &updates::update_directory()?
        ),
        "Invalid download location"
    );
    let verify = prepared.clone();
    tokio::task::spawn_blocking(move || verified_installer(&verify).map(drop)).await??;
    let current_hash = hash_file(&mut File::open(&exe)?)?;
    let helper = folder.join("straja-updater.exe");
    tokio::fs::copy(&exe, &helper).await?;
    let token = uuid::Uuid::new_v4().to_string();
    let agent = match protocol::call(Request::PrepareUpdate(token.clone())).await {
        Ok(Response::UpdateReady {
            agent_executable,
            data_dir,
            service_mode,
        }) => {
            if !same_path(&agent_executable, &target.join("straja-agent.exe")) {
                let _ = protocol::call(Request::CancelUpdate(token)).await;
                bail!(
                    "Agent belongs to a different installation. Open that installation to update it."
                );
            }
            Some(Agent {
                token,
                data_dir,
                service_mode,
            })
        }
        Ok(Response::Error(error)) => bail!(error),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.raw_os_error() == Some(2)) =>
        {
            None
        }
        Err(error) => return Err(error),
        _ => bail!("Agent does not support safe in-app updates. Restart the matching agent first."),
    };
    let result = async {
        let job = Job {
            handoff: uuid::Uuid::new_v4().to_string(),
            prepared,
            all_users: all_users(&target),
            target,
            parent_pid: std::process::id(),
            current_hash,
            agent: agent.clone(),
            language: language_name().into(),
        };
        let path = folder.join("job.json");
        // A previous handoff may have been cancelled while the GUI stayed open.
        let _ = tokio::fs::remove_file(folder.join("cancel")).await;
        tokio::fs::write(&path, serde_json::to_vec(&job)?).await?;
        let mut child = Command::new(helper)
            .arg("--apply-update")
            .arg(path)
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let start = Instant::now();
        loop {
            if tokio::fs::read_to_string(folder.join("ready"))
                .await
                .ok()
                .as_deref()
                == Some(&job.handoff)
            {
                break;
            }
            if child.try_wait()?.is_some() || start.elapsed() > Duration::from_secs(15) {
                let _ = tokio::fs::write(folder.join("cancel"), b"cancel").await;
                bail!("Update helper did not become ready. The application remains open.");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok::<_, anyhow::Error>(())
    }
    .await;
    if result.is_err()
        && let Some(agent) = agent
    {
        let _ = protocol::call(Request::CancelUpdate(agent.token)).await;
    }
    result
}

fn image_path(process: HANDLE) -> Option<PathBuf> {
    let mut buffer = [0u16; 32768];
    let mut len = buffer.len() as u32;
    (unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut len)
    } != 0)
        .then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..len as usize])))
}
fn wait_for_parent(job: &Job) -> Result<()> {
    let raw = unsafe {
        OpenProcess(
            SYNCHRONIZATION_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            job.parent_pid,
        )
    };
    if raw.is_null() {
        ensure!(
            unsafe { GetLastError() } == ERROR_INVALID_PARAMETER,
            "Cannot verify running application"
        );
        return Ok(());
    }
    let process = Handle(raw);
    ensure!(
        image_path(process.0).is_some_and(|p| same_path(&p, &job.target.join("straja.exe"))),
        "Application process identity changed"
    );
    ensure!(
        unsafe { WaitForSingleObject(process.0, 45000) } == WAIT_OBJECT_0,
        "Application did not close; update cancelled"
    );
    Ok(())
}
fn close_tray(target: &Path) {
    unsafe extern "system" fn visit(hwnd: HWND, value: LPARAM) -> windows_sys::core::BOOL {
        unsafe {
            let expected = &*(value as *const PathBuf);
            let mut class = [0u16; 128];
            let length = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
            if length <= 0
                || String::from_utf16_lossy(&class[..length as usize]) != "StrajaBackupTrayWindow"
            {
                return 1;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let process = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
            if !process.0.is_null()
                && image_path(process.0).is_some_and(|p| same_path(&p, expected))
            {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            1
        }
    }
    let expected = target.join("straja-agent.exe");
    unsafe {
        EnumWindows(Some(visit), (&expected as *const PathBuf) as LPARAM);
    }
}
fn unlocked(target: &Path, all_users: bool) -> bool {
    ["straja.exe", "straja-agent.exe", "restic.exe"]
        .iter()
        .all(|name| {
            let file = target.join(name);
            !file.exists()
                || match OpenOptions::new().write(true).share_mode(7).open(file) {
                    Ok(_) => true,
                    // An unelevated helper cannot write to Program Files. The elevated
                    // installer performs the authoritative file-in-use check before copying.
                    Err(error) => all_users && error.raw_os_error() == Some(5),
                }
        })
}
fn installer_args(job: &Job) -> Result<Vec<String>> {
    let target = job
        .target
        .to_str()
        .context("Installation path is not Unicode")?;
    ensure!(
        !target.contains(['"', '\r', '\n']),
        "Invalid installation path"
    );
    ensure!(
        [
            "english",
            "german",
            "spanish",
            "portuguese",
            "french",
            "italian",
            "romanian"
        ]
        .contains(&job.language.as_str()),
        "Invalid installer language"
    );
    Ok(vec![
        "/SP-".into(),
        "/SILENT".into(),
        "/SUPPRESSMSGBOXES".into(),
        "/NORESTART".into(),
        "/NOCLOSEAPPLICATIONS".into(),
        "/NORESTARTAPPLICATIONS".into(),
        "/RESTARTEXITCODE=3010".into(),
        "/LOG".into(),
        if job.all_users {
            "/ALLUSERS".into()
        } else {
            "/CURRENTUSER".into()
        },
        format!("/DIR={target}"),
        format!("/LANG={}", job.language),
    ])
}

fn owns_service(target: &Path) -> Result<()> {
    use windows_service::{
        service::ServiceAccess,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(crate::service::NAME, ServiceAccess::QUERY_CONFIG)?;
    let command = service
        .query_config()?
        .executable_path
        .to_string_lossy()
        .into_owned();
    let exe = if let Some(rest) = command.strip_prefix('"') {
        rest.split('"').next().unwrap_or("")
    } else {
        command.split_whitespace().next().unwrap_or("")
    };
    ensure!(
        same_path(Path::new(exe), &target.join("straja-agent.exe")),
        "Service belongs to another installation"
    );
    Ok(())
}
fn restart_service(target: &Path) -> Result<()> {
    use windows_service::{
        service::ServiceAccess,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    owns_service(target)?;
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    if let Ok(service) = manager.open_service(crate::service::NAME, ServiceAccess::QUERY_STATUS)
        && service.query_status()?.current_state == windows_service::service::ServiceState::Running
    {
        return Ok(());
    }
    if let Ok(service) = manager.open_service(crate::service::NAME, ServiceAccess::START)
        && service.start::<&str>(&[]).is_ok()
    {
        return Ok(());
    }
    // A normal user's helper stays unelevated. Only the fixed service-start command asks for UAC.
    let mut system = [0u16; 32768];
    let length = unsafe {
        windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
            system.as_mut_ptr(),
            system.len() as u32,
        )
    } as usize;
    ensure!(
        length > 0 && length < system.len(),
        "Cannot locate Windows system directory"
    );
    let exe = PathBuf::from(String::from_utf16_lossy(&system[..length])).join("sc.exe");
    let file = wide(exe.as_os_str());
    let verb = ws("runas");
    let parameters = ws("start StrajaBackup");
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.nShow = SW_HIDE;
    ensure!(
        unsafe { ShellExecuteExW(&mut info) } != 0,
        "Windows did not authorize restarting the service"
    );
    let process = Handle(info.hProcess);
    ensure!(
        !process.0.is_null() && unsafe { WaitForSingleObject(process.0, 30000) } == WAIT_OBJECT_0,
        "Service restart did not finish"
    );
    let mut code = 1;
    unsafe {
        GetExitCodeProcess(process.0, &mut code);
    }
    ensure!(code == 0 || code == 1056, "Service restart failed ({code})");
    Ok(())
}
async fn restart(job: &Job) -> Result<()> {
    if matches!(
        tokio::time::timeout(Duration::from_secs(3), protocol::call(Request::State)).await,
        Ok(Ok(Response::State(_)))
    ) {
        return Ok(());
    }
    if let Some(agent) = &job.agent {
        if agent.service_mode {
            restart_service(&job.target)?;
        } else {
            Command::new(job.target.join("straja-agent.exe"))
                .arg("--data-dir")
                .arg(&agent.data_dir)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?;
        }
    }
    Ok(())
}

async fn install(job: &Job) -> Result<u32> {
    let fresh = updates::check(true)
        .await
        .map_err(|e| anyhow::anyhow!(e.message()))?
        .context("Release no longer available")?;
    ensure!(
        fresh.version == job.prepared.update.version
            && fresh.installer == job.prepared.update.installer,
        "Published installer changed; update cancelled"
    );
    let locked = verified_installer(&job.prepared)?;
    wait_for_parent(job)?;
    ensure!(
        !job.prepared.path.with_file_name("cancel").exists(),
        "Update handoff cancelled"
    );
    if let Some(agent) = &job.agent {
        if agent.service_mode {
            owns_service(&job.target)?;
        }
        match protocol::call(Request::ShutdownForUpdate(agent.token.clone())).await? {
            Response::Ok(_) => {}
            Response::Error(e) => bail!(e),
            _ => bail!("Agent did not authorize update"),
        }
    }
    close_tray(&job.target);
    let start = Instant::now();
    while !unlocked(&job.target, job.all_users) {
        ensure!(
            start.elapsed() < Duration::from_secs(45),
            "Straja is still running. Update cancelled without forcing it to stop."
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let args = installer_args(job)?;
    let status = tokio::process::Command::new(&job.prepared.path)
        .args(args)
        .status()
        .await?;
    drop(locked);
    let code = status
        .code()
        .context("Installer did not return an exit code")? as u32;
    ensure!(
        code == 0 || code == 3010,
        "Installer was cancelled or failed (code {code})."
    );
    Ok(code)
}

pub fn apply(path: &Path) -> Result<()> {
    crate::i18n::reload();
    let folder = path.parent().context("Missing update folder")?;
    ensure!(
        path.file_name().is_some_and(|n| n == "job.json")
            && same_path(
                folder.parent().context("Invalid job path")?,
                &updates::update_directory()?
            ),
        "Invalid update job location"
    );
    ensure!(
        std::fs::metadata(path)?.len() <= 32768,
        "Update job too large"
    );
    let job: Job = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(
        same_path(
            folder,
            job.prepared
                .path
                .parent()
                .context("Invalid installer path")?
        ),
        "Installer outside update folder"
    );
    ensure!(
        job.prepared
            .path
            .file_name()
            .is_some_and(|n| n == "installer.exe"),
        "Invalid installer filename"
    );
    ensure!(
        hash_file(&mut File::open(std::env::current_exe()?)?)? == job.current_hash
            && hash_file(&mut File::open(job.target.join("straja.exe"))?)? == job.current_hash,
        "Installation identity changed"
    );
    let guard = lock()?;
    let runtime = crate::resources::monitor_runtime()?;
    std::fs::write(folder.join("ready"), &job.handoff)?;
    let result = runtime.block_on(install(&job));
    if let Some(agent) = &job.agent {
        let _ = runtime.block_on(protocol::call(Request::CancelUpdate(agent.token.clone())));
    }
    // Release the update mutex before starting the new binaries.
    drop(guard);
    let resume = if matches!(result, Ok(3010)) {
        Ok(())
    } else {
        runtime.block_on(restart(&job))
    };
    let message = match (&result, &resume) {
        (Ok(0), Ok(())) => None,
        (Ok(3010), _) => Some(crate::i18n::tr(
            "Actualizarea este instalată. Repornește Windows pentru a o finaliza.",
        )),
        (Err(error), _) => Some(format!(
            "{}\n\n{error:#}",
            crate::i18n::tr("Actualizarea nu s-a încheiat. Datele de backup nu au fost șterse.")
        )),
        (_, Err(error)) => Some(format!(
            "{}\n\n{error:#}",
            crate::i18n::tr("Actualizarea este instalată, dar agentul nu a putut fi repornit.")
        )),
        _ => None,
    };
    if let Some(message) = message {
        notify(&message);
    }
    if !matches!(result, Ok(3010)) {
        Command::new(job.target.join("straja.exe"))
            .spawn()
            .context("Cannot reopen Straja")?;
    }
    // Retain failed downloads for retry/diagnostics; remove the large installer after success.
    if result.is_ok() {
        let _ = std::fs::remove_file(&job.prepared.path);
    }
    Ok(()) // Result has already been shown; early failures are reported by main.
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepared(path: PathBuf, bytes: &[u8]) -> PreparedUpdate {
        PreparedUpdate {
            path,
            update: updates::Update {
                version: "1.0.0".into(),
                notes: String::new(),
                page_url: String::new(),
                installer: Some(updates::Installer {
                    url: String::new(),
                    size: bytes.len() as u64,
                    sha256: format!("{:x}", Sha256::digest(bytes)),
                }),
            },
        }
    }
    #[test]
    fn installer_is_reverified_and_locked_against_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("installer.exe");
        std::fs::write(&path, b"original").unwrap();
        let p = prepared(path.clone(), b"original");
        let guard = verified_installer(&p).unwrap();
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(guard);
        std::fs::write(&path, b"modified").unwrap();
        assert!(verified_installer(&p).is_err());
        std::fs::write(&path, b"short").unwrap();
        assert!(verified_installer(&p).is_err());
    }
    #[test]
    fn installer_preserves_scope_path_language_and_never_forces_close_or_reboot() {
        let mut job = Job {
            handoff: "test".into(),
            prepared: prepared(PathBuf::new(), b"test"),
            target: PathBuf::from(r"C:\Program Files\Straja Backup"),
            parent_pid: 0,
            current_hash: String::new(),
            agent: None,
            all_users: true,
            language: "german".into(),
        };
        let args = installer_args(&job).unwrap();
        for required in [
            "/ALLUSERS",
            "/LANG=german",
            r"/DIR=C:\Program Files\Straja Backup",
            "/NORESTART",
            "/NOCLOSEAPPLICATIONS",
            "/NORESTARTAPPLICATIONS",
            "/RESTARTEXITCODE=3010",
        ] {
            assert!(args.iter().any(|a| a == required));
        }
        assert!(
            !args
                .iter()
                .any(|a| a.starts_with("/TASKS") || a.contains("FORCE"))
        );
        job.all_users = false;
        assert!(
            installer_args(&job)
                .unwrap()
                .contains(&"/CURRENTUSER".into())
        );
        job.language = "english /FORCECLOSEAPPLICATIONS".into();
        assert!(installer_args(&job).is_err());
        job.language = "english".into();
        job.target = PathBuf::from("C:\\bad\"path");
        assert!(installer_args(&job).is_err());
    }
    #[test]
    fn app_files_must_be_released_before_replacement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("straja.exe");
        std::fs::write(&path, b"app").unwrap();
        let guard = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        assert!(!unlocked(root.path(), false));
        assert!(
            !unlocked(root.path(), true),
            "sharing violations are never bypassed"
        );
        drop(guard);
        assert!(unlocked(root.path(), false));
        assert_eq!(std::fs::read(path).unwrap(), b"app");
    }
}
