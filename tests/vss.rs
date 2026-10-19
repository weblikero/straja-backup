#![cfg(windows)]
use anyhow::{Result, ensure};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use straja::{engine::Engine, model::*, store::Store, vss::SnapshotSet};

fn fixture(mode: BackupMode) -> Result<(tempfile::TempDir, Engine, Plan)> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("Grüße și date");
    let repo = temp.path().join("backup");
    fs::create_dir_all(source.join("cache"))?;
    fs::create_dir(&repo)?;
    fs::write(source.join("locked.txt"), b"saved before snapshot")?;
    fs::write(source.join("cache/excluded.txt"), b"excluded path")?;
    fs::write(source.join("skip.tmp"), b"excluded pattern")?;
    let single = temp.path().join("single.txt");
    fs::write(&single, b"single file locked")?;
    let store = Store::open(&source.join("StrajaState"))?;
    let plan = store.add(NewPlan {
        mode,
        name: "VSS test".into(),
        sources: vec![source.clone(), single],
        repository: repo,
        excluded_paths: vec![source.join("cache")],
        excludes: vec!["*.tmp".into()],
        keep_last: Some(1),
        use_vss: true,
        schedule: Schedule::Manual,
        password: "isolated-vss-test-password".into(),
    })?;
    let engine = Engine {
        binary: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".tools/restic.exe"),
        store,
        live: Arc::new(Mutex::new(Some(Progress::default()))),
    };
    Ok((temp, engine, plan))
}

#[test]
fn vss_option_roundtrips_and_old_plans_default_to_disabled() -> Result<()> {
    let (_temp, engine, mut plan) = fixture(BackupMode::Plain)?;
    assert!(engine.store.plan(&plan.id)?.use_vss);
    let mut json = serde_json::to_value(&plan)?;
    json.as_object_mut().unwrap().remove("use_vss");
    assert!(!serde_json::from_value::<Plan>(json)?.use_vss);
    let changes = PlanChanges {
        name: plan.name.clone(),
        sources: plan.sources.clone(),
        repository: plan.repository.clone(),
        excluded_paths: plan.excluded_paths.clone(),
        excludes: plan.excludes.clone(),
        keep_last: plan.keep_last,
        use_vss: false,
        schedule: Schedule::Manual,
    };
    plan = engine.store.update(&plan.id, changes)?;
    assert!(!plan.use_vss);
    assert!(!engine.store.plan(&plan.id)?.use_vss);
    Ok(())
}

#[tokio::test]
async fn insufficient_rights_never_creates_a_live_backup() -> Result<()> {
    if straja::vss::available() {
        return Ok(());
    }
    let (_temp, engine, plan) = fixture(BackupMode::Plain)?;
    let error = engine.backup(&plan).await.unwrap_err().to_string();
    assert!(error.contains("administrator"), "{error}");
    assert_eq!(fs::read_dir(&plan.repository)?.count(), 0);
    Ok(())
}

#[tokio::test]
#[ignore = "Requires an elevated Windows process; creates only temporary owned VSS snapshots"]
async fn snapshot_reads_exclusively_locked_file_and_is_stable_then_released() -> Result<()> {
    straja::vss::require_privileges()?;
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("locked.txt");
    let mut lock = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(&source)?;
    lock.write_all(b"before")?;
    lock.sync_all()?;
    assert!(fs::read(&source).is_err());
    let snapshots = SnapshotSet::create(vec![source.clone()]).await?;
    let shadow = snapshots.resolve(&source)?;
    assert_eq!(fs::read(&shadow)?, b"before");
    lock.seek(SeekFrom::Start(0))?;
    lock.write_all(b"after!")?;
    lock.sync_all()?;
    assert_eq!(fs::read(&shadow)?, b"before");
    snapshots.close().await?;
    assert!(!shadow.exists(), "Our shadow device must be released");
    drop(lock);
    assert_eq!(fs::read(&source)?, b"after!");
    let dropped = SnapshotSet::create(vec![source.clone()]).await?;
    let dropped_path = dropped.resolve(&source)?;
    assert!(dropped_path.exists());
    drop(dropped); // Cancellation/error paths rely on Drop instead of explicit close.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while dropped_path.exists() {
        ensure!(
            std::time::Instant::now() < deadline,
            "Dropped snapshot lease was not cleaned up"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "Requires elevated Windows and bundled Restic; tests five real backup formats"]
async fn all_formats_backup_locked_files_restore_and_keep_original_exclusions() -> Result<()> {
    straja::vss::require_privileges()?;
    for mode in [
        BackupMode::Plain,
        BackupMode::Zip,
        BackupMode::TarGz,
        BackupMode::SevenZip,
        BackupMode::Encrypted,
    ] {
        let (temp, engine, plan) = fixture(mode)?;
        let lock = fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(plan.sources[0].join("locked.txt"))?;
        let single = fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(&plan.sources[1])?;
        let outcome = engine.backup(&plan).await?;
        ensure!(outcome.0 == "Reușit", "{mode:?}: {outcome:?}");
        let snapshot = engine.snapshots(&plan).await?.remove(0);
        assert_eq!(snapshot.paths.len(), 2);
        assert!(!snapshot.paths.iter().any(|p| p.contains("GLOBALROOT")));
        let parent = temp.path().join("restore");
        fs::create_dir(&parent)?;
        let restored = engine.restore(&plan, &snapshot.id, &parent).await?;
        ensure!(restored.0 == "Reușit", "{restored:?}");
        let target = fs::read_dir(&parent)?.next().unwrap()?.path();
        let mut stack = vec![target];
        let mut data = Vec::new();
        while let Some(path) = stack.pop() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                assert!(
                    !["cache", "skip.tmp", "StrajaState"].contains(&name.as_str()),
                    "{mode:?}: excluded {name}"
                );
                if entry.file_type()?.is_dir() {
                    stack.push(entry.path());
                } else {
                    data.push(fs::read(entry.path())?);
                }
            }
        }
        assert_eq!(data.len(), 2, "{mode:?}");
        assert!(data.contains(&b"saved before snapshot".to_vec()));
        assert!(data.contains(&b"single file locked".to_vec()));
        drop((lock, single));
        println!("Verified VSS backup + restore + exclusions: {mode:?}");
    }
    Ok(())
}

#[tokio::test]
#[ignore = "Requires elevated Windows; verifies error cleanup preserves another owned snapshot"]
async fn failed_backup_releases_its_set_without_deleting_another_set() -> Result<()> {
    straja::vss::require_privileges()?;
    let (_temp, engine, plan) = fixture(BackupMode::Plain)?;
    assert_eq!(engine.backup(&plan).await?.0, "Reușit");
    let previous = engine.snapshots(&plan).await?.remove(0).id;
    let other = SnapshotSet::create(plan.sources.clone()).await?;
    let other_file = other.resolve(&plan.sources[0].join("locked.txt"))?;
    std::os::windows::fs::symlink_dir(&plan.sources[0], plan.sources[0].join("unsupported-link"))?;
    let error = engine.backup(&plan).await.unwrap_err();
    assert!(format!("{error:#}").contains("reparse point"));
    assert_eq!(fs::read(&other_file)?, b"saved before snapshot");
    let remaining = engine.snapshots(&plan).await?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, previous);
    assert_eq!(engine.check(&plan).await?.0, "Reușit");
    other.close().await?;
    assert!(!other_file.exists());
    Ok(())
}
