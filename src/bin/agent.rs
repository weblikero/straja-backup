use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
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
    let root = value("--data-dir")
        .map(Ok)
        .unwrap_or_else(straja::security::data_dir)?;
    let restic = value("--restic")
        .map(Ok)
        .unwrap_or_else(straja::engine::Engine::bundled)?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
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
        if let Err(e) = straja::tray::launch() {
            eprintln!("Pictograma Straja: {e:#}");
        }
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(true);
        });
        straja::worker::run(root, restic, false, rx).await
    })
}
