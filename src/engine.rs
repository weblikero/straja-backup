use crate::{model::*, store::Store};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

pub type LiveProgress = Arc<Mutex<Option<Progress>>>;

#[derive(Clone)]
pub struct Engine {
    pub binary: PathBuf,
    pub store: Store,
    pub live: LiveProgress,
}

impl Engine {
    pub fn bundled() -> Result<PathBuf> {
        Ok(std::env::current_exe()?
            .parent()
            .context("Folder executabil inexistent")?
            .join("restic.exe"))
    }
    pub fn update(&self, detail: &str) {
        if let Some(p) = self.live.lock().unwrap().as_mut() {
            p.detail = detail.into();
        }
    }
    async fn command(
        &self,
        plan: &Plan,
        args: &[String],
        track: bool,
    ) -> Result<(i32, String, String)> {
        if !self.binary.is_file() {
            bail!("restic.exe lipsește. Rulează scripts\\package.ps1 pentru a pregăti aplicația.");
        }
        let secret = self.store.secret(&plan.id)?;
        let mut cmd = Command::new(&self.binary);
        // stdin avoids exposing the password in command lines, environment, or plaintext files.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("RESTIC_") {
                cmd.env_remove(key);
            }
        }
        cmd.args(["--repo"])
            .arg(&plan.repository)
            .arg("--json")
            .arg("--cache-dir")
            .arg(self.store.root.join("cache"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        crate::resources::restic_budget(&mut cmd);
        let mut child = cmd.spawn().context("Restic nu a putut porni")?;
        let mut input = child.stdin.take().context("Lipsește stdin")?;
        input.write_all(&secret).await?;
        input.write_all(b"\n").await?;
        drop(input);
        let stderr = child.stderr.take().context("Lipsește stderr")?;
        // Drain concurrently: a full stderr pipe must never deadlock a running backup.
        let errors = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut buffer = [0u8; 4096];
            let mut tail = Vec::new();
            loop {
                let n = reader.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }
                tail.extend_from_slice(&buffer[..n]);
                if tail.len() > 32768 {
                    tail.drain(..tail.len() - 32768);
                }
            }
            Ok::<_, std::io::Error>(String::from_utf8_lossy(&tail).into_owned())
        });
        let mut lines = BufReader::new(child.stdout.take().context("Lipsește stdout")?).lines();
        let mut output = String::new();
        while let Some(line) = lines.next_line().await? {
            if track {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    self.parse_progress(&value);
                }
                // Retain only the final messages, not every per-file progress event.
                if output.len() > 32768 {
                    output.clear();
                }
            } else if output.len() > 16 * 1024 * 1024 {
                bail!("Răspunsul Restic depășește limita de 16 MB.");
            }
            output.push_str(&line);
            output.push('\n');
        }
        let status = child.wait().await?;
        let stderr = errors.await??;
        Ok((status.code().unwrap_or(-1), output, stderr))
    }
    fn parse_progress(&self, v: &Value) {
        if let Some(p) = self.live.lock().unwrap().as_mut() {
            if let Some(n) = v["percent_done"].as_f64() {
                p.percent = (n as f32).clamp(0., 1.);
            }
            if let Some(n) = v["files_done"].as_u64() {
                p.files_done = n;
            }
            if let Some(n) = v["bytes_done"].as_u64() {
                p.bytes_done = n;
            }
            if let Some(path) = v["current_files"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
            {
                p.detail = path.into();
            }
        }
    }
    fn checked(code: i32, output: String, error: String) -> Result<String> {
        if code != 0 {
            bail!(
                "Restic (cod {code}): {}",
                if error.trim().is_empty() {
                    output.trim()
                } else {
                    error.trim()
                }
            );
        }
        Ok(output)
    }
    async fn ensure_repo(&self, plan: &Plan) -> Result<()> {
        if !plan.repository.is_dir() {
            bail!("Destinația nu este disponibilă. Conectează discul sau verifică rețeaua.");
        }
        if plan.repository.join("config").exists() {
            return Ok(());
        }
        // Never initialize a nonempty folder or recreate an established but damaged repository.
        if plan.last_success.is_some() {
            bail!("Destinația nu este un depozit Restic valid și gol. Nu a fost modificată.");
        }
        validate_empty_repository(&plan.repository)?;
        self.update("Pregătesc destinația criptată…");
        let (c, o, e) = self.command(plan, &["init".into()], false).await?;
        Self::checked(c, o, e)?;
        Ok(())
    }
    pub async fn backup(&self, plan: &Plan) -> Result<(String, String)> {
        crate::exclusions::validate_paths(&plan.sources, &plan.excluded_paths)?;
        validate_retention(plan.keep_last)?;
        let initialized = plan.repository.join("config").exists()
            || plan.repository.join(crate::plain::MARKER).exists();
        let before = if plan.keep_last.is_some() && initialized {
            self.snapshots(plan).await.map(|copies| {
                copies
                    .into_iter()
                    .map(|s| s.id)
                    .collect::<std::collections::HashSet<_>>()
            })
        } else {
            Ok(Default::default())
        };
        let (mut outcome, mut detail) = self.backup_inner(plan).await?;
        if outcome == "Reușit"
            && let Some(keep) = plan.keep_last
        {
            match match before {
                Ok(before) => self.retain(plan, keep, &before).await,
                Err(e) => Err(e),
            } {
                Ok(count) => detail.push_str(&format!(
                    " Retenție: {count} copii vechi eliminate; {}.",
                    retention_label(Some(keep))
                )),
                Err(e) => {
                    outcome = "Reușit cu avertisment".into();
                    detail.push_str(&format!(" ATENȚIE: backupul nou este salvat, dar curățarea copiilor vechi nu s-a încheiat: {e:#}"));
                }
            }
        }
        Ok((outcome, detail))
    }
    async fn retain(
        &self,
        plan: &Plan,
        keep: u32,
        before: &std::collections::HashSet<String>,
    ) -> Result<usize> {
        let snapshots = self.snapshots(plan).await?;
        let added: Vec<_> = snapshots
            .iter()
            .filter(|s| !before.contains(&s.id))
            .collect();
        anyhow::ensure!(
            added.len() == 1,
            "Copia nouă nu poate fi identificată fără ambiguitate; nu șterg nimic."
        );
        // Always retain the newly completed copy, even if the system clock moved backwards.
        let old: Vec<_> = snapshots
            .iter()
            .filter(|s| s.id != added[0].id)
            .skip(keep as usize - 1)
            .collect();
        if old.is_empty() {
            return Ok(0);
        }
        self.update("Verific copiile înainte de eliminarea versiunilor vechi…");
        self.check(plan).await?;
        self.update("Elimin copiile vechi conform limitei planului…");
        if plan.mode != BackupMode::Encrypted {
            for snapshot in &old {
                crate::plain::validate_removal(plan, &snapshot.id).await?;
            }
            for snapshot in &old {
                crate::plain::remove_snapshot(plan, &snapshot.id).await?;
            }
        } else {
            // Explicit full IDs, filtered by this plan's tag; no implicit host/path grouping.
            // Batch to stay below Windows' command-line length limit.
            for chunk in old.chunks(100) {
                let mut args = vec!["forget".into()];
                args.extend(chunk.iter().map(|s| s.id.clone()));
                let (c, o, e) = self.command(plan, &args, true).await?;
                Self::checked(c, o, e)?;
            }
            let (c, o, e) = self.command(plan, &["prune".into()], true).await?;
            Self::checked(c, o, e)?;
            let (c, o, e) = self.command(plan, &["check".into()], true).await?;
            Self::checked(c, o, e)?;
        }
        Ok(old.len())
    }
    async fn backup_inner(&self, plan: &Plan) -> Result<(String, String)> {
        if plan.mode != BackupMode::Encrypted {
            return crate::plain::backup(self, plan).await;
        }
        // Revalidate paths on each run: removable disks and junction targets can change.
        let repo =
            std::fs::canonicalize(&plan.repository).context("Destinația nu este disponibilă")?;
        for path in &plan.sources {
            let source = std::fs::canonicalize(path)
                .with_context(|| format!("Sursa nu este disponibilă: {}", path.display()))?;
            if overlaps(&repo, &source) {
                bail!("Sursa și destinația se suprapun.");
            }
        }
        self.ensure_repo(plan).await?;
        self.update("Citesc fișierele…");
        let mut args = vec![
            "backup".into(),
            "--tag".into(),
            format!("straja:{}", plan.id),
        ];
        for pattern in &plan.excludes {
            args.extend(["--exclude".into(), pattern.clone()]);
        }
        for path in &plan.excluded_paths {
            args.extend(["--iexclude".into(), crate::exclusions::restic_pattern(path)]);
        }
        // Prevent recursion into live application secrets/database if a parent folder is selected.
        args.extend([
            "--exclude".into(),
            self.store.root.to_string_lossy().into_owned(),
        ]);
        args.push("--".into());
        args.extend(
            plan.sources
                .iter()
                .map(|p| p.to_string_lossy().into_owned()),
        );
        let (code, output, error) = self.command(plan, &args, true).await?;
        if code == 3 {
            return Ok((
                "Incomplet".into(),
                format!("Unele fișiere nu au putut fi citite. Copia nu este completă. {error}"),
            ));
        }
        Self::checked(code, output, error)?;
        Ok((
            "Reușit".into(),
            "Backup finalizat. Copia este disponibilă în Restaurare.".into(),
        ))
    }
    pub async fn snapshots(&self, plan: &Plan) -> Result<Vec<Snapshot>> {
        if plan.mode != BackupMode::Encrypted {
            return crate::plain::snapshots(plan).await;
        }
        let args = [
            "snapshots".into(),
            "--tag".into(),
            format!("straja:{}", plan.id),
        ];
        let (c, o, e) = self.command(plan, &args, false).await?;
        let mut snapshots: Vec<Snapshot> = serde_json::from_str(&Self::checked(c, o, e)?)?;
        snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.time));
        Ok(snapshots)
    }
    pub async fn restore(
        &self,
        plan: &Plan,
        snapshot: &str,
        parent: &Path,
    ) -> Result<(String, String)> {
        if plan.mode != BackupMode::Encrypted {
            return crate::plain::restore(self, plan, snapshot, parent).await;
        }
        // Explicit snapshot IDs only; validate membership before touching the destination.
        let selected = self
            .snapshots(plan)
            .await?
            .into_iter()
            .find(|s| s.id == snapshot)
            .context("Copia selectată nu aparține acestui plan.")?;
        let parent =
            std::fs::canonicalize(parent).context("Alege un folder existent pentru restaurare")?;
        if !parent.is_dir() {
            bail!("Destinația restaurării trebuie să fie un folder.");
        }
        let repo = std::fs::canonicalize(&plan.repository)?;
        if overlaps(&parent, &repo) || overlaps(&parent, &std::fs::canonicalize(&self.store.root)?)
        {
            bail!("Restaurarea trebuie făcută separat de depozit și de datele aplicației.");
        }
        let target = parent.join(format!("Straja-Restaurare-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&target)?;
        self.update(&format!("Restaurare în {}", target.display()));
        // Restore below the sources' common parent. Recreating artificial C/Users
        // ancestors also restores their system ACLs and fails for ordinary Windows users.
        let selector = restore_selector(snapshot, &selected.paths);
        let args = [
            "restore".into(),
            selector,
            "--target".into(),
            target.to_string_lossy().into_owned(),
            "--verify".into(),
        ];
        let (c, o, e) = self.command(plan, &args, true).await?;
        Self::checked(c, o, e).with_context(|| {
            format!(
                "Restaurare nereușită; fișierele parțiale sunt în {}",
                target.display()
            )
        })?;
        Ok((
            "Reușit".into(),
            format!("Fișiere restaurate și verificate în {}", target.display()),
        ))
    }
    pub async fn check(&self, plan: &Plan) -> Result<(String, String)> {
        if plan.mode != BackupMode::Encrypted {
            return crate::plain::check(self, plan).await;
        }
        self.update("Verific integritatea tuturor datelor din depozit…");
        let (c, o, e) = self
            .command(plan, &["check".into(), "--read-data".into()], true)
            .await?;
        Self::checked(c, o, e)?;
        Ok((
            "Reușit".into(),
            "Verificarea integrală a depozitului s-a încheiat fără erori.".into(),
        ))
    }
}

pub fn restore_selector(snapshot: &str, paths: &[String]) -> String {
    let Some(mut common) = paths
        .first()
        .and_then(|p| Path::new(p).parent())
        .map(Path::to_path_buf)
    else {
        return snapshot.into();
    };
    for source in paths.iter().skip(1) {
        let Some(parent) = Path::new(source).parent() else {
            return snapshot.into();
        };
        while !parent.starts_with(&common) {
            if !common.pop() {
                return snapshot.into();
            }
        }
    }
    let normalized = common.to_string_lossy().replace('\\', "/");
    let normalized = normalized.strip_prefix("//?/").unwrap_or(&normalized);
    if normalized.as_bytes().get(1) == Some(&b':') {
        let tree_path = normalized.replace(':', "");
        format!("{snapshot}:/{}", tree_path.trim_matches('/'))
    } else {
        snapshot.into()
    }
}
