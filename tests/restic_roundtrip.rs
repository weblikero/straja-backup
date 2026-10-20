use anyhow::Result;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use straja::{engine::Engine, model::*, store::Store};

#[tokio::test]
#[ignore = "Requires the verified Restic executable; run scripts/test.ps1"]
async fn real_backup_incremental_history_restore_and_integrity() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let binary = root
        .join(".tools")
        .join(straja::platform::executable("restic"));
    anyhow::ensure!(binary.is_file(), "Run scripts/get-restic.ps1 first");
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("Documente cu spații și diacritice");
    let repo = temp.path().join("backup");
    let restore = temp.path().join("restore");
    std::fs::create_dir_all(source.join("subfolder"))?;
    std::fs::create_dir(&repo)?;
    std::fs::create_dir(&restore)?;
    std::fs::write(source.join("subfolder/notițe.txt"), "versiunea inițială")?;
    std::fs::write(source.join("temporar.tmp"), "excluded")?;
    // Even if a user backs up a parent folder, the application's database,
    // encrypted credentials and live cache must never become backup sources.
    let store = Store::open(&source.join("StrajaState"))?;
    let p = store.add(NewPlan {
        remote: None,
        remote_credentials: None,
        mode: BackupMode::Encrypted,
        name: "Roundtrip".into(),
        sources: vec![source.clone()],
        repository: repo,
        excluded_paths: vec![],
        excludes: vec!["*.tmp".into()],
        keep_last: None,
        use_vss: false,
        schedule: Schedule::Manual,
        password: "integration-test-only".into(),
    })?;
    let engine = Engine {
        binary,
        store,
        live: Arc::new(Mutex::new(Some(Progress::default()))),
    };
    assert_eq!(engine.backup(&p).await?.0, "Reușit");
    engine.store.finish_backup(&p.id, true)?;
    let first = engine.snapshots(&p).await?;
    assert_eq!(first.len(), 1);
    std::fs::write(
        source.join("subfolder/notițe.txt"),
        "versiunea nouă și mai lungă",
    )?;
    std::fs::write(source.join("nou.txt"), "new file")?;
    assert_eq!(engine.backup(&p).await?.0, "Reușit");
    assert_eq!(engine.snapshots(&p).await?.len(), 2);
    assert_eq!(engine.check(&p).await?.0, "Reușit");
    assert_eq!(
        engine.restore(&p, &first[0].id, &restore).await?.0,
        "Reușit"
    );
    fn files(path: &std::path::Path, out: &mut Vec<PathBuf>) -> Result<()> {
        for item in std::fs::read_dir(path)? {
            let path = item?.path();
            if path.is_dir() {
                files(&path, out)?;
            } else {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut restored = vec![];
    files(&restore, &mut restored)?;
    assert_eq!(
        restored.len(),
        1,
        "Excluded files and files added after snapshot must not appear"
    );
    assert_eq!(std::fs::read_to_string(&restored[0])?, "versiunea inițială");
    assert_eq!(
        std::fs::read_to_string(source.join("subfolder/notițe.txt"))?,
        "versiunea nouă și mai lungă"
    );
    assert!(
        engine
            .restore(&p, "not-a-snapshot", &restore)
            .await
            .is_err()
    );
    let mut wrong = p.clone();
    wrong.id = engine
        .store
        .add(NewPlan {
            remote: None,
            remote_credentials: None,
            mode: BackupMode::Encrypted,
            name: "Wrong key".into(),
            sources: vec![source],
            repository: {
                let dir = temp.path().join("second");
                std::fs::create_dir(&dir)?;
                dir
            },
            excluded_paths: vec![],
            excludes: vec![],
            keep_last: None,
            use_vss: false,
            schedule: Schedule::Manual,
            password: "wrong-test-key".into(),
        })?
        .id;
    assert!(
        engine.snapshots(&wrong).await.is_err(),
        "A wrong password must be rejected"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let locked_path = p.sources[0].join("locked.txt");
        std::fs::write(&locked_path, "locked data")?;
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&locked_path)?;
        assert_eq!(
            engine.backup(&p).await?.0,
            "Incomplet",
            "An unreadable file must never be reported as a successful full backup"
        );
        drop(locked);
    }

    let foreign = std::process::Command::new(&engine.binary)
        .arg("--repo")
        .arg(&p.repository)
        .args(["backup", "--tag", "foreign-plan", "--exclude"])
        .arg(&engine.store.root)
        .arg("--")
        .arg(&p.sources[0])
        .env("RESTIC_PASSWORD", "integration-test-only")
        .output()?;
    assert!(
        foreign.status.success(),
        "{}",
        String::from_utf8_lossy(&foreign.stderr)
    );
    // Retention also applies to encrypted plans. Failed/incomplete backups never prune.
    let mut limited = p.clone();
    limited.keep_last = Some(1);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let locked_path = p.sources[0].join("locked.txt");
        let before = engine.snapshots(&limited).await?;
        std::fs::write(
            &locked_path,
            "changed after the foreign snapshot, so Restic must read it again",
        )?;
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&locked_path)?;
        assert_eq!(engine.backup(&limited).await?.0, "Incomplet");
        let after_failure = engine.snapshots(&limited).await?;
        assert!(
            before
                .iter()
                .all(|s| after_failure.iter().any(|a| a.id == s.id))
        );
        drop(locked);
    }
    let result = engine.backup(&limited).await?;
    assert_eq!(result.0, "Reușit");
    assert!(!result.1.contains("ATENȚIE"), "{result:?}");
    let kept = engine.snapshots(&limited).await?;
    assert_eq!(kept.len(), 1);
    engine.restore(&limited, &kept[0].id, &restore).await?;
    engine.check(&limited).await?;
    let foreign = std::process::Command::new(&engine.binary)
        .arg("--repo")
        .arg(&p.repository)
        .args(["snapshots", "--json", "--tag", "foreign-plan"])
        .env("RESTIC_PASSWORD", "integration-test-only")
        .output()?;
    assert!(foreign.status.success());
    assert_eq!(
        serde_json::from_slice::<Vec<serde_json::Value>>(&foreign.stdout)?.len(),
        1,
        "Foreign snapshots must survive retention/prune"
    );
    Ok(())
}
