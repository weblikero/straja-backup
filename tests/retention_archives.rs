use anyhow::Result;
use std::{
    fs,
    io::Read,
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
fn fixture(mode: BackupMode, keep_last: Option<u32>) -> Result<Fixture> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("Documente și poze");
    let repo = temp.path().join("backup");
    let restore = temp.path().join("restore");
    for p in [&source.join("gol"), &repo, &restore] {
        fs::create_dir_all(p)?;
    }
    fs::write(
        source.join("notițe.txt"),
        "conținut inițial\n".repeat(30000),
    )?;
    fs::write(source.join("exclus.TMP"), "omit")?;
    fs::write(source.join("zero.txt"), "")?;
    let single = temp.path().join("separat.txt");
    fs::write(&single, "sursă individuală")?;
    let store = Store::open(&temp.path().join("state"))?;
    let plan = store.add(NewPlan {
        mode,
        keep_last,
        use_vss: false,
        name: "Arhive".into(),
        sources: vec![source, single],
        repository: repo,
        excluded_paths: vec![],
        excludes: vec!["*.tmp".into()],
        schedule: Schedule::Manual,
        password: String::new(),
    })?;
    let engine = Engine {
        binary: temp.path().join("missing-restic.exe"),
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

// Read using the format libraries directly, independently of Straja's manifest/extraction code.
fn independently_read(path: &std::path::Path, mode: BackupMode) -> Result<String> {
    let wanted = "Fisiere/01-Documente și poze/notițe.txt";
    let mut result = String::new();
    match mode {
        BackupMode::Zip => {
            let mut reader = zip::ZipArchive::new(fs::File::open(path)?)?;
            reader.by_name(wanted)?.read_to_string(&mut result)?;
            assert!(
                reader
                    .by_name("Fisiere/01-Documente și poze/gol/")?
                    .is_dir()
            );
            assert!(
                reader
                    .by_name("Fisiere/01-Documente și poze/exclus.TMP")
                    .is_err()
            );
        }
        BackupMode::TarGz => {
            let mut reader = tar::Archive::new(flate2::read::GzDecoder::new(fs::File::open(path)?));
            for entry in reader.entries()? {
                let mut entry = entry?;
                if entry.path()?.to_string_lossy().replace('\\', "/") == wanted {
                    entry.read_to_string(&mut result)?;
                }
            }
        }
        BackupMode::SevenZip => {
            let mut reader =
                sevenz_rust2::ArchiveReader::open(path, sevenz_rust2::Password::empty())?;
            result = String::from_utf8(reader.read_file(wanted)?)?;
        }
        _ => unreachable!(),
    }
    Ok(result)
}

#[tokio::test]
async fn all_archive_formats_roundtrip_compress_and_keep_two() -> Result<()> {
    for mode in [BackupMode::Zip, BackupMode::TarGz, BackupMode::SevenZip] {
        let f = fixture(mode, Some(2))?;
        let original = fs::read_to_string(f.plan.sources[0].join("notițe.txt"))?;
        let mut ids = Vec::new();
        for i in 0..3 {
            if i > 0 {
                fs::write(
                    f.plan.sources[0].join("notițe.txt"),
                    format!("versiunea {i}\n").repeat(30000),
                )?;
            }
            let result = f.engine.backup(&f.plan).await?;
            assert_eq!(result.0, "Reușit", "{mode}: {result:?}");
            assert!(!result.1.contains("ATENȚIE"), "{mode}: {result:?}");
            let copies = f.engine.snapshots(&f.plan).await?;
            assert_eq!(copies.len(), (i + 1).min(2));
            let folder = f.plan.repository.join(&copies[0].id);
            ids.push(copies[0].id.clone());
            let archive = folder.join(mode.archive_name().unwrap());
            assert!(archive.is_file());
            assert!(
                !folder.join("Fisiere").exists(),
                "Staged plaintext must not remain on success"
            );
            assert!(
                fs::metadata(&archive)?.len() < original.len() as u64 / 2,
                "Real compression expected"
            );
            let expected = fs::read_to_string(f.plan.sources[0].join("notițe.txt"))?;
            assert_eq!(independently_read(&archive, mode)?, expected);
            f.engine.restore(&f.plan, &copies[0].id, &f.restore).await?;
            let restored = fs::read_dir(&f.restore)?.next().unwrap()?.path();
            assert_eq!(
                fs::read_to_string(restored.join("01-Documente și poze/notițe.txt"))?,
                expected
            );
            assert_eq!(
                fs::read_to_string(restored.join("02-separat.txt"))?,
                "sursă individuală"
            );
            assert!(restored.join("01-Documente și poze/gol").is_dir());
            assert!(!restored.join("01-Documente și poze/exclus.TMP").exists());
            assert_eq!(
                fs::metadata(restored.join("01-Documente și poze/notițe.txt"))?.modified()?,
                fs::metadata(f.plan.sources[0].join("notițe.txt"))?.modified()?
            );
            // Only the test-owned restoration folder is removed.
            assert!(restored.starts_with(f._temp.path()));
            fs::remove_dir_all(restored)?;
        }
        assert!(!f.plan.repository.join(&ids[0]).exists());
        assert!(f.plan.repository.join(&ids[1]).exists());
        f.engine.check(&f.plan).await?;
        assert!(!fs::read_dir(&f.plan.repository)?.any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".in-progress")
        }));
    }
    Ok(())
}

#[tokio::test]
async fn retention_default_edit_failure_unknown_files_and_clock_reversal() -> Result<()> {
    let f = fixture(BackupMode::Plain, None)?;
    for _ in 0..3 {
        f.engine.backup(&f.plan).await?;
    }
    let originals = f.engine.snapshots(&f.plan).await?;
    assert_eq!(originals.len(), 3);
    let changes = PlanChanges {
        name: f.plan.name.clone(),
        sources: f.plan.sources.clone(),
        repository: f.plan.repository.clone(),
        excluded_paths: vec![],
        excludes: f.plan.excludes.clone(),
        schedule: Schedule::Manual,
        keep_last: Some(1),
        use_vss: false,
    };
    let edited = f.engine.store.update(&f.plan.id, changes)?;
    assert_eq!(f.engine.store.plan(&edited.id)?.keep_last, Some(1));
    assert_eq!(
        f.engine.snapshots(&edited).await?.len(),
        3,
        "Editing must not delete anything"
    );
    use std::os::windows::fs::OpenOptionsExt;
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(edited.sources[0].join("notițe.txt"))?;
    assert!(f.engine.backup(&edited).await.is_err());
    assert_eq!(f.engine.snapshots(&edited).await?.len(), 3);
    drop(locked);
    // A newer wall clock on old snapshots must never cause deletion of the just-completed copy.
    for copy in &originals {
        let path = edited
            .repository
            .join(&copy.id)
            .join(".straja-manifest.json");
        let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
        json["time"] = "2099-01-01T00:00:00Z".into();
        fs::write(path, serde_json::to_vec(&json)?)?;
    }
    let foreign = edited
        .repository
        .join(&originals[0].id)
        .join("my-extra-file.txt");
    fs::write(&foreign, "do not delete")?;
    let result = f.engine.backup(&edited).await?;
    assert!(result.1.contains("ATENȚIE"));
    assert_eq!(fs::read_to_string(&foreign)?, "do not delete");
    fs::remove_file(foreign)?;
    let result = f.engine.backup(&edited).await?;
    assert!(!result.1.contains("ATENȚIE"), "{result:?}");
    let kept = f.engine.snapshots(&edited).await?;
    assert_eq!(kept.len(), 1);
    assert!(kept[0].time.year() < 2099);
    assert!(
        originals
            .iter()
            .all(|s| !edited.repository.join(&s.id).exists())
    );
    assert!(
        fs::read_dir(&edited.repository)?.any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".in-progress")),
        "Partial copies are not retention candidates"
    );
    Ok(())
}
use chrono::Datelike;

#[tokio::test]
async fn corrupt_archives_block_restore_and_retention() -> Result<()> {
    for mode in [BackupMode::Zip, BackupMode::TarGz, BackupMode::SevenZip] {
        let f = fixture(mode, Some(1))?;
        f.engine.backup(&f.plan).await?;
        let first = f.engine.snapshots(&f.plan).await?.remove(0);
        let archive = f
            .plan
            .repository
            .join(&first.id)
            .join(mode.archive_name().unwrap());
        fs::write(&archive, "damaged archive")?;
        assert!(f.engine.check(&f.plan).await.is_err());
        assert!(
            f.engine
                .restore(&f.plan, &first.id, &f.restore)
                .await
                .is_err()
        );
        assert_eq!(fs::read_dir(&f.restore)?.count(), 0);
        let result = f.engine.backup(&f.plan).await?;
        assert_eq!(result.0, "Reușit cu avertisment");
        assert!(result.1.contains("ATENȚIE"), "{mode}: {result:?}");
        assert!(archive.is_file());
        assert_eq!(f.engine.snapshots(&f.plan).await?.len(), 2);
    }
    Ok(())
}

#[tokio::test]
async fn archive_traversal_and_foreign_repository_are_rejected() -> Result<()> {
    let f = fixture(BackupMode::Zip, Some(1))?;
    f.engine.backup(&f.plan).await?;
    let first = f.engine.snapshots(&f.plan).await?.remove(0);
    let path = f.plan.repository.join(&first.id).join("Backup.zip");
    // Replace an entry without changing archive count, retaining valid CRCs.
    let mut original = zip::ZipArchive::new(fs::File::open(&path)?)?;
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..original.len() {
        let mut e = original.by_index(i)?;
        let name = if e.name().ends_with("notițe.txt") {
            "../escape.txt".into()
        } else {
            e.name().to_string()
        };
        let options = zip::write::SimpleFileOptions::default();
        if e.is_dir() {
            writer.add_directory(name, options)?;
        } else {
            writer.start_file(name, options)?;
            std::io::copy(&mut e, &mut writer)?;
        }
    }
    drop(original);
    fs::write(&path, writer.finish()?.into_inner())?;
    assert!(
        f.engine
            .restore(&f.plan, &first.id, &f.restore)
            .await
            .is_err()
    );
    assert_eq!(fs::read_dir(&f.restore)?.count(), 0);
    assert!(!f._temp.path().join("escape.txt").exists());
    let marker = f.plan.repository.join(straja::plain::MARKER);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&marker)?)?;
    json["plan_id"] = "foreign-plan".into();
    fs::write(&marker, serde_json::to_vec(&json)?)?;
    assert!(f.engine.backup(&f.plan).await.is_err());
    assert!(path.is_file());
    Ok(())
}

#[test]
fn legacy_plans_keep_all_and_invalid_limits_are_rejected() -> Result<()> {
    let f = fixture(BackupMode::Plain, None)?;
    let mut json = serde_json::to_value(&f.plan)?;
    json.as_object_mut().unwrap().remove("keep_last");
    assert_eq!(serde_json::from_value::<Plan>(json)?.keep_last, None);
    assert!(validate_retention(Some(0)).is_err());
    assert!(validate_retention(Some(10001)).is_err());
    assert!(validate_retention(Some(1)).is_ok());
    Ok(())
}
