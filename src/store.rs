use crate::{model::*, security};
use anyhow::{Context, Result, bail};
use chrono::{Duration, Utc};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
}

fn protect_remote(credentials: &crate::remote::Credentials) -> Result<Vec<u8>> {
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(credentials)?);
    security::protect(&bytes)
}

impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        #[cfg(windows)]
        std::fs::create_dir_all(root)?;
        #[cfg(unix)]
        crate::unix_security::private_directory(root)?;
        let store = Self {
            root: root.to_path_buf(),
        };
        store.connect()?.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS plans(id TEXT PRIMARY KEY, json TEXT NOT NULL, secret BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS remote_credentials(id TEXT PRIMARY KEY, secret BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY, started TEXT NOT NULL, json TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS runs_started ON runs(started DESC);
            CREATE INDEX IF NOT EXISTS runs_finished_started ON runs(started DESC) WHERE json_extract(json, '$.finished') IS NOT NULL;
            PRAGMA user_version=1;")?;
        Ok(store)
    }
    fn connect(&self) -> Result<Connection> {
        let db = Connection::open(self.root.join("straja.sqlite"))?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(db)
    }
    pub fn plans(&self) -> Result<Vec<Plan>> {
        let db = self.connect()?;
        let mut query = db.prepare("SELECT json FROM plans ORDER BY rowid")?;
        query
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect()
    }
    pub fn plan(&self, id: &str) -> Result<Plan> {
        self.plans()?
            .into_iter()
            .find(|p| p.id == id)
            .context("Planul nu există.")
    }
    pub fn add(&self, mut draft: NewPlan) -> Result<Plan> {
        use zeroize::Zeroize;
        let validation = draft.validate(&self.root);
        if let Err(e) = validation {
            draft.password.zeroize();
            return Err(e);
        }
        let secret = if draft.mode == BackupMode::Encrypted {
            security::protect(draft.password.as_bytes())
        } else {
            Ok(Vec::new())
        };
        draft.password.zeroize();
        let secret = secret?;
        if draft.remote.is_none() {
            let repo = std::fs::canonicalize(&draft.repository)?;
            for existing in self.plans()?.into_iter().filter(|p| p.remote.is_none()) {
                // An unavailable destination still cannot be registered twice by its saved path.
                let other =
                    std::fs::canonicalize(&existing.repository).unwrap_or(existing.repository);
                if overlaps(&repo, &other) {
                    bail!(
                        "Acest folder este deja folosit de alt plan. Alege o destinație separată."
                    );
                }
            }
        }
        let remote_secret = draft
            .remote_credentials
            .as_ref()
            .map(protect_remote)
            .transpose()?;
        let plan = Plan {
            remote: draft.remote,
            mode: draft.mode,
            id: Uuid::new_v4().to_string(),
            name: draft.name.trim().into(),
            sources: draft.sources,
            repository: draft.repository,
            excluded_paths: draft.excluded_paths,
            excludes: draft.excludes,
            next_due: draft.schedule.next_after(Utc::now()),
            keep_last: draft.keep_last,
            use_vss: draft.use_vss,
            schedule: draft.schedule,
            enabled: true,
            last_success: None,
        };
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO plans VALUES (?1,?2,?3)",
            params![plan.id, serde_json::to_string(&plan)?, secret],
        )?;
        if let Some(secret) = remote_secret {
            tx.execute(
                "INSERT INTO remote_credentials VALUES (?1,?2)",
                params![plan.id, secret],
            )?;
        }
        tx.commit()?;
        Ok(plan)
    }
    pub fn update(&self, id: &str, changes: PlanChanges) -> Result<Plan> {
        changes.validate(&self.root)?;
        let mut plan = self.plan(id)?;
        if let Some(remote) = &changes.remote {
            ensure_remote_mode(plan.mode)?;
            anyhow::ensure!(
                plan.remote
                    .as_ref()
                    .is_some_and(|old| old.same_location(remote)),
                "Pentru altă destinație online creează un plan nou."
            );
            if let Some(credentials) = &changes.remote_credentials {
                credentials.validate(remote.provider)?;
            }
        } else {
            anyhow::ensure!(
                plan.remote.is_none(),
                "Pentru altă destinație online creează un plan nou."
            );
            let repo = std::fs::canonicalize(&changes.repository)?;
            let old_repo =
                std::fs::canonicalize(&plan.repository).unwrap_or_else(|_| plan.repository.clone());
            let changed_repo = repo != old_repo;
            let initialized = plan.repository.join("config").exists()
                || plan.repository.join(crate::plain::MARKER).exists();
            if plan.mode != BackupMode::Encrypted {
                crate::plain::exclusions(&changes.excludes)?;
            }
            if changed_repo && (plan.last_success.is_some() || initialized) {
                bail!(
                    "Acest plan are deja un depozit inițializat. Pentru altă destinație creează un plan nou; copiile existente rămân accesibile din acest plan."
                );
            }
            if changed_repo || !initialized {
                validate_empty_repository(&changes.repository)?;
            }
            for other in self
                .plans()?
                .into_iter()
                .filter(|p| p.id != id && p.remote.is_none())
            {
                let other_repo =
                    std::fs::canonicalize(&other.repository).unwrap_or(other.repository);
                if overlaps(&repo, &other_repo) {
                    bail!(
                        "Acest folder este deja folosit de alt plan. Alege o destinație separată."
                    );
                }
            }
        }
        if plan.schedule != changes.schedule {
            plan.next_due = changes.schedule.next_after(Utc::now());
        }
        plan.name = changes.name.trim().into();
        plan.sources = changes.sources;
        plan.repository = changes.repository;
        plan.excludes = changes.excludes;
        plan.excluded_paths = changes.excluded_paths;
        plan.schedule = changes.schedule;
        plan.keep_last = changes.keep_last;
        plan.use_vss = changes.use_vss;
        plan.remote = changes.remote;
        // Keep the same ID, encrypted password, history and enabled state.
        let secret = changes
            .remote_credentials
            .as_ref()
            .map(protect_remote)
            .transpose()?;
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        tx.execute(
            "UPDATE plans SET json=?2 WHERE id=?1",
            params![plan.id, serde_json::to_string(&plan)?],
        )?;
        if let Some(secret) = secret {
            tx.execute(
                "INSERT OR REPLACE INTO remote_credentials VALUES (?1,?2)",
                params![plan.id, secret],
            )?;
        }
        tx.commit()?;
        Ok(plan)
    }
    pub fn remote_secret(&self, id: &str) -> Result<crate::remote::Credentials> {
        let bytes: Vec<u8> = self
            .connect()?
            .query_row(
                "SELECT secret FROM remote_credentials WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .context("Conectează destinația online înainte de salvare.")?;
        let clear = zeroize::Zeroizing::new(security::unprotect(&bytes)?);
        Ok(serde_json::from_slice(&clear)?)
    }
    pub fn secret(&self, id: &str) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        let bytes: Vec<u8> =
            self.connect()?
                .query_row("SELECT secret FROM plans WHERE id=?1", [id], |r| r.get(0))?;
        Ok(zeroize::Zeroizing::new(security::unprotect(&bytes)?))
    }
    pub fn delete(&self, id: &str) -> Result<Plan> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let json: String = tx
            .query_row("SELECT json FROM plans WHERE id=?1", [id], |r| r.get(0))
            .context("Planul nu există.")?;
        let plan: Plan = serde_json::from_str(&json)?;
        let unfinished: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM runs WHERE json_extract(json, '$.plan_id')=?1 AND json_extract(json, '$.finished') IS NULL)",
            [id], |r| r.get(0),
        )?;
        if unfinished {
            bail!(
                "Planul are o operație nefinalizată. Așteaptă terminarea ei sau repornește agentul pentru recuperarea stării."
            );
        }
        // Only configuration and its saved credential are removed. History and disk contents remain.
        tx.execute("DELETE FROM plans WHERE id=?1", [id])?;
        tx.execute("DELETE FROM remote_credentials WHERE id=?1", [id])?;
        tx.commit()?;
        Ok(plan)
    }
    fn save(&self, plan: &Plan) -> Result<()> {
        self.connect()?.execute(
            "UPDATE plans SET json=?2 WHERE id=?1",
            params![plan.id, serde_json::to_string(plan)?],
        )?;
        Ok(())
    }
    pub fn toggle(&self, id: &str) -> Result<()> {
        let mut p = self.plan(id)?;
        p.enabled = !p.enabled;
        if p.enabled {
            p.next_due = p.schedule.next_after(Utc::now());
        }
        self.save(&p)
    }
    pub fn finish_backup(&self, id: &str, success: bool) -> Result<()> {
        let mut p = self.plan(id)?;
        let now = Utc::now();
        if success {
            p.last_success = Some(now);
        }
        p.next_due = if !success && p.schedule != Schedule::Manual {
            Some(now + Duration::minutes(15))
        } else {
            p.schedule.next_after(now)
        };
        self.save(&p)
    }
    pub fn put_run(&self, run: &Run) -> Result<()> {
        self.connect()?.execute(
            "INSERT INTO runs VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
            params![
                run.id,
                run.started.to_rfc3339(),
                serde_json::to_string(run)?
            ],
        )?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<Run>> {
        let db = self.connect()?;
        let mut q = db.prepare("SELECT json FROM runs ORDER BY started DESC LIMIT 100")?;
        q.query_map([], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect()
    }
    pub fn status_summary(&self) -> Result<(u64, Option<LastOutcome>)> {
        use rusqlite::OptionalExtension;
        let db = self.connect()?;
        let count: i64 = db.query_row("SELECT COUNT(*) FROM plans", [], |r| r.get(0))?;
        let last = db.query_row(
            "SELECT json_extract(json, '$.plan_name'), json_extract(json, '$.outcome') FROM runs WHERE json_extract(json, '$.finished') IS NOT NULL ORDER BY started DESC LIMIT 1",
            [], |r| Ok(LastOutcome { plan_name: r.get(0)?, outcome: r.get(1)? }),
        ).optional()?;
        Ok((count.try_into()?, last))
    }
    pub fn recover_interrupted(&self) -> Result<()> {
        // Recovery must include old unfinished runs, beyond the 100 rows displayed in the UI.
        let pending: Vec<Run> = {
            let db = self.connect()?;
            let mut query =
                db.prepare("SELECT json FROM runs WHERE json_extract(json, '$.finished') IS NULL")?;
            query
                .query_map([], |r| r.get::<_, String>(0))?
                .map(|r| Ok(serde_json::from_str(&r?)?))
                .collect::<Result<_>>()?
        };
        for mut run in pending {
            if run.finished.is_none() {
                run.finished = Some(Utc::now());
                run.outcome = "Întrerupt".into();
                run.detail = "Agentul s-a oprit înainte de finalizarea operației. Verifică destinația și reîncearcă.".into();
                self.put_run(&run)?;
                if run.operation == "Backup" {
                    self.finish_backup(&run.plan_id, false)?;
                }
            }
        }
        Ok(())
    }
}
