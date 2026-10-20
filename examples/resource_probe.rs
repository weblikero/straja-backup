//! Isolated idle-agent probe. No real plans, tray, service or user database.
use anyhow::Result;
use std::time::{Duration, Instant};
use straja::{
    model::Run,
    protocol::{self, Request, Response},
    store::Store,
};

fn main() -> Result<()> {
    let root = tempfile::tempdir()?;
    let store = Store::open(root.path())?;
    for i in 0..100 {
        store.put_run(&Run {
            id: i.to_string(),
            plan_id: "synthetic".into(),
            plan_name: "Synthetic plan".into(),
            operation: "Backup".into(),
            started: chrono::Utc::now(),
            finished: Some(chrono::Utc::now()),
            outcome: "Reușit".into(),
            detail: "Synthetic diagnostic. ".repeat(200),
        })?;
    }
    straja::resources::background_process()?;
    let runtime = straja::resources::runtime()?;
    runtime.block_on(async {
        #[cfg(windows)]
        let pipe_name = format!(r"\\.\pipe\StrajaProbe-{}", uuid::Uuid::new_v4());
        #[cfg(unix)]
        let pipe_name = root
            .path()
            .join("probe.sock")
            .to_string_lossy()
            .into_owned();
        let (tx, rx) = tokio::sync::watch::channel(false);
        let worker = tokio::spawn(straja::worker::run_on_pipe(
            root.path().to_owned(),
            root.path().join("absent-restic.exe"),
            false,
            rx,
            pipe_name.clone(),
        ));
        tokio::time::sleep(Duration::from_millis(200)).await;
        let start = Instant::now();
        let mut requests = 0;
        let mut bytes = 0;
        let mut next_ui = start;
        let mut next_tray = start;
        while start.elapsed() < Duration::from_secs(20) {
            for (next, interval, request) in [
                (
                    &mut next_ui,
                    straja::resources::IDLE_UI_POLL,
                    Request::State,
                ),
                (
                    &mut next_tray,
                    straja::resources::IDLE_TRAY_POLL,
                    Request::Status,
                ),
            ] {
                if Instant::now() < *next {
                    continue;
                }
                #[cfg(windows)]
                let mut pipe =
                    tokio::net::windows::named_pipe::ClientOptions::new().open(&pipe_name)?;
                #[cfg(unix)]
                let mut pipe = tokio::net::UnixStream::connect(&pipe_name).await?;
                protocol::write(&mut pipe, &request).await?;
                let response: Response = protocol::read(&mut pipe).await?;
                bytes += serde_json::to_vec(&response)?.len();
                requests += 1;
                *next = Instant::now() + interval;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        tx.send(true)?;
        worker.await??;
        println!("requests={requests} response_bytes={bytes}");
        Ok::<_, anyhow::Error>(())
    })
}
