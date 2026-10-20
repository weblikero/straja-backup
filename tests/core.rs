use chrono::{TimeZone, Utc};
use straja::{model::*, security, store::Store};

#[test]
fn deleting_plan_preserves_files_history_other_plans_and_handles_offline_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(&tmp.path().join("app")).unwrap();
    let source = tmp.path().join("source");
    let repo = tmp.path().join("repo");
    let other_repo = tmp.path().join("other");
    for path in [&source, &repo, &other_repo] {
        std::fs::create_dir(path).unwrap();
    }
    std::fs::write(source.join("original.txt"), "source data").unwrap();
    let draft = NewPlan {
        remote: None,
        remote_credentials: None,
        mode: BackupMode::Encrypted,
        name: "Delete me".into(),
        sources: vec![source.clone()],
        repository: repo.clone(),
        excluded_paths: vec![],
        excludes: vec![],
        keep_last: None,
        use_vss: false,
        schedule: Schedule::Interval { minutes: 60 },
        password: "test-secret-password".into(),
    };
    let plan = store.add(draft.clone()).unwrap();
    let mut second = draft;
    second.repository = other_repo;
    let other = store.add(second).unwrap();
    std::fs::write(repo.join("backup-canary.txt"), "keep backup").unwrap();
    let mut run = Run {
        id: "run-to-retain".into(),
        plan_id: plan.id.clone(),
        plan_name: plan.name.clone(),
        operation: "Backup".into(),
        started: Utc::now(),
        finished: None,
        outcome: "În curs".into(),
        detail: String::new(),
    };
    store.put_run(&run).unwrap();
    assert!(store.delete(&plan.id).is_err());
    assert_eq!(&*store.secret(&plan.id).unwrap(), b"test-secret-password");
    // An old interrupted run must not permanently block deletion after restart.
    for index in 0..101 {
        let mut newer = run.clone();
        newer.id = format!("recent-{index}");
        newer.plan_id = other.id.clone();
        newer.started += chrono::Duration::seconds(index + 1);
        newer.finished = Some(newer.started);
        newer.outcome = "Reușit".into();
        store.put_run(&newer).unwrap();
    }
    assert!(!store.history().unwrap().iter().any(|r| r.id == run.id));
    store.recover_interrupted().unwrap();
    let recovered: String = rusqlite::Connection::open(store.root.join("straja.sqlite"))
        .unwrap()
        .query_row("SELECT json FROM runs WHERE id=?1", [&run.id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Run>(&recovered).unwrap().outcome,
        "Întrerupt"
    );
    run.finished = Some(Utc::now());
    run.outcome = "Reușit".into();
    store.put_run(&run).unwrap();
    let offline_source = tmp.path().join("disconnected-source");
    let offline_repo = tmp.path().join("disconnected-repo");
    std::fs::rename(&source, &offline_source).unwrap();
    std::fs::rename(&repo, &offline_repo).unwrap();
    assert_eq!(store.delete(&plan.id).unwrap().id, plan.id);
    let reopened = Store::open(&store.root).unwrap();
    assert_eq!(reopened.plans().unwrap().len(), 1);
    assert!(reopened.plan(&plan.id).is_err());
    assert!(reopened.secret(&plan.id).is_err());
    assert_eq!(
        &*reopened.secret(&other.id).unwrap(),
        b"test-secret-password"
    );
    let retained: String = rusqlite::Connection::open(reopened.root.join("straja.sqlite"))
        .unwrap()
        .query_row("SELECT json FROM runs WHERE id=?1", [&run.id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Run>(&retained).unwrap().plan_id,
        plan.id
    );
    reopened.recover_interrupted().unwrap();
    assert!(reopened.delete(&plan.id).is_err());
    assert_eq!(
        std::fs::read_to_string(offline_source.join("original.txt")).unwrap(),
        "source data"
    );
    assert_eq!(
        std::fs::read_to_string(offline_repo.join("backup-canary.txt")).unwrap(),
        "keep backup"
    );
}

#[test]
fn schedules_are_strictly_in_the_future() {
    let now = Utc.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
    assert_eq!(Schedule::Manual.next_in(now, &Utc), None);
    assert_eq!(
        Schedule::Interval { minutes: 60 }
            .next_in(now, &Utc)
            .unwrap(),
        now + chrono::Duration::hours(1)
    );
    assert_eq!(
        Schedule::Daily {
            hour: 12,
            minute: 0
        }
        .next_in(now, &Utc)
        .unwrap(),
        now + chrono::Duration::days(1)
    );
    assert_eq!(
        Schedule::Weekly {
            weekday: 0,
            hour: 12,
            minute: 0
        }
        .next_in(now, &Utc)
        .unwrap(),
        now + chrono::Duration::days(1)
    );
    assert!(
        Schedule::Daily {
            hour: 25,
            minute: 0
        }
        .validate()
        .is_err()
    );
    assert!(Schedule::Interval { minutes: 0 }.validate().is_err());
}

#[test]
fn dst_skips_nonexistent_time_and_avoids_duplicate_fall_back_run() {
    let tz = chrono_tz::Europe::Bucharest;
    let daily = Schedule::Daily {
        hour: 3,
        minute: 30,
    };
    let before_spring = Utc.with_ymd_and_hms(2026, 3, 28, 23, 0, 0).unwrap();
    let next = daily.next_in(before_spring, &tz).unwrap();
    assert_eq!(next, Utc.with_ymd_and_hms(2026, 3, 30, 0, 30, 0).unwrap());
    let between_fall_occurrences = Utc.with_ymd_and_hms(2026, 10, 25, 0, 45, 0).unwrap();
    assert_eq!(
        daily.next_in(between_fall_occurrences, &tz).unwrap(),
        Utc.with_ymd_and_hms(2026, 10, 26, 1, 30, 0).unwrap()
    );
}

#[cfg(windows)]
#[test]
fn path_boundaries_do_not_confuse_similar_folder_names() {
    use std::path::Path;
    assert!(overlaps(
        Path::new(r"C:\Data"),
        Path::new(r"c:\data\backup")
    ));
    assert!(!overlaps(Path::new(r"C:\Data"), Path::new(r"C:\Database")));
}

#[test]
fn secrets_and_schedules_survive_database_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("app");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("repository");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&dest).unwrap();
    let store = Store::open(&root).unwrap();
    let draft = NewPlan {
        remote: None,
        remote_credentials: None,
        mode: BackupMode::Encrypted,
        name: "Documente".into(),
        sources: vec![source.clone()],
        repository: dest.clone(),
        excluded_paths: vec![],
        excludes: vec![],
        keep_last: None,
        use_vss: false,
        schedule: Schedule::Interval { minutes: 60 },
        password: "test-only-password".into(),
    };
    let p = store.add(draft.clone()).unwrap();
    assert!(store.add(draft.clone()).is_err());
    assert_eq!(&*store.secret(&p.id).unwrap(), b"test-only-password");
    let reopened = Store::open(&root).unwrap();
    assert_eq!(reopened.plan(&p.id).unwrap().next_due, p.next_due);
    reopened.finish_backup(&p.id, false).unwrap();
    let failed = reopened.plan(&p.id).unwrap();
    assert!(failed.last_success.is_none());
    assert!((failed.next_due.unwrap() - Utc::now()).num_minutes() <= 15);
    reopened.finish_backup(&p.id, true).unwrap();
    assert!(reopened.plan(&p.id).unwrap().last_success.is_some());
    reopened.toggle(&p.id).unwrap();
    assert!(!reopened.plan(&p.id).unwrap().enabled);
    let nested = source.join("backups");
    std::fs::create_dir(&nested).unwrap();
    let mut bad = draft;
    bad.repository = nested;
    assert!(bad.validate(&root).is_err());
    let encrypted = security::protect(b"not plaintext").unwrap();
    assert_ne!(encrypted, b"not plaintext");
    assert_eq!(security::unprotect(&encrypted).unwrap(), b"not plaintext");
}

#[tokio::test]
async fn ipc_rejects_oversized_messages() {
    use tokio::io::AsyncWriteExt;
    let (mut writer, mut reader) = tokio::io::duplex(64);
    writer.write_u32_le(17 * 1024 * 1024).await.unwrap();
    assert!(
        straja::protocol::read::<straja::protocol::Request>(&mut reader)
            .await
            .is_err()
    );
}

#[test]
fn edits_preserve_secrets_identity_and_history_and_validate_destination() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("app");
    let source = tmp.path().join("source");
    let repo = tmp.path().join("repo");
    let replacement = tmp.path().join("replacement");
    for p in [&source, &repo, &replacement] {
        std::fs::create_dir(p).unwrap();
    }
    let store = Store::open(&root).unwrap();
    let draft = NewPlan {
        remote: None,
        remote_credentials: None,
        mode: BackupMode::Encrypted,
        name: "Initial".into(),
        sources: vec![source.clone()],
        repository: repo.clone(),
        excluded_paths: vec![],
        excludes: vec![],
        keep_last: None,
        use_vss: false,
        schedule: Schedule::Manual,
        password: "test-password".into(),
    };
    std::fs::write(repo.join("existing.txt"), "keep").unwrap();
    assert!(
        store
            .add(draft.clone())
            .unwrap_err()
            .to_string()
            .contains("nu este gol")
    );
    std::fs::remove_file(repo.join("existing.txt")).unwrap();
    let plan = store.add(draft).unwrap();
    let encrypted_before: Vec<u8> = rusqlite::Connection::open(root.join("straja.sqlite"))
        .unwrap()
        .query_row("SELECT secret FROM plans WHERE id=?1", [&plan.id], |row| {
            row.get(0)
        })
        .unwrap();
    store.toggle(&plan.id).unwrap();
    let changes = PlanChanges {
        remote: None,
        remote_credentials: None,
        name: "Edited".into(),
        sources: vec![source],
        repository: replacement.clone(),
        excluded_paths: vec![],
        excludes: vec!["*.tmp".into()],
        keep_last: None,
        use_vss: false,
        schedule: Schedule::Interval { minutes: 90 },
    };
    let edited = store.update(&plan.id, changes.clone()).unwrap();
    assert_eq!(edited.id, plan.id);
    assert!(!edited.enabled);
    assert_eq!(edited.excludes, vec!["*.tmp"]);
    assert_eq!(edited.repository, replacement);
    assert!(edited.next_due.unwrap() > Utc::now());
    assert_eq!(&*store.secret(&plan.id).unwrap(), b"test-password");
    let encrypted_after: Vec<u8> = rusqlite::Connection::open(root.join("straja.sqlite"))
        .unwrap()
        .query_row("SELECT secret FROM plans WHERE id=?1", [&plan.id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(encrypted_before, encrypted_after);
    // Once initialized, destination changes must not orphan the existing snapshots.
    std::fs::write(replacement.join("config"), "initialized repository marker").unwrap();
    store.finish_backup(&plan.id, true).unwrap();
    let success = store.plan(&plan.id).unwrap().last_success;
    let mut rename = changes.clone();
    rename.name = "Renamed".into();
    assert_eq!(
        store.update(&plan.id, rename).unwrap().last_success,
        success
    );
    let mut move_repo = changes;
    move_repo.repository = repo;
    assert!(store.update(&plan.id, move_repo).is_err());
    let saved = Store::open(&root).unwrap().plan(&plan.id).unwrap();
    assert_eq!(saved.name, "Renamed");
    assert_eq!(saved.repository, replacement);
}
