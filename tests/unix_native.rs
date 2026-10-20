#![cfg(unix)]
use anyhow::Result;
use std::{path::Path, time::Duration};
use straja::{
    model::*,
    protocol::{self, Request, Response},
};

#[test]
fn unix_exclusions_preserve_case_and_boundaries() -> Result<()> {
    use straja::exclusions::{PathMatcher, validate_paths};
    let sources = vec!["/home/person/Projects".into()];
    let excluded = vec!["/home/person/Projects/Cache".into()];
    validate_paths(&sources, &excluded)?;
    let matcher = PathMatcher::new(&excluded);
    assert!(matcher.contains(Path::new("/home/person/Projects/Cache/nested/file")));
    assert!(!matcher.contains(Path::new("/home/person/Projects/cache/file")));
    assert!(!matcher.contains(Path::new("/home/person/Projects/Cache-copy/file")));
    for path in [
        "/home/person/Projects",
        "/home/person/Other",
        "/home/person/Projects/../Secret",
    ] {
        assert!(validate_paths(&sources, &[path.into()]).is_err());
    }
    assert!(overlaps(Path::new("/"), Path::new("/home/a")));
    assert!(!overlaps(Path::new("/home/A"), Path::new("/home/a")));
    assert_eq!(
        straja::engine::restore_selector("abc", &["/home/a/docs".into()]),
        "abc:/home/a"
    );
    Ok(())
}

async fn call(path: &Path, value: Request) -> Result<Response> {
    let mut stream = tokio::net::UnixStream::connect(path).await?;
    straja::unix_transport::verify_peer(&stream)?;
    protocol::write(&mut stream, &value).await?;
    protocol::read(&mut stream).await
}

#[test]
fn overlapping_directories_are_rejected_through_filesystem_aliases() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("CaseRoot");
    let destination = source.join("backup");
    std::fs::create_dir_all(&destination)?;
    let alias = temp.path().join("alias");
    std::os::unix::fs::symlink(&source, &alias)?;
    assert!(overlaps(&alias, &destination));
    assert!(overlaps(&destination, &alias));
    let different_case = temp.path().join("caseroot");
    if different_case.exists() {
        assert!(overlaps(&different_case, &destination));
        assert!(overlaps(&destination, &different_case));
    }
    let unrelated = temp.path().join("unrelated");
    std::fs::create_dir(&unrelated)?;
    assert!(!overlaps(&source, &unrelated));
    Ok(())
}

#[tokio::test]
async fn unix_agent_runs_backup_preserves_state_and_prevents_duplicate_listener() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    let destination = temp.path().join("backup");
    std::fs::create_dir(&source)?;
    std::fs::create_dir(&destination)?;
    std::fs::write(source.join("hello.txt"), "native backup")?;
    let root = temp.path().join("state");
    let store = straja::store::Store::open(&root)?;
    let plan = store.add(NewPlan {
        name: "Native".into(),
        mode: BackupMode::Plain,
        sources: vec![source],
        repository: destination,
        remote: None,
        remote_credentials: None,
        excluded_paths: vec![],
        excludes: vec![],
        password: String::new(),
        use_vss: false,
        keep_last: Some(2),
        schedule: Schedule::Manual,
    })?;
    let socket = root.join("agent.sock");
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let agent = tokio::spawn(straja::worker::run_on_pipe(
        root.clone(),
        "unused-restic".into(),
        false,
        rx,
        socket.to_str().unwrap().into(),
    ));
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(straja::unix_transport::Listener::bind(socket.to_str().unwrap()).is_err());
    assert!(matches!(
        call(&socket, Request::Backup(plan.id.clone())).await?,
        Response::Started(_)
    ));
    let mut finished = false;
    for _ in 0..200 {
        if let Response::State(state) = call(&socket, Request::State).await?
            && state.active.is_none()
        {
            assert_eq!(state.history[0].outcome, "Reușit");
            finished = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(finished, "Native worker did not finish");
    assert!(matches!(
        call(&socket, Request::Shutdown).await?,
        Response::Ok(_)
    ));
    tokio::time::timeout(Duration::from_secs(10), agent).await???;
    assert!(!socket.exists());
    assert!(
        straja::store::Store::open(&root)?
            .plan(&plan.id)?
            .last_success
            .is_some()
    );
    let restarted = straja::unix_transport::Listener::bind(socket.to_str().unwrap())?;
    drop(restarted);
    Ok(())
}

#[test]
fn private_ipc_and_keychain_ciphertext_are_enforced() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir()?;
    let public = temp.path().join("public");
    std::fs::create_dir(&public)?;
    std::fs::set_permissions(&public, std::fs::Permissions::from_mode(0o755))?;
    assert!(
        straja::unix_transport::Listener::bind(public.join("agent.sock").to_str().unwrap())
            .is_err()
    );
    let secret = b"native-regression-secret";
    let encrypted = straja::security::protect(secret)?;
    assert!(!encrypted.windows(secret.len()).any(|w| w == secret));
    assert_eq!(straja::security::unprotect(&encrypted)?, secret);
    let mut corrupt = encrypted;
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(straja::security::unprotect(&corrupt).is_err());
    assert!(straja::vss::validate_sources(&[]).is_err());
    Ok(())
}
