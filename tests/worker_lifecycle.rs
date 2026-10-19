use anyhow::Result;
use chrono::{Duration, Utc};
use std::{path::PathBuf, time::Duration as StdDuration};
use straja::{
    model::*,
    protocol::{self, Request, Response},
    store::Store,
    worker,
};

async fn request(name: &str, request: Request) -> Result<Response> {
    let mut pipe = tokio::net::windows::named_pipe::ClientOptions::new().open(name)?;
    protocol::write(&mut pipe, &request).await?;
    protocol::read(&mut pipe).await
}

#[tokio::test]
#[ignore = "Starts an isolated Windows named-pipe worker with real Restic"]
async fn missed_schedule_runs_without_gui_and_survives_restart() -> Result<()> {
    scheduled_and_manual_roundtrip(BackupMode::Encrypted).await
}

#[tokio::test]
#[ignore = "Starts an isolated Windows worker; run scripts/test.ps1"]
async fn plain_schedule_restart_and_manual_backup_without_restic() -> Result<()> {
    scheduled_and_manual_roundtrip(BackupMode::Plain).await
}

#[tokio::test]
#[ignore = "Requires Windows named pipes; run scripts/test.ps1"]
async fn archive_modes_scheduled_manual_restart_and_delete() -> Result<()> {
    for mode in [BackupMode::Zip, BackupMode::TarGz, BackupMode::SevenZip] {
        scheduled_and_manual_roundtrip(mode).await?;
    }
    Ok(())
}

async fn scheduled_and_manual_roundtrip(mode: BackupMode) -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().join("app");
    let src = tmp.path().join("source");
    let dst = tmp.path().join("repo");
    std::fs::create_dir(&src)?;
    std::fs::create_dir(&dst)?;
    std::fs::write(src.join("test.txt"), "scheduled data")?;
    let store = Store::open(&root)?;
    let mut plan = store.add(NewPlan {
        mode,
        name: "Scheduled test".into(),
        sources: vec![src],
        repository: dst,
        excluded_paths: vec![],
        excludes: vec![],
        keep_last: None,
        schedule: Schedule::Daily {
            hour: 23,
            minute: 59,
        },
        password: "test-only-secret".into(),
    })?;
    plan.next_due = Some(Utc::now() - Duration::hours(2));
    rusqlite::Connection::open(root.join("straja.sqlite"))?.execute(
        "UPDATE plans SET json=?1 WHERE id=?2",
        rusqlite::params![serde_json::to_string(&plan)?, plan.id],
    )?;
    let pipe = format!(r"\\.\pipe\StrajaTest-{}", uuid::Uuid::new_v4());
    let restic = if mode != BackupMode::Encrypted {
        tmp.path().join("restic-deliberately-absent.exe")
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".tools/restic.exe")
    };
    let (tx, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(worker::run_on_pipe(
        root.clone(),
        restic.clone(),
        false,
        rx,
        pipe.clone(),
    ));
    let state = tokio::time::timeout(StdDuration::from_secs(45), async {
        loop {
            if let Ok(Response::State(s)) = request(&pipe, Request::State).await
                && s.history.first().is_some_and(|r| r.finished.is_some())
            {
                return s;
            }
            tokio::time::sleep(StdDuration::from_millis(100)).await;
        }
    })
    .await?;
    assert_eq!(state.history.len(), 1);
    assert_eq!(
        state.history[0].outcome, "Reușit",
        "{}",
        state.history[0].detail
    );
    assert!(state.plans[0].last_success.is_some());
    assert!(state.plans[0].next_due.unwrap() > Utc::now());
    let (_other_tx, other_rx) = tokio::sync::watch::channel(false);
    assert!(
        worker::run_on_pipe(root.clone(), restic.clone(), false, other_rx, pipe.clone())
            .await
            .is_err(),
        "Second agent must not acquire the same pipe"
    );
    tx.send(true)?;
    task.await??;
    let (tx, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(worker::run_on_pipe(root, restic, false, rx, pipe.clone()));
    let state = tokio::time::timeout(StdDuration::from_secs(10), async {
        loop {
            if let Ok(Response::State(s)) = request(&pipe, Request::State).await {
                return s;
            }
            tokio::time::sleep(StdDuration::from_millis(100)).await;
        }
    })
    .await?;
    assert_eq!(
        state.history.len(),
        1,
        "Restart must not duplicate the scheduled backup"
    );
    assert!(state.active.is_none());
    assert!(matches!(
        request(&pipe, Request::Backup(plan.id.clone())).await?,
        Response::Started(_)
    ));
    let state = tokio::time::timeout(StdDuration::from_secs(45), async {
        loop {
            if let Ok(Response::State(s)) = request(&pipe, Request::State).await
                && s.active.is_none()
                && s.history.len() == 2
            {
                return s;
            }
            tokio::time::sleep(StdDuration::from_millis(100)).await;
        }
    })
    .await?;
    assert!(state.history.iter().all(|r| r.outcome == "Reușit"));
    assert!(
        matches!(request(&pipe, Request::Snapshots(plan.id.clone())).await?, Response::Snapshots(s) if s.len() == 2)
    );
    let source_before = std::fs::read(plan.sources[0].join("test.txt"))?;
    let repository_before = repository_contents(&plan.repository)?;
    assert!(matches!(
        request(&pipe, Request::Delete(plan.id.clone())).await?,
        Response::Ok(_)
    ));
    let Response::State(deleted) = request(&pipe, Request::State).await? else {
        panic!("Missing state")
    };
    assert!(deleted.plans.is_empty());
    assert_eq!(deleted.history.len(), 2);
    assert_eq!(
        std::fs::read(plan.sources[0].join("test.txt"))?,
        source_before
    );
    assert_eq!(repository_contents(&plan.repository)?, repository_before);
    assert!(matches!(
        request(&pipe, Request::Backup(plan.id.clone())).await?,
        Response::Error(_)
    ));
    assert!(matches!(
        request(&pipe, Request::Delete(plan.id.clone())).await?,
        Response::Error(_)
    ));
    assert!(matches!(
        request(&pipe, Request::Shutdown).await?,
        Response::Ok(_)
    ));
    tokio::time::timeout(StdDuration::from_secs(10), task).await???;
    drop(tx);
    assert!(Store::open(&store.root)?.plans()?.is_empty());
    Ok(())
}

fn repository_contents(
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>> {
    let mut contents = std::collections::BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                contents.insert(path.strip_prefix(root)?.to_path_buf(), std::fs::read(path)?);
            }
        }
    }
    Ok(contents)
}

#[tokio::test]
#[ignore = "Starts an isolated worker and real Restic for manual backup/edit regression"]
async fn manual_failure_is_reported_and_plan_can_be_fixed_without_losing_password() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().join("app");
    let source = tmp.path().join("source");
    let repo = tmp.path().join("repo");
    let corrected_repo = tmp.path().join("corrected");
    for path in [&source, &repo, &corrected_repo] {
        std::fs::create_dir(path)?;
    }
    std::fs::write(source.join("document.txt"), "test data")?;
    let store = Store::open(&root)?;
    let plan = store.add(NewPlan {
        mode: BackupMode::Encrypted,
        name: "Manual".into(),
        sources: vec![source.clone()],
        repository: repo.clone(),
        excluded_paths: vec![],
        excludes: vec![],
        keep_last: None,
        schedule: Schedule::Manual,
        password: "manual-test-password".into(),
    })?;
    // Reproduce a legacy plan whose destination was accepted despite being nonempty.
    std::fs::write(repo.join("existing.txt"), "do not overwrite")?;
    let pipe = format!(r"\\.\pipe\StrajaTest-{}", uuid::Uuid::new_v4());
    let (tx, rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(worker::run_on_pipe(
        root.clone(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".tools/restic.exe"),
        false,
        rx,
        pipe.clone(),
    ));
    tokio::time::timeout(StdDuration::from_secs(10), async {
        loop {
            if request(&pipe, Request::State).await.is_ok() {
                break;
            }
            tokio::time::sleep(StdDuration::from_millis(50)).await;
        }
    })
    .await?;
    async fn finish(pipe: &str, id: &str) -> Result<Run> {
        Ok(tokio::time::timeout(StdDuration::from_secs(60), async {
            loop {
                if let Response::State(s) = request(pipe, Request::State).await.unwrap()
                    && let Some(run) = s
                        .history
                        .into_iter()
                        .find(|r| r.id == id && r.finished.is_some())
                {
                    return run;
                }
                tokio::time::sleep(StdDuration::from_millis(50)).await;
            }
        })
        .await?)
    }
    let Response::Started(started) = request(&pipe, Request::Backup(plan.id.clone())).await? else {
        panic!("No operation ID returned")
    };
    let failed = finish(&pipe, &started.id).await?;
    assert_eq!(failed.outcome, "Eroare");
    assert!(failed.detail.contains("nu este gol"));
    assert_eq!(
        std::fs::read_to_string(repo.join("existing.txt"))?,
        "do not overwrite"
    );
    let changes = PlanChanges {
        name: "Corrected".into(),
        sources: vec![source],
        repository: corrected_repo,
        excluded_paths: vec![],
        excludes: vec!["*.tmp".into()],
        keep_last: None,
        schedule: Schedule::Manual,
    };
    assert!(matches!(
        request(
            &pipe,
            Request::Update {
                id: plan.id.clone(),
                changes
            }
        )
        .await?,
        Response::Ok(_)
    ));
    let Response::Started(started) = request(&pipe, Request::Backup(plan.id.clone())).await? else {
        panic!("No operation ID returned")
    };
    let success = finish(&pipe, &started.id).await?;
    assert_eq!(success.outcome, "Reușit", "{}", success.detail);
    assert_eq!(success.plan_id, failed.plan_id);
    assert_eq!(store.history()?.len(), 2);
    assert!(store.plan(&plan.id)?.last_success.is_some());
    assert_eq!(&*store.secret(&plan.id)?, b"manual-test-password");
    assert!(
        matches!(request(&pipe, Request::Snapshots(plan.id)).await?, Response::Snapshots(s) if s.len()==1)
    );
    tx.send(true)?;
    task.await??;
    Ok(())
}
