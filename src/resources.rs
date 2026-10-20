//! Resource budgets apply only to Straja and its own child processes.
use std::time::Duration;

pub const ACTIVE_POLL: Duration = Duration::from_secs(2);
pub const IDLE_UI_POLL: Duration = Duration::from_secs(10);
pub const IDLE_TRAY_POLL: Duration = Duration::from_secs(15);

pub fn runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .thread_name("straja-worker")
        .enable_all()
        .build()
}

pub fn monitor_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(2)
        .enable_all()
        .build()
}

/// Call only from the dedicated agent/service process, never the interactive GUI.
#[cfg(windows)]
pub fn background_process() -> std::io::Result<()> {
    use windows_sys::Win32::System::Threading::{
        BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, PROCESS_MODE_BACKGROUND_BEGIN,
        SetPriorityClass,
    };
    unsafe {
        if SetPriorityClass(GetCurrentProcess(), PROCESS_MODE_BACKGROUND_BEGIN) != 0
            || SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) != 0
        {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

pub fn restic_budget(command: &mut tokio::process::Command) {
    // These override only the child's environment; never change machine/user settings.
    // Go's memory target is soft, not a hard cap for very large repositories.
    command
        .env("GOMAXPROCS", "2")
        .env("GOMEMLIMIT", "512MiB")
        .env("RESTIC_READ_CONCURRENCY", "1");
    #[cfg(windows)]
    command.creation_flags(
        windows_sys::Win32::System::Threading::CREATE_NO_WINDOW
            | windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_is_bounded_and_restic_budget_is_child_local() {
        let runtime = runtime().unwrap();
        assert_eq!(runtime.metrics().num_workers(), 2);
        let previous = std::env::var_os("GOMAXPROCS");
        let mut command = tokio::process::Command::new("unused.exe");
        restic_budget(&mut command);
        let vars: std::collections::HashMap<_, _> = command
            .as_std()
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.unwrap().to_string_lossy().into_owned(),
                )
            })
            .collect();
        assert_eq!(vars["GOMAXPROCS"], "2");
        assert_eq!(vars["GOMEMLIMIT"], "512MiB");
        assert_eq!(vars["RESTIC_READ_CONCURRENCY"], "1");
        assert_eq!(std::env::var_os("GOMAXPROCS"), previous);
    }
}

#[cfg(unix)]
pub fn background_process() -> std::io::Result<()> {
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 10) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}
