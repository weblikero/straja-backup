use anyhow::Result;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use straja::{engine::Engine, exclusions, model::*, store::Store};

fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut stack = vec![root.to_path_buf()];
    let mut result = Vec::new();
    while let Some(path) = stack.pop() {
        for child in fs::read_dir(path)? {
            let path = child?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                result.push(path);
            }
        }
    }
    Ok(result)
}

#[tokio::test]
async fn selected_exclusions_plain_and_all_archives() -> Result<()> {
    for mode in [
        BackupMode::Plain,
        BackupMode::Zip,
        BackupMode::TarGz,
        BackupMode::SevenZip,
    ] {
        exercise(mode).await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "Requires verified Restic executable; run scripts/test.ps1"]
async fn selected_exclusions_restic() -> Result<()> {
    exercise(BackupMode::Encrypted).await
}

async fn exercise(mode: BackupMode) -> Result<()> {
    let temp = tempfile::tempdir()?;
    // Brackets and semicolon are literal filename characters, not patterns/separators.
    let source = temp.path().join("Folder A [teste]; și spații");
    let excluded_folder = source.join("FolderB[1]");
    let same_name = source.join("AltFolder/FolderB[1]");
    let prefix_sibling = source.join("FolderB[1]-păstrat");
    let repo = temp.path().join("repo");
    let restore = temp.path().join("restore");
    for p in [
        &excluded_folder.join("subfolder"),
        &same_name,
        &prefix_sibling,
        &repo,
        &restore,
    ] {
        fs::create_dir_all(p)?;
    }
    fs::write(
        excluded_folder.join("subfolder/secret.txt"),
        "excluded folder",
    )?;
    fs::write(same_name.join("same-name.txt"), "included other folder")?;
    fs::write(prefix_sibling.join("prefix.txt"), "included prefix sibling")?;
    let excluded_file = source.join("privat[2];.txt");
    fs::write(&excluded_file, "excluded file")?;
    fs::write(
        source.join("privat2;.txt"),
        "included different literal name",
    )?;
    fs::write(source.join("keep.txt"), "included main")?;
    fs::write(source.join("temp.tmp"), "excluded by wildcard")?;
    let store = Store::open(&temp.path().join("state"))?;
    let plan = store.add(NewPlan {
        mode,
        name: "Excluderi selectate".into(),
        sources: vec![source.clone()],
        repository: repo,
        excluded_paths: vec![excluded_folder.clone(), excluded_file.clone()],
        excludes: vec!["*.tmp".into()],
        keep_last: None,
        schedule: Schedule::Manual,
        password: "test-password-only".into(),
    })?;
    assert_eq!(
        Store::open(&store.root)?.plan(&plan.id)?.excluded_paths,
        plan.excluded_paths
    );
    let engine = Engine {
        binary: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".tools/restic.exe"),
        store,
        live: Arc::new(Mutex::new(None)),
    };
    // Exclusions must be applied before opening locked files, including nested files.
    use std::os::windows::fs::OpenOptionsExt;
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&excluded_file)?;
    let locked_nested = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(excluded_folder.join("subfolder/secret.txt"))?;
    let result = engine.backup(&plan).await?;
    assert_eq!(result.0, "Reușit", "{mode}: {result:?}");
    drop((locked, locked_nested));
    let first = engine.snapshots(&plan).await?.remove(0);
    engine.restore(&plan, &first.id, &restore).await?;
    let mut contents: Vec<_> = files(&restore)?
        .iter()
        .map(fs::read_to_string)
        .collect::<std::io::Result<_>>()?;
    contents.sort();
    assert_eq!(
        contents,
        [
            "included different literal name",
            "included main",
            "included other folder",
            "included prefix sibling"
        ],
        "{mode}"
    );
    assert_eq!(fs::read_to_string(&excluded_file)?, "excluded file");
    // New files under an excluded folder stay excluded in later backups.
    fs::write(excluded_folder.join("later.txt"), "new excluded content")?;
    engine.backup(&plan).await?;
    let middle = engine.snapshots(&plan).await?.remove(0);
    let middle_restore = temp.path().join("restore-middle");
    fs::create_dir(&middle_restore)?;
    engine.restore(&plan, &middle.id, &middle_restore).await?;
    let middle_contents: Vec<_> = files(&middle_restore)?
        .iter()
        .map(fs::read_to_string)
        .collect::<std::io::Result<_>>()?;
    assert_eq!(middle_contents.len(), 4);
    assert!(!middle_contents.iter().any(|s| s.contains("excluded")));
    // Removing selected exclusions from the plan includes those paths on the next run.
    let edited = engine.store.update(
        &plan.id,
        PlanChanges {
            name: plan.name.clone(),
            sources: plan.sources.clone(),
            repository: plan.repository.clone(),
            excluded_paths: vec![],
            excludes: plan.excludes.clone(),
            keep_last: None,
            schedule: plan.schedule.clone(),
        },
    )?;
    engine.backup(&edited).await?;
    let latest = engine.snapshots(&edited).await?.remove(0);
    let restored_again = temp.path().join("restore-new");
    fs::create_dir(&restored_again)?;
    engine.restore(&edited, &latest.id, &restored_again).await?;
    let restored: Vec<_> = files(&restored_again)?
        .iter()
        .map(fs::read_to_string)
        .collect::<std::io::Result<_>>()?;
    for expected in ["excluded file", "excluded folder", "new excluded content"] {
        assert!(
            restored.iter().any(|text| text == expected),
            "{mode}: {expected}"
        );
    }
    engine.check(&edited).await?;
    assert_eq!(
        engine.snapshots(&edited).await?.len(),
        3,
        "Editing exclusions must keep old versions"
    );
    Ok(())
}

#[test]
fn literal_paths_boundaries_validation_and_legacy_defaults() -> Result<()> {
    let source = PathBuf::from(r"C:\FolderA");
    let paths = vec![source.join("FolderB")];
    exclusions::validate_paths(std::slice::from_ref(&source), &paths)?;
    assert!(
        exclusions::PathMatcher::new(&paths).contains(Path::new(r"c:\foldera\folderb\new.txt"))
    );
    assert!(
        exclusions::PathMatcher::new(&paths).contains(Path::new(r"\\?\C:\FolderA\FolderB\new.txt"))
    );
    assert!(
        !exclusions::PathMatcher::new(&paths)
            .contains(Path::new(r"C:\FolderA\FolderB-copy\new.txt"))
    );
    assert!(
        !exclusions::PathMatcher::new(&paths)
            .contains(Path::new(r"C:\FolderA\Other\FolderB\new.txt"))
    );
    for bad in [
        r"C:\FolderA",
        r"C:\FolderA-other\x",
        r"C:\FolderA\..\Other",
        r"FolderB",
        r"C:\FolderA\*",
    ] {
        assert!(
            exclusions::validate_paths(std::slice::from_ref(&source), &[bad.into()]).is_err(),
            "{bad}"
        );
    }
    assert!(
        exclusions::validate_paths(&[source.clone(), source.join("FolderB/file.txt")], &paths)
            .is_err()
    );
    assert_eq!(
        exclusions::normalized(Path::new(r"\\?\UNC\Server\Share\Folder")),
        "//server/share/folder"
    );
    let legacy = serde_json::json!({"id":"id", "name":"legacy", "sources":["C:\\FolderA"], "repository":"E:\\Backup", "excludes":["*.tmp"], "schedule":"Manual", "enabled":true, "next_due":null, "last_success":null});
    assert!(
        serde_json::from_value::<Plan>(legacy)?
            .excluded_paths
            .is_empty()
    );
    Ok(())
}
