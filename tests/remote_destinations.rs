use anyhow::{Result, ensure};
use std::{path::PathBuf, process::Stdio, time::Duration};
use straja::{
    engine::Engine,
    model::*,
    remote::{Credentials, Destination, Provider},
    store::Store,
};

fn credentials() -> Credentials {
    Credentials {
        password: "ftp-test-canary-secret".into(),
        access_key: String::new(),
        secret_key: String::new(),
        token: String::new(),
    }
}
fn target(port: u16) -> Destination {
    Destination {
        provider: Provider::Ftp,
        host: "127.0.0.1".into(),
        port,
        user: "straja-test".into(),
        ftp_tls: false,
        ..Default::default()
    }
}
fn draft(source: PathBuf, remote: Destination) -> NewPlan {
    NewPlan {
        remote: Some(remote),
        remote_credentials: Some(credentials()),
        mode: BackupMode::Encrypted,
        name: "Remote test".into(),
        sources: vec![source],
        repository: PathBuf::new(),
        excluded_paths: vec![],
        excludes: vec![],
        keep_last: Some(1),
        use_vss: false,
        schedule: Schedule::Manual,
        password: "repository-test-password".into(),
    }
}

#[test]
fn credentials_are_protected_and_edits_preserve_remote_identity() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    std::fs::create_dir(&source)?;
    let store = Store::open(&temp.path().join("state"))?;
    let plan = store.add(draft(source, target(2121)))?;
    assert_eq!(
        store.remote_secret(&plan.id)?.password,
        "ftp-test-canary-secret"
    );
    assert!(!serde_json::to_string(&store.plans()?)?.contains("canary-secret"));
    let db = rusqlite::Connection::open(store.root.join("straja.sqlite"))?;
    let encrypted: Vec<u8> =
        db.query_row("SELECT secret FROM remote_credentials", [], |r| r.get(0))?;
    assert!(!String::from_utf8_lossy(&encrypted).contains("canary-secret"));
    let mut changes = PlanChanges {
        remote: plan.remote.clone(),
        remote_credentials: None,
        name: "Edited".into(),
        sources: plan.sources.clone(),
        repository: PathBuf::new(),
        excluded_paths: vec![],
        excludes: vec![],
        keep_last: Some(2),
        use_vss: false,
        schedule: Schedule::Manual,
    };
    changes.remote.as_mut().unwrap().bandwidth_kib = 512;
    let updated = store.update(&plan.id, changes.clone())?;
    assert_eq!(updated.remote.unwrap().bandwidth_kib, 512);
    assert_eq!(
        store.remote_secret(&plan.id)?.password,
        "ftp-test-canary-secret"
    );
    changes.remote.as_mut().unwrap().host = "different.example".into();
    assert!(store.update(&plan.id, changes).is_err());
    store.delete(&plan.id)?;
    assert!(store.remote_secret(&plan.id).is_err());
    Ok(())
}

#[tokio::test]
#[ignore = "Uses bundled Restic and Rclone with an isolated loopback FTP server"]
async fn ftp_backup_restore_retention_and_failure_preserve_other_files() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(180), async {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".tools");
        let temp = tempfile::tempdir()?;
        let server_root = temp.path().join("server");
        std::fs::create_dir(&server_root)?;
        std::fs::write(server_root.join("unrelated.txt"), b"untouched")?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let mut command =
            tokio::process::Command::new(tools.join(straja::platform::executable("rclone")));
        straja::resources::restic_budget(&mut command);
        command
            .args(["serve", "ftp"])
            .arg(&server_root)
            .args([
                "--addr",
                &format!("127.0.0.1:{port}"),
                "--user",
                "straja-test",
                "--pass",
                "ftp-test-canary-secret",
                "--dir-cache-time",
                "0s",
                "--config",
                "NUL",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut server = command.spawn()?;
        let mut ready = false;
        for _ in 0..50 {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        ensure!(ready, "FTP test server not ready");
        let source = temp.path().join("source");
        std::fs::create_dir(&source)?;
        std::fs::write(source.join("data.txt"), b"first version")?;
        let store = Store::open(&temp.path().join("state"))?;
        let plan = store.add(draft(source.clone(), target(port)))?;
        // Exercise Restic's CSV/command quoting for installations under paths with spaces.
        let binaries = temp.path().join("tools with spaces");
        std::fs::create_dir(&binaries)?;
        for name in [
            straja::platform::executable("restic"),
            straja::platform::executable("rclone"),
        ] {
            std::fs::hard_link(tools.join(name), binaries.join(name))?;
        }
        let engine = Engine {
            binary: binaries.join(straja::platform::executable("restic")),
            store: store.clone(),
            live: Default::default(),
        };
        assert_eq!(engine.backup(&plan).await?.0, "Reușit");
        store.finish_backup(&plan.id, true)?;
        std::fs::write(source.join("data.txt"), b"second version")?;
        let plan = store.plan(&plan.id)?;
        assert_eq!(engine.backup(&plan).await?.0, "Reușit");
        let snapshots = engine.snapshots(&plan).await?;
        assert_eq!(snapshots.len(), 1);
        engine.check(&plan).await?;
        let restore = temp.path().join("restore");
        std::fs::create_dir(&restore)?;
        engine.restore(&plan, &snapshots[0].id, &restore).await?;
        let restored_root = std::fs::read_dir(&restore)?.next().unwrap()?.path();
        assert_eq!(
            std::fs::read(restored_root.join("source/data.txt"))?,
            b"second version"
        );
        assert_eq!(
            std::fs::read(server_root.join("unrelated.txt"))?,
            b"untouched"
        );
        let mut wrong = plan.clone();
        wrong.remote.as_mut().unwrap().port = 1;
        assert!(engine.backup(&wrong).await.is_err());
        assert_eq!(engine.snapshots(&plan).await?.len(), 1);
        let unsafe_plan = store.add(draft(source, target(port)))?;
        let unsafe_root = server_root
            .join("StrajaBackup")
            .join(format!("plan-{}", unsafe_plan.id));
        std::fs::create_dir_all(&unsafe_root)?;
        std::fs::write(unsafe_root.join("unrelated.txt"), b"keep")?;
        assert!(engine.backup(&unsafe_plan).await.is_err());
        assert_eq!(std::fs::read(unsafe_root.join("unrelated.txt"))?, b"keep");
        assert!(!unsafe_root.join("config").exists());
        store.delete(&plan.id)?;
        assert!(
            server_root
                .join("StrajaBackup")
                .join(format!("plan-{}", plan.id))
                .join("config")
                .is_file()
        );
        server.kill().await?;
        server.wait().await?;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

#[tokio::test]
#[ignore = "Uses bundled Restic/Rclone with an isolated loopback S3-compatible server"]
async fn s3_transport_roundtrip_with_isolated_server() -> Result<()> {
    use tokio::io::AsyncWriteExt;
    tokio::time::timeout(Duration::from_secs(120), async {
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".tools");
        let temp = tempfile::tempdir()?;
        let server_root = temp.path().join("server");
        std::fs::create_dir_all(server_root.join("test-bucket"))?;
        let cert = temp.path().join("cert.pem");
        let key = temp.path().join("key.pem");
        let openssl = std::env::var_os("STRAJA_TEST_OPENSSL")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:/Program Files/Git/usr/bin/openssl.exe"));
        let mut certificate = tokio::process::Command::new(openssl);
        straja::resources::restic_budget(&mut certificate);
        let generated = certificate
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
                "-addext",
                "subjectAltName=IP:127.0.0.1",
                "-keyout",
            ])
            .arg(&key)
            .arg("-out")
            .arg(&cert)
            .output()
            .await?;
        ensure!(
            generated.status.success(),
            "Test TLS certificate generation failed"
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let mut command =
            tokio::process::Command::new(tools.join(straja::platform::executable("rclone")));
        straja::resources::restic_budget(&mut command);
        let mut server = command
            .args(["serve", "s3"])
            .arg(&server_root)
            .args([
                "--addr",
                &format!("127.0.0.1:{port}"),
                "--auth-key",
                "test-access,test-secret",
                "--config",
                "NUL",
            ])
            .arg("--cert")
            .arg(&cert)
            .arg("--key")
            .arg(&key)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut ready = false;
        for _ in 0..50 {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        ensure!(ready, "S3 test server not ready");
        let source = temp.path().join("data.txt");
        std::fs::write(&source, b"S3 restored content")?;
        let restored = temp.path().join("restored");
        let d = Destination {
            provider: Provider::S3,
            bucket: "test-bucket".into(),
            ..Default::default()
        };
        let c = Credentials {
            password: String::new(),
            access_key: "test-access".into(),
            secret_key: "test-secret".into(),
            token: String::new(),
        };
        let id = uuid::Uuid::new_v4().to_string();
        for args in [
            vec!["init".into()],
            vec!["backup".into(), source.to_string_lossy().into_owned()],
            vec!["check".into()],
            vec![
                "restore".into(),
                straja::engine::restore_selector(
                    "latest",
                    &[source.to_string_lossy().into_owned()],
                ),
                "--target".into(),
                restored.to_string_lossy().into_owned(),
            ],
        ] {
            let mut cmd =
                tokio::process::Command::new(tools.join(straja::platform::executable("restic")));
            straja::remote::configure(
                &mut cmd,
                &tools.join(straja::platform::executable("rclone")),
                &d,
                &c,
            )
            .await?;
            // Test-only endpoint override. Production config always uses Amazon's HTTPS endpoints.
            cmd.env(
                "RCLONE_CONFIG_STRAJA_ENDPOINT",
                format!("https://127.0.0.1:{port}"),
            )
            .env("RCLONE_CONFIG_STRAJA_FORCE_PATH_STYLE", "true")
            .env("RCLONE_CA_CERT", &cert);
            cmd.env("RCLONE_TIMEOUT", "5s")
                .env("RCLONE_RETRIES", "1")
                .env("RCLONE_LOW_LEVEL_RETRIES", "1");
            eprintln!("S3 transport stage: {}", args[0]);
            let program = format!(
                "rclone.program=\"{}\"",
                tools
                    .join(straja::platform::executable("rclone"))
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            cmd.args([
                "-o",
                &format!("\"{}\"", program.replace('"', "\"\"")),
                "--repo",
                &format!("rclone:{}", d.root(&id)?),
            ])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
            let mut child = cmd.spawn()?;
            let mut input = child.stdin.take().unwrap();
            input.write_all(b"isolated-repository-secret\n").await?;
            drop(input);
            let result = child.wait_with_output().await?;
            ensure!(
                result.status.success(),
                "Isolated S3 transport failed: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        fn find(path: &std::path::Path) -> Result<bool> {
            for item in std::fs::read_dir(path)? {
                let path = item?.path();
                if path.is_dir() {
                    if find(&path)? {
                        return Ok(true);
                    }
                } else if path.file_name().is_some_and(|n| n == "data.txt") {
                    return Ok(std::fs::read(path)? == b"S3 restored content");
                }
            }
            Ok(false)
        }
        assert!(find(&restored)?);
        server.kill().await?;
        server.wait().await?;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}
