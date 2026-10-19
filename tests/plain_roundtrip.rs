use anyhow::Result;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use straja::{engine::Engine, model::*, store::Store};

struct Fixture {
    _temp: tempfile::TempDir,
    engine: Engine,
    plan: Plan,
    restore: PathBuf,
}
fn fixture() -> Result<Fixture> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("Documente și poze");
    let second = temp.path().join("alt folder/Documente și poze");
    let single = temp.path().join("separat.txt");
    let repo = temp.path().join("copii");
    let restore = temp.path().join("restore");
    for p in [
        &source.join("gol"),
        &source.join("cache"),
        &second,
        &repo,
        &restore,
    ] {
        fs::create_dir_all(p)?;
    }
    fs::write(source.join("notițe.txt"), "versiunea inițială")?;
    fs::write(source.join("temporar.TMP"), "excluded")?;
    fs::write(source.join("cache/secret.txt"), "excluded directory")?;
    fs::write(second.join("notițe.txt"), "another source")?;
    fs::write(&single, "single source")?;
    let store = Store::open(&source.join("StrajaState"))?;
    let plan = store.add(NewPlan {
        mode: BackupMode::Plain,
        name: "Fără parolă".into(),
        sources: vec![source, second, single],
        repository: repo,
        excluded_paths: vec![],
        excludes: vec!["*.tmp".into(), "cache".into()],
        keep_last: None,
        use_vss: false,
        schedule: Schedule::Manual,
        password: String::new(),
    })?;
    let engine = Engine {
        binary: temp.path().join("restic-deliberately-absent.exe"),
        store,
        live: Arc::new(Mutex::new(Some(Progress::default()))),
    };
    Ok(Fixture {
        _temp: temp,
        engine,
        plan,
        restore,
    })
}

#[tokio::test]
async fn readable_versions_restore_exclusions_and_edit_without_password() -> Result<()> {
    let f = fixture()?;
    let secret: Vec<u8> = rusqlite::Connection::open(f.engine.store.root.join("straja.sqlite"))?
        .query_row(
            "SELECT secret FROM plans WHERE id=?1",
            [&f.plan.id],
            |row| row.get(0),
        )?;
    assert!(secret.is_empty(), "Plain plans must not persist a password");
    assert_eq!(f.engine.backup(&f.plan).await?.0, "Reușit");
    f.engine.store.finish_backup(&f.plan.id, true)?;
    let first = f.engine.snapshots(&f.plan).await?.remove(0);
    let files = f.plan.repository.join(&first.id).join("Fisiere");
    let document = files.join("01-Documente și poze/notițe.txt");
    assert_eq!(fs::read_to_string(&document)?, "versiunea inițială");
    assert_eq!(
        fs::read_to_string(files.join("02-Documente și poze/notițe.txt"))?,
        "another source"
    );
    assert_eq!(
        fs::read_to_string(files.join("03-separat.txt"))?,
        "single source"
    );
    assert!(files.join("01-Documente și poze/gol").is_dir());
    for excluded in ["StrajaState", "cache", "temporar.TMP"] {
        assert!(!files.join("01-Documente și poze").join(excluded).exists());
    }
    assert_eq!(
        fs::metadata(&document)?.modified()?,
        fs::metadata(f.plan.sources[0].join("notițe.txt"))?.modified()?
    );
    fs::write(f.plan.sources[0].join("notițe.txt"), "versiunea nouă")?;
    fs::remove_file(&f.plan.sources[2])?;
    let changes = PlanChanges {
        name: "Editat".into(),
        sources: f.plan.sources[..2].to_vec(),
        repository: f.plan.repository.clone(),
        excluded_paths: vec![],
        excludes: f.plan.excludes.clone(),
        keep_last: None,
        use_vss: false,
        schedule: Schedule::Interval { minutes: 90 },
    };
    let edited = f.engine.store.update(&f.plan.id, changes.clone())?;
    assert_eq!(edited.mode, BackupMode::Plain);
    assert!(edited.last_success.is_some());
    assert_eq!(f.engine.backup(&edited).await?.0, "Reușit");
    assert_eq!(f.engine.snapshots(&edited).await?.len(), 2);
    assert_eq!(fs::read_to_string(&document)?, "versiunea inițială");
    assert_eq!(f.engine.check(&edited).await?.0, "Reușit");
    assert_eq!(
        f.engine.restore(&edited, &first.id, &f.restore).await?.0,
        "Reușit"
    );
    let restored = fs::read_dir(&f.restore)?.next().unwrap()?.path();
    assert_eq!(
        fs::read_to_string(restored.join("01-Documente și poze/notițe.txt"))?,
        "versiunea inițială"
    );
    assert_eq!(
        fs::read_to_string(restored.join("03-separat.txt"))?,
        "single source"
    );
    assert_eq!(
        fs::read_to_string(f.plan.sources[0].join("notițe.txt"))?,
        "versiunea nouă"
    );
    assert!(!f.plan.sources[2].exists());
    let mut moved = changes;
    moved.repository = f.restore.clone();
    assert!(f.engine.store.update(&f.plan.id, moved).is_err());
    fs::write(&document, "corrupted")?;
    assert!(f.engine.check(&edited).await.is_err());
    assert!(
        f.engine
            .restore(&edited, &first.id, &f.restore)
            .await
            .is_err()
    );
    assert_eq!(
        fs::read_dir(&f.restore)?.count(),
        1,
        "Corrupt backups must not be restored"
    );
    Ok(())
}

#[tokio::test]
async fn mixed_file_sizes_survive_reused_buffers_and_restore() -> Result<()> {
    let f = fixture()?;
    let mut expected = Vec::new();
    // Cross the I/O chunk boundary, then alternate short and empty files. This
    // catches stale bytes from earlier reads leaking into the next file or hash.
    for (i, size) in [524_289, 3, 0, 262_143, 262_144, 262_145, 1]
        .into_iter()
        .cycle()
        .take(28)
        .enumerate()
    {
        let name = format!("mixed-{i:02}.bin");
        let bytes: Vec<u8> = (0..size).map(|n| ((n + i) % 251) as u8).collect();
        fs::write(f.plan.sources[0].join(&name), &bytes)?;
        expected.push((name, bytes));
    }
    f.engine.backup(&f.plan).await?;
    f.engine.check(&f.plan).await?;
    let snapshot = f.engine.snapshots(&f.plan).await?.remove(0);
    f.engine.restore(&f.plan, &snapshot.id, &f.restore).await?;
    let restored = fs::read_dir(&f.restore)?.next().unwrap()?.path();
    for (name, bytes) in expected {
        assert_eq!(
            fs::read(restored.join("01-Documente și poze").join(name))?,
            bytes
        );
    }
    Ok(())
}

#[tokio::test]
async fn locked_source_fails_without_publishing_partial_backup() -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    let f = fixture()?;
    f.engine.backup(&f.plan).await?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&f.plan.sources[2])?;
    let error = f.engine.backup(&f.plan).await.unwrap_err();
    assert!(format!("{error:#}").contains("posibil blocat"));
    assert_eq!(f.engine.snapshots(&f.plan).await?.len(), 1);
    assert!(fs::read_dir(&f.plan.repository)?.any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".in-progress-")
    }));
    drop(lock);
    f.engine.backup(&f.plan).await?;
    assert_eq!(f.engine.snapshots(&f.plan).await?.len(), 2);
    f.engine.check(&f.plan).await?;
    Ok(())
}

#[tokio::test]
async fn rejects_untrusted_paths_and_foreign_repository() -> Result<()> {
    let f = fixture()?;
    f.engine.backup(&f.plan).await?;
    let snapshot = f.engine.snapshots(&f.plan).await?.remove(0);
    let mut other = f.plan.clone();
    other.id = "other-plan".into();
    assert!(f.engine.backup(&other).await.is_err());
    assert!(f.engine.snapshots(&other).await.is_err());
    assert!(
        f.engine
            .restore(&f.plan, "../outside", &f.restore)
            .await
            .is_err()
    );
    let path = f
        .plan
        .repository
        .join(&snapshot.id)
        .join(".straja-manifest.json");
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    for invalid in [
        r"Fisiere\..\..\outside.txt",
        r"C:\outside.txt",
        r"Fisiere\file.txt:stream",
        r"Fisiere\bad.",
    ] {
        let mut altered = original.clone();
        altered["entries"][0]["path"] = invalid.into();
        fs::write(&path, serde_json::to_vec(&altered)?)?;
        assert!(
            f.engine
                .restore(&f.plan, &snapshot.id, &f.restore)
                .await
                .is_err(),
            "Accepted {invalid}"
        );
        assert_eq!(fs::read_dir(&f.restore)?.count(), 0);
    }
    Ok(())
}

#[test]
fn legacy_plans_remain_encrypted_and_plain_patterns_are_validated() -> Result<()> {
    let f = fixture()?;
    let mut legacy = serde_json::to_value(&f.plan)?;
    legacy.as_object_mut().unwrap().remove("mode");
    assert_eq!(
        serde_json::from_value::<Plan>(legacy)?.mode,
        BackupMode::Encrypted
    );
    assert!(straja::plain::exclusions(&["!include".into()]).is_err());
    assert!(straja::plain::exclusions(&["[".into()]).is_err());
    Ok(())
}
