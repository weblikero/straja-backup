use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let result = run();
    if std::env::args().any(|a| a == "--vss-start")
        && let Err(error) = &result
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW};
        let message: Vec<u16> = format!("{error:#}").encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "Straja Backup — VSS"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                MB_ICONERROR,
            );
        }
    }
    result
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if straja::updater::running() && !args.iter().any(|a| a == "--service" || a == "--status") {
        return Ok(());
    }
    if args.iter().any(|a| a == "--tray") {
        return straja::tray::run();
    }
    if args.iter().any(|a| a == "--service") {
        return straja::service::dispatch();
    }
    let value = |key| {
        args.windows(2)
            .find(|a| a[0] == key)
            .map(|a| PathBuf::from(&a[1]))
    };
    let mut root = value("--data-dir")
        .map(Ok)
        .unwrap_or_else(straja::security::data_dir)?;
    let restic = value("--restic")
        .map(Ok)
        .unwrap_or_else(straja::engine::Engine::bundled)?;
    if !args.iter().any(|a| a == "--status")
        && let Err(e) = straja::resources::background_process()
    {
        eprintln!("Background resource priority: {e}");
    }
    let runtime = straja::resources::runtime()?;
    runtime.block_on(async {
        if args.iter().any(|a| a == "--vss-start") {
            straja::vss::require_privileges()?;
            let expected = args.windows(2).find(|a| a[0] == "--expected-user-sid").map(|a| a[1].as_str());
            anyhow::ensure!(expected == Some(straja::security::user_sid()?.as_str()),
                "Use the same Windows account for VSS. Switching accounts would make your plans and saved passwords inaccessible.");
            use straja::protocol::{call, Request, Response};
            // Reserve the idle agent first: scheduled jobs cannot start during handover.
            if let Ok(response) = call(Request::State).await {
                let Response::State(state) = response else { anyhow::bail!("Cannot inspect the current agent."); };
                anyhow::ensure!(!state.service_mode, "Configure VSS privileges for the Windows service account, then restart that service.");
                anyhow::ensure!(state.active.is_none(), "Wait for the current operation to finish before enabling VSS.");
                let token = uuid::Uuid::new_v4().to_string();
                match call(Request::PrepareUpdate(token.clone())).await? {
                    Response::UpdateReady { agent_executable, data_dir, service_mode: false } => {
                        if std::fs::canonicalize(agent_executable)? != std::fs::canonicalize(std::env::current_exe()?)? {
                            let _ = call(Request::CancelUpdate(token)).await;
                            anyhow::bail!("The agent runs from a different installation. Close it safely before enabling VSS here.");
                        }
                        root = data_dir;
                        match call(Request::ShutdownForUpdate(token.clone())).await {
                            Ok(Response::Ok(_)) => {},
                            other => { let _ = call(Request::CancelUpdate(token)).await; anyhow::bail!("Cannot stop the current agent: {other:?}"); }
                        }
                        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
                        while call(Request::State).await.is_ok() {
                            anyhow::ensure!(std::time::Instant::now() < deadline, "The current agent has not stopped; retry later.");
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                    }
                    response => anyhow::bail!("Cannot reserve the current agent: {response:?}"),
                }
            }
        }
        if args.iter().any(|a| a == "--status") {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &straja::protocol::call(straja::protocol::Request::State).await?
                )?
            );
            return Ok(());
        }
        let (tx, rx) = tokio::sync::watch::channel(false);
        // Interactive session only; Windows services never display UI in session 0.
        if !args.iter().any(|a| a == "--vss-start") && let Err(e) = straja::tray::launch() {
            eprintln!("Pictograma Straja: {e:#}");
        }
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(true);
        });
        straja::worker::run(root, restic, false, rx).await
    })
}
