use anyhow::Result;
use std::{ffi::OsString, time::Duration};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

pub const NAME: &str = "StrajaBackup";
define_windows_service!(ffi_main, service_main);
pub fn dispatch() -> Result<()> {
    Ok(service_dispatcher::start(NAME, ffi_main)?)
}
fn service_main(_args: Vec<OsString>) {
    if let Err(e) = run() {
        eprintln!("{e:#}");
    }
}
fn run() -> Result<()> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = service_control_handler::register(NAME, move |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            let _ = tx.send(true);
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;
    let status = |state, exit| ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: if state == ServiceState::Running {
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
        } else {
            ServiceControlAccept::empty()
        },
        exit_code: ServiceExitCode::Win32(exit),
        checkpoint: 0,
        wait_hint: Duration::from_secs(30),
        process_id: None,
    };
    handle.set_service_status(status(ServiceState::StartPending, 0))?;
    if crate::updater::running() {
        handle.set_service_status(status(ServiceState::Stopped, 0))?;
        return Ok(());
    }
    let args: Vec<String> = std::env::args().collect();
    let root = args
        .windows(2)
        .find(|a| a[0] == "--data-dir")
        .map(|a| std::path::PathBuf::from(&a[1]))
        .map(Ok)
        .unwrap_or_else(crate::security::data_dir)?;
    let restic = crate::engine::Engine::bundled()?;
    if let Err(e) = crate::resources::background_process() {
        eprintln!("Background resource priority: {e}");
    }
    let runtime = crate::resources::runtime()?;
    handle.set_service_status(status(ServiceState::Running, 0))?;
    let result = runtime.block_on(crate::worker::run(root, restic, true, rx));
    handle.set_service_status(status(
        ServiceState::Stopped,
        if result.is_ok() { 0 } else { 1 },
    ))?;
    result
}
