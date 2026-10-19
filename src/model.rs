use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum Schedule {
    #[default]
    Manual,
    Interval {
        minutes: u32,
    },
    Daily {
        hour: u32,
        minute: u32,
    },
    Weekly {
        weekday: u32,
        hour: u32,
        minute: u32,
    },
}

impl Schedule {
    pub fn validate(&self) -> Result<()> {
        match *self {
            Self::Interval { minutes } if !(15..=10080).contains(&minutes) => {
                bail!("Intervalul trebuie să fie între 15 minute și 7 zile.")
            }
            Self::Daily { hour, minute } | Self::Weekly { hour, minute, .. }
                if hour > 23 || minute > 59 =>
            {
                bail!("Ora nu este validă.")
            }
            Self::Weekly { weekday, .. } if weekday > 6 => bail!("Ziua săptămânii nu este validă."),
            _ => Ok(()),
        }
    }
    pub fn next_after(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.next_in(now, &Local)
    }
    pub fn next_in<Tz: TimeZone>(&self, now: DateTime<Utc>, tz: &Tz) -> Option<DateTime<Utc>> {
        if self.validate().is_err() {
            return None;
        }
        let (hour, minute, weekday) = match *self {
            Self::Manual => return None,
            Self::Interval { minutes } => return Some(now + Duration::minutes(minutes.into())),
            Self::Daily { hour, minute } => (hour, minute, None),
            Self::Weekly {
                weekday,
                hour,
                minute,
            } => (hour, minute, Some(weekday)),
        };
        let today = now.with_timezone(tz).date_naive();
        for offset in 0..=14 {
            let date = today + Duration::days(offset);
            if weekday.is_some_and(|d| date.weekday().num_days_from_monday() != d) {
                continue;
            }
            // Skip a nonexistent spring-forward time; choose the first fall-back occurrence.
            if let Some(candidate) = tz
                .from_local_datetime(&date.and_hms_opt(hour, minute, 0)?)
                .earliest()
            {
                let utc = candidate.with_timezone(&Utc);
                if utc > now {
                    return Some(utc);
                }
            }
        }
        None
    }
    pub fn label(&self) -> String {
        match *self {
            Self::Manual => "Doar manual".into(),
            Self::Interval { minutes } => format!("La fiecare {minutes} minute"),
            Self::Daily { hour, minute } => format!("Zilnic, {hour:02}:{minute:02}"),
            Self::Weekly {
                weekday,
                hour,
                minute,
            } => format!(
                "Săptămânal · {} · {hour:02}:{minute:02}",
                [
                    "Luni",
                    "Marți",
                    "Miercuri",
                    "Joi",
                    "Vineri",
                    "Sâmbătă",
                    "Duminică"
                ][weekday.min(6) as usize]
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default)]
    pub remote: Option<crate::remote::Destination>,
    #[serde(default)]
    pub mode: BackupMode,
    pub id: String,
    pub name: String,
    pub sources: Vec<PathBuf>,
    pub repository: PathBuf,
    #[serde(default)]
    pub excluded_paths: Vec<PathBuf>,
    pub excludes: Vec<String>,
    #[serde(default)]
    pub keep_last: Option<u32>,
    #[serde(default)]
    pub use_vss: bool,
    pub schedule: Schedule,
    pub enabled: bool,
    pub next_due: Option<DateTime<Utc>>,
    pub last_success: Option<DateTime<Utc>>,
}

impl Plan {
    pub fn destination_label(&self) -> String {
        self.remote
            .as_ref()
            .map(|r| r.label())
            .unwrap_or_else(|| self.repository.display().to_string())
    }
}

// Deliberately no Debug: requests contain a repository password.
#[derive(Clone, Serialize, Deserialize)]
pub struct NewPlan {
    #[serde(default)]
    pub remote: Option<crate::remote::Destination>,
    #[serde(default)]
    pub remote_credentials: Option<crate::remote::Credentials>,
    #[serde(default)]
    pub mode: BackupMode,
    pub name: String,
    pub sources: Vec<PathBuf>,
    pub repository: PathBuf,
    #[serde(default)]
    pub excluded_paths: Vec<PathBuf>,
    pub excludes: Vec<String>,
    #[serde(default)]
    pub keep_last: Option<u32>,
    #[serde(default)]
    pub use_vss: bool,
    pub schedule: Schedule,
    pub password: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupMode {
    #[default]
    Encrypted,
    Plain,
    Zip,
    TarGz,
    SevenZip,
}
impl std::fmt::Display for BackupMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::i18n::tr(match self {
            Self::Encrypted => "Criptat (Restic)",
            Self::Plain => "Fișiere necriptate (copy/paste)",
            Self::Zip => "Arhivă ZIP (necriptată)",
            Self::TarGz => "Arhivă TAR.GZ (necriptată)",
            Self::SevenZip => "Arhivă 7z (necriptată)",
        }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanChanges {
    #[serde(default)]
    pub remote: Option<crate::remote::Destination>,
    #[serde(default)]
    pub remote_credentials: Option<crate::remote::Credentials>,
    pub name: String,
    pub sources: Vec<PathBuf>,
    pub repository: PathBuf,
    #[serde(default)]
    pub excluded_paths: Vec<PathBuf>,
    pub excludes: Vec<String>,
    #[serde(default)]
    pub keep_last: Option<u32>,
    #[serde(default)]
    pub use_vss: bool,
    pub schedule: Schedule,
}

pub fn validate_empty_repository(path: &Path) -> Result<()> {
    if std::fs::read_dir(path)?.next().is_some() {
        bail!(
            "Folderul {} nu este gol. Creează și alege un subfolder dedicat (de exemplu Straja-backup), nu rădăcina discului. Fișierele existente nu au fost modificate.",
            path.display()
        );
    }
    Ok(())
}

fn canonical(path: &Path) -> Result<PathBuf> {
    Ok(std::fs::canonicalize(path)?)
}
pub fn overlaps(a: &Path, b: &Path) -> bool {
    // Canonicalize before this check, resolving Windows junctions and symlinks.
    let a = a.to_string_lossy().replace('/', "\\").to_lowercase();
    let b = b.to_string_lossy().replace('/', "\\").to_lowercase();
    let a = a.trim_end_matches('\\');
    let b = b.trim_end_matches('\\');
    a == b || a.starts_with(&format!("{b}\\")) || b.starts_with(&format!("{a}\\"))
}

impl NewPlan {
    pub fn validate(&self, data_dir: &Path) -> Result<()> {
        if self.mode == BackupMode::Encrypted && self.password.len() < 8 {
            bail!("Parola backupului trebuie să aibă minimum 8 caractere.");
        }
        if self.mode == BackupMode::Encrypted && self.password.contains(['\n', '\r', '\0']) {
            bail!("Parola nu poate conține caractere de control.");
        }
        PlanChanges {
            remote: self.remote.clone(),
            remote_credentials: self.remote_credentials.clone(),
            name: self.name.clone(),
            sources: self.sources.clone(),
            repository: self.repository.clone(),
            excluded_paths: self.excluded_paths.clone(),
            excludes: self.excludes.clone(),
            keep_last: self.keep_last,
            use_vss: self.use_vss,
            schedule: self.schedule.clone(),
        }
        .validate(data_dir)?;
        if self.mode != BackupMode::Encrypted {
            crate::plain::exclusions(&self.excludes)?;
        }
        if let Some(remote) = &self.remote {
            ensure_remote_mode(self.mode)?;
            self.remote_credentials
                .as_ref()
                .context("Conectează destinația online înainte de salvare.")?
                .validate(remote.provider)
        } else {
            validate_empty_repository(&self.repository)
        }
    }
}

impl PlanChanges {
    pub fn validate(&self, data_dir: &Path) -> Result<()> {
        anyhow::ensure!(
            self.remote.is_some() || self.remote_credentials.is_none(),
            "Configurația destinației online nu este validă."
        );
        if self.name.trim().is_empty() || self.name.len() > 120 {
            bail!("Introdu un nume de maximum 120 de caractere.");
        }
        if self.sources.is_empty() || self.sources.len() > 100 {
            bail!("Alege între 1 și 100 de fișiere sau foldere.");
        }
        crate::exclusions::validate_paths(&self.sources, &self.excluded_paths)?;
        validate_retention(self.keep_last)?;
        if self.use_vss {
            crate::vss::validate_sources(&self.sources)?;
        }
        self.schedule.validate()?;
        if let Some(remote) = &self.remote {
            remote.validate()?;
        } else if !self.repository.is_absolute() || !self.repository.is_dir() {
            bail!("Alege un folder de destinație existent, folosind calea completă.");
        }
        let repo = if self.remote.is_none() {
            Some(canonical(&self.repository)?)
        } else {
            None
        };
        if let Some(repo) = &repo
            && overlaps(repo, &canonical(data_dir)?)
        {
            bail!("Destinația nu poate suprapune datele aplicației.");
        }
        for source in &self.sources {
            if !source.is_absolute() {
                bail!("Sursele trebuie să aibă căi complete.");
            }
            let source = canonical(source)?;
            if repo.as_ref().is_some_and(|repo| overlaps(repo, &source)) {
                bail!("Sursa și destinația trebuie să fie foldere separate, fără suprapunere.");
            }
        }
        if self.excludes.len() > 100
            || self
                .excludes
                .iter()
                .any(|s| s.len() > 1024 || s.contains(['\n', '\r', '\0']))
        {
            bail!("Lista de excluderi nu este validă.");
        }
        Ok(())
    }
}

pub fn ensure_remote_mode(mode: BackupMode) -> Result<()> {
    anyhow::ensure!(
        mode == BackupMode::Encrypted,
        "Destinațiile online folosesc backup criptat și incremental."
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    pub plan_id: String,
    pub operation: String,
    pub percent: f32,
    pub files_done: u64,
    pub bytes_done: u64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub plan_id: String,
    pub plan_name: String,
    pub operation: String,
    pub started: DateTime<Utc>,
    pub finished: Option<DateTime<Utc>>,
    pub outcome: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub short_id: String,
    pub time: DateTime<Utc>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub agent_version: String,
    pub plans: Vec<Plan>,
    pub history: Vec<Run>,
    pub active: Option<Progress>,
    pub service_mode: bool,
    pub engine_available: bool,
    #[serde(default)]
    pub vss_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastOutcome {
    pub plan_name: String,
    pub outcome: String,
}

/// Tray health checks never transfer source paths, exclusions or full history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentStatus {
    pub agent_version: String,
    pub plan_count: u64,
    pub last_outcome: Option<LastOutcome>,
    pub active: Option<Progress>,
    pub service_mode: bool,
}

impl From<State> for AgentStatus {
    fn from(state: State) -> Self {
        Self {
            agent_version: state.agent_version,
            plan_count: state.plans.len() as u64,
            last_outcome: state
                .history
                .into_iter()
                .find(|r| r.finished.is_some())
                .map(|r| LastOutcome {
                    plan_name: r.plan_name,
                    outcome: r.outcome,
                }),
            active: state.active,
            service_mode: state.service_mode,
        }
    }
}

pub fn validate_retention(keep: Option<u32>) -> Result<()> {
    if keep.is_some_and(|n| !(1..=10000).contains(&n)) {
        bail!("Numărul de copii trebuie să fie între 1 și 10000, sau alege Păstrează toate.");
    }
    Ok(())
}
pub fn retention_label(keep: Option<u32>) -> String {
    match keep {
        None => "Toate copiile păstrate".into(),
        Some(1) => "Păstrează doar cea mai nouă copie".into(),
        Some(n) => format!("Păstrează ultimele {n} copii (inclusiv cea mai nouă)"),
    }
}
impl BackupMode {
    pub fn archive_name(self) -> Option<&'static str> {
        match self {
            Self::Zip => Some("Backup.zip"),
            Self::TarGz => Some("Backup.tar.gz"),
            Self::SevenZip => Some("Backup.7z"),
            _ => None,
        }
    }
}
