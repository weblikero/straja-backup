//! Versioned plain or compressed copies. Sources are never deleted or overwritten.
use crate::{engine::Engine, model::*};
use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Utc};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
};

pub const MARKER: &str = ".straja-plain.json";
pub(crate) const MANIFEST: &str = ".straja-manifest.json";
pub(crate) const FILES: &str = "Fisiere";
// Reuse across files and verification. Repeated 1 MiB zeroed allocations per
// small file cause excessive page faults in Windows background processing mode.
const IO_BUFFER_SIZE: usize = 256 * 1024;

#[derive(Serialize, Deserialize)]
struct Repository {
    #[serde(default = "plain_mode")]
    mode: BackupMode,
    version: u32,
    plan_id: String,
}
fn plain_mode() -> BackupMode {
    BackupMode::Plain
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Manifest {
    #[serde(default = "plain_mode")]
    pub(crate) mode: BackupMode,
    pub(crate) version: u32,
    pub(crate) plan_id: String,
    pub(crate) id: String,
    pub(crate) time: DateTime<Utc>,
    pub(crate) sources: Vec<String>,
    pub(crate) entries: Vec<Entry>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) path: PathBuf,
    pub(crate) directory: bool,
    pub(crate) size: u64,
    pub(crate) sha256: String,
    pub(crate) modified: DateTime<Utc>,
}

/// Conservative serialized-size budget, accumulated while scanning the sources.
/// No second entry list or serialized JSON buffer is allocated for this estimate.
struct ManifestBudget(crate::metadata::BudgetWriter<std::io::Sink>);
impl ManifestBudget {
    fn new(header: &Manifest, limit: u64) -> Result<Self> {
        let mut writer = crate::metadata::BudgetWriter::new(std::io::sink(), limit);
        serde_json::to_writer(&mut writer, header)?;
        Ok(Self(writer))
    }
    fn add(&mut self, path: &Path, directory: bool, modified: DateTime<Utc>) -> Result<()> {
        use std::io::Write;
        #[derive(Serialize)]
        struct Estimate<'a> {
            path: &'a Path,
            directory: bool,
            size: u64,
            sha256: &'a str,
            modified: DateTime<Utc>,
        }
        let entry = Estimate {
            path,
            directory,
            size: if directory { 0 } else { u64::MAX },
            sha256: if directory {
                ""
            } else {
                "0000000000000000000000000000000000000000000000000000000000000000"
            },
            modified,
        };
        (|| -> Result<()> {
            serde_json::to_writer(&mut self.0, &entry)?;
            // Comma plus margin for a different timestamp representation if a live
            // file changes between scanning and copying. Sizes already use u64::MAX.
            self.0.write_all(&[0; 33])?;
            Ok(())
        })().context("Prea multe metadate pentru o singură copie (maximum 512 MB). Împarte sursele în mai multe planuri.")
    }
}

pub fn exclusions(patterns: &[String]) -> Result<GlobSet> {
    let mut set = GlobSetBuilder::new();
    for pattern in patterns {
        ensure!(
            !pattern.starts_with('!'),
            "În modul necriptat, excluderile nu acceptă prefixul !."
        );
        let pattern = pattern.replace('\\', "/");
        set.add(
            GlobBuilder::new(&pattern)
                .case_insensitive(cfg!(windows))
                .backslash_escape(false)
                .build()
                .with_context(|| format!("Excludere nevalidă: {pattern}"))?,
        );
    }
    Ok(set.build()?)
}

pub(crate) fn not_link(meta: &std::fs::Metadata, path: &Path) -> Result<()> {
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;
    #[cfg(windows)]
    ensure!(
        meta.file_attributes() & 0x400 == 0,
        "Legătură/junction/reparse point neacceptat: {}. Alege folderul real sau exclude legătura.",
        path.display()
    );
    ensure!(
        !meta.file_type().is_symlink(),
        "Symbolic link is not supported: {}. Exclude the link or choose its real target.",
        path.display()
    );
    ensure!(
        meta.is_dir() || meta.is_file(),
        "Tip de fișier neacceptat: {}",
        path.display()
    );
    Ok(())
}

async fn read_json<T: serde::de::DeserializeOwned + Send + 'static>(
    path: &Path,
    limit: u64,
) -> Result<T> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || crate::metadata::read(&path, limit)).await?
}
async fn write_json<T: Serialize + Send + 'static>(path: &Path, value: T, limit: u64) -> Result<T> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        crate::metadata::write(&path, &value, limit)?;
        Ok(value)
    })
    .await?
}
async fn repository(plan: &Plan) -> Result<PathBuf> {
    let root = fs::canonicalize(&plan.repository)
        .await
        .context("Destinația nu este disponibilă")?;
    let marker: Repository = read_json(&root.join(MARKER), crate::metadata::REPOSITORY_LIMIT)
        .await
        .context("Folderul nu este o destinație de backup necriptat Straja")?;
    ensure!(
        marker.version == 1 && marker.plan_id == plan.id && marker.mode == plan.mode,
        "Destinația aparține altui plan sau unui format neacceptat."
    );
    Ok(root)
}
fn valid_id(id: &str) -> bool {
    id.starts_with("Backup-")
        && id.len() < 100
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn validate_manifest(m: &Manifest, plan: &Plan, id: &str) -> Result<()> {
    ensure!(
        m.version == 1 && m.plan_id == plan.id && m.id == id && m.mode == plan.mode,
        "Copia nu aparține acestui plan sau are metadate nevalide."
    );
    let mut seen = HashSet::new();
    for entry in &m.entries {
        ensure!(
            entry.path.starts_with(FILES) && entry.path != Path::new(FILES),
            "Cale nevalidă în manifest."
        );
        for part in entry.path.components() {
            let Component::Normal(part) = part else {
                bail!("Cale nesigură în manifest.");
            };
            let part = part.to_str().context("Nume nevalid în manifest")?;
            ensure!(
                !part.contains(':') && !part.ends_with(['.', ' ']),
                "Cale nesigură în manifest."
            );
        }
        ensure!(
            seen.insert(crate::platform::path_key(&entry.path)),
            "Cale duplicată în manifest."
        );
        ensure!(
            (entry.directory && entry.size == 0 && entry.sha256.is_empty())
                || (!entry.directory
                    && entry.sha256.len() == 64
                    && entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())),
            "Sumă de control nevalidă."
        );
    }
    Ok(())
}
async fn manifest(plan: &Plan, id: &str) -> Result<(PathBuf, Manifest)> {
    ensure!(valid_id(id), "Identificator de copie nevalid.");
    let folder = repository(plan).await?.join(id);
    not_link(&fs::symlink_metadata(&folder).await?, &folder)?;
    let path = folder.join(MANIFEST);
    let (plan, id) = (plan.clone(), id.to_owned());
    let m = tokio::task::spawn_blocking(move || {
        let m = crate::metadata::read(&path, crate::metadata::MANIFEST_LIMIT)?;
        validate_manifest(&m, &plan, &id)?;
        Ok::<_, anyhow::Error>(m)
    })
    .await??;
    Ok((folder, m))
}
async fn checked_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in relative.components() {
        ensure!(
            matches!(part, Component::Normal(_)),
            "Cale nesigură în copie."
        );
        path.push(part);
        not_link(&fs::symlink_metadata(&path).await?, &path)?;
    }
    Ok(path)
}
fn progress(engine: &Engine, files: u64, bytes: u64, total: u64, detail: String) {
    if let Some(p) = engine.live.lock().unwrap().as_mut() {
        p.files_done = files;
        p.bytes_done = bytes;
        p.percent = (bytes as f64 / total.max(1) as f64).clamp(0., 1.) as f32;
        p.detail = detail;
    }
}
async fn digest(path: &Path, buffer: &mut [u8]) -> Result<(String, u64)> {
    let mut file = fs::File::open(path).await?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    loop {
        let count = file.read(buffer).await?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        bytes += count as u64;
    }
    Ok((format!("{:x}", hash.finalize()), bytes))
}
async fn copy_file(
    engine: &Engine,
    from: &Path,
    logical: &Path,
    to: &Path,
    completed: (u64, u64, u64),
    buffer: &mut [u8],
) -> Result<(String, u64, DateTime<Utc>)> {
    let (files_done, bytes_done, total) = completed;
    let input = crate::platform::open_read(from).with_context(|| {
        format!(
            "Fișierul nu poate fi citit (posibil blocat): {}",
            logical.display()
        )
    })?;
    let before = input.metadata()?;
    not_link(&before, logical)?;
    let modified = before.modified()?;
    let mut input = fs::File::from_std(input);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .await?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    loop {
        let count = input.read(buffer).await?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count]).await?;
        hash.update(&buffer[..count]);
        bytes += count as u64;
        progress(
            engine,
            files_done,
            bytes_done + bytes,
            total,
            format!("Copiez {}", logical.display()),
        );
    }
    ensure!(
        bytes == before.len() && input.metadata().await?.modified()? == modified,
        "Fișierul s-a modificat în timpul copierii: {}",
        logical.display()
    );
    // Release the source's read lock before flushing and verifying the destination.
    drop(input);
    output.sync_all().await?;
    let output = output.into_std().await;
    output.set_times(std::fs::FileTimes::new().set_modified(modified))?;
    drop(output);
    let expected = format!("{:x}", hash.finalize());
    engine.update(&format!("Verific copia: {}", to.display()));
    ensure!(
        digest(to, buffer).await? == (expected.clone(), bytes),
        "Verificarea fișierului copiat a eșuat: {}",
        to.display()
    );
    Ok((expected, bytes, modified.into()))
}

pub async fn backup(engine: &Engine, plan: &Plan) -> Result<(String, String)> {
    let root = fs::canonicalize(&plan.repository)
        .await
        .context("Destinația nu este disponibilă")?;
    let internal = fs::canonicalize(&engine.store.root).await?;
    ensure!(
        !overlaps(&root, &internal),
        "Destinația suprapune datele aplicației."
    );
    for source in &plan.sources {
        not_link(&fs::symlink_metadata(source).await?, source)?;
        ensure!(
            !overlaps(&root, &fs::canonicalize(source).await?),
            "Sursa și destinația se suprapun."
        );
    }
    let snapshots = if plan.use_vss {
        engine.update("Creez snapshotul Windows VSS…");
        Some(crate::vss::SnapshotSet::create(plan.sources.clone()).await?)
    } else {
        None
    };
    if !root.join(MARKER).exists() {
        ensure!(
            plan.last_success.is_none(),
            "Metadatele destinației lipsesc. Nu recreez un depozit existent."
        );
        validate_empty_repository(&root)?;
        write_json(
            &root.join(MARKER),
            Repository {
                mode: plan.mode,
                version: 1,
                plan_id: plan.id.clone(),
            },
            crate::metadata::REPOSITORY_LIMIT,
        )
        .await?;
    }
    repository(plan).await?;
    let time = Utc::now();
    let id = format!(
        "Backup-{}-{}",
        time.format("%Y%m%d-%H%M%S"),
        uuid::Uuid::new_v4().simple()
    );
    let staging = root.join(format!(".in-progress-{id}"));
    fs::create_dir(&staging).await?;
    let result = backup_into(
        engine,
        plan,
        &internal,
        &staging,
        &id,
        time,
        snapshots.as_ref(),
    )
    .await;
    // Release before archive compression, retaining a cleanup error and the partial copy.
    let cleanup = match snapshots {
        Some(set) => set.close().await,
        None => Ok(()),
    };
    let m = result.and_then(|manifest| cleanup.map(|()| manifest)).with_context(|| {
        format!(
            "Backup nefinalizat. Copia parțială se află în {}; versiunile anterioare sunt intacte.",
            staging.display()
        )
    })?;
    if plan.mode.archive_name().is_some() {
        let m = crate::archive::create(engine, plan.mode, &staging, m)
            .await
            .with_context(|| {
                format!(
                    "Compresie nefinalizată. Copie parțială: {}",
                    staging.display()
                )
            })?;
        // The complete archive was decoded and SHA-256 checked before removing staging files.
        remove_known_tree(&staging, &m, BackupMode::Plain, true, false).await?;
    }
    let final_path = root.join(&id);
    fs::rename(&staging, &final_path).await?;
    if let Some(archive) = plan.mode.archive_name() {
        return Ok((
            "Reușit".into(),
            format!(
                "Arhivă finalizată și verificată SHA-256: {}",
                final_path.join(archive).display()
            ),
        ));
    }
    Ok((
        "Reușit".into(),
        format!(
            "Backup necriptat finalizat și verificat. Fișierele pot fi deschise și copiate din {}",
            final_path.join(FILES).display()
        ),
    ))
}

async fn backup_into(
    engine: &Engine,
    plan: &Plan,
    internal: &Path,
    staging: &Path,
    id: &str,
    time: DateTime<Utc>,
    snapshots: Option<&crate::vss::SnapshotSet>,
) -> Result<Manifest> {
    let patterns = exclusions(&plan.excludes)?;
    let selected = crate::exclusions::PathMatcher::new(&plan.excluded_paths);
    let internal_paths = crate::exclusions::PathMatcher::new(&[internal.to_path_buf()]);
    let sources = plan
        .sources
        .iter()
        .map(|p| {
            p.to_str()
                .map(String::from)
                .context("Sursă cu nume Unicode nevalid")
        })
        .collect::<Result<Vec<_>>>()?;
    let mut m = Manifest {
        mode: plan.mode,
        version: 1,
        plan_id: plan.id.clone(),
        id: id.into(),
        time,
        sources,
        entries: Vec::new(),
    };
    let mut budget = ManifestBudget::new(&m, crate::metadata::MANIFEST_LIMIT)?;
    engine.update("Scanez sursele pentru copia necriptată…");
    let mut stack = Vec::new();
    for (i, source) in plan.sources.iter().enumerate() {
        let name = source
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Disc");
        stack.push((
            source.clone(),
            PathBuf::from(FILES).join(format!("{:02}-{name}", i + 1)),
            source.clone(),
        ));
    }
    let mut items = Vec::new();
    let mut total = 0u64;
    while let Some((source, destination, base)) = stack.pop() {
        let relative = source
            .strip_prefix(&base)
            .unwrap_or(&source)
            .to_string_lossy()
            .replace('\\', "/");
        if selected.contains(&source)
            || patterns.is_match(&relative)
            || patterns.is_match(source.file_name().unwrap_or_default())
            || patterns.is_match(source.to_string_lossy().replace('\\', "/"))
        {
            continue;
        }
        let physical = match snapshots {
            Some(set) => set.resolve(&source)?,
            None => source.clone(),
        };
        let meta = fs::symlink_metadata(&physical)
            .await
            .with_context(|| format!("Sursa nu poate fi citită: {}", source.display()))?;
        not_link(&meta, &source)?;
        let is_internal = if snapshots.is_some() {
            internal_paths.contains(&source)
        } else {
            fs::canonicalize(&source).await?.starts_with(internal)
        };
        if is_internal {
            continue;
        }
        if meta.is_dir() {
            let mut children = fs::read_dir(&physical).await?;
            while let Some(child) = children.next_entry().await? {
                stack.push((
                    source.join(child.file_name()),
                    destination.join(child.file_name()),
                    base.clone(),
                ));
            }
        } else {
            total = total
                .checked_add(meta.len())
                .context("Dimensiune prea mare")?;
        }
        let modified = meta.modified()?;
        // Fail during enumeration, before copying any file, if metadata cannot fit.
        budget.add(&destination, meta.is_dir(), modified.into())?;
        items.push((source, destination, meta.is_dir(), modified));
    }
    fs::create_dir(staging.join(FILES)).await?;
    let mut entries = Vec::new();
    let (mut files, mut bytes) = (0, 0);
    let mut buffer = vec![0; IO_BUFFER_SIZE];
    for (source, relative, directory, modified) in items {
        let target = staging.join(&relative);
        if directory {
            fs::create_dir_all(&target).await?;
            entries.push(Entry {
                path: relative,
                directory: true,
                size: 0,
                sha256: String::new(),
                modified: modified.into(),
            });
        } else {
            fs::create_dir_all(target.parent().context("Folder lipsă")?).await?;
            let physical = match snapshots {
                Some(set) => set.resolve(&source)?,
                None => source.clone(),
            };
            let (sha256, size, modified) = copy_file(
                engine,
                &physical,
                &source,
                &target,
                (files, bytes, total),
                &mut buffer,
            )
            .await?;
            files += 1;
            bytes += size;
            entries.push(Entry {
                path: relative,
                directory: false,
                size,
                sha256,
                modified,
            });
            progress(
                engine,
                files,
                bytes,
                total,
                format!("{} fișiere copiate și verificate", files),
            );
        }
    }
    m.entries = entries;
    validate_manifest(&m, plan, id)?;
    write_json(&staging.join(MANIFEST), m, crate::metadata::MANIFEST_LIMIT).await
}

pub async fn snapshots(plan: &Plan) -> Result<Vec<Snapshot>> {
    let root = repository(plan).await?;
    let mut children = fs::read_dir(root).await?;
    let mut snapshots = Vec::new();
    while let Some(child) = children.next_entry().await? {
        let id = child.file_name().to_string_lossy().into_owned();
        if !valid_id(&id) {
            continue;
        }
        let (_, m) = manifest(plan, &id).await?;
        snapshots.push(Snapshot {
            short_id: id.clone(),
            id,
            time: m.time,
            paths: m.sources,
            tags: vec![format!("straja:{}", plan.id)],
        });
    }
    snapshots.sort_by_key(|s| std::cmp::Reverse(s.time));
    Ok(snapshots)
}
async fn verify_entry(root: &Path, entry: &Entry, buffer: &mut [u8]) -> Result<PathBuf> {
    let path = checked_path(root, &entry.path).await?;
    if entry.directory {
        ensure!(path.is_dir(), "Folder lipsă: {}", path.display());
    } else {
        ensure!(
            digest(&path, buffer).await? == (entry.sha256.clone(), entry.size),
            "Fișier modificat sau corupt: {}",
            path.display()
        );
    }
    Ok(path)
}
pub async fn check(engine: &Engine, plan: &Plan) -> Result<(String, String)> {
    let copies = snapshots(plan).await?;
    ensure!(!copies.is_empty(), "Nu există încă un backup finalizat.");
    let mut buffer = vec![0; IO_BUFFER_SIZE];
    for snapshot in &copies {
        let (folder, m) = manifest(plan, &snapshot.id).await?;
        if plan.mode.archive_name().is_some() {
            crate::archive::verify(engine, plan.mode, &folder, std::sync::Arc::new(m), None)
                .await?;
            continue;
        }
        for entry in &m.entries {
            engine.update(&format!("Verific {}", entry.path.display()));
            verify_entry(&folder, entry, &mut buffer).await?;
        }
    }
    Ok((
        "Reușit".into(),
        format!("{} copii verificate prin SHA-256.", copies.len()),
    ))
}
pub async fn restore(
    engine: &Engine,
    plan: &Plan,
    id: &str,
    parent: &Path,
) -> Result<(String, String)> {
    let (folder, m) = manifest(plan, id).await?;
    let parent = fs::canonicalize(parent).await?;
    ensure!(
        parent.is_dir()
            && !overlaps(&parent, &repository(plan).await?)
            && !overlaps(&parent, &fs::canonicalize(&engine.store.root).await?),
        "Alege un folder de restaurare separat de depozit și datele aplicației."
    );
    if plan.mode.archive_name().is_some() {
        let m = std::sync::Arc::new(m);
        crate::archive::verify(engine, plan.mode, &folder, m.clone(), None).await?;
        let staging = parent.join(format!(
            ".Straja-Restaurare-incompleta-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&staging).await?;
        crate::archive::verify(engine, plan.mode, &folder, m, Some(staging.clone()))
            .await
            .with_context(|| format!("Restaurare nefinalizată: {}", staging.display()))?;
        let target = parent.join(format!("Straja-Restaurare-{}", uuid::Uuid::new_v4()));
        fs::rename(staging, &target).await?;
        return Ok((
            "Reușit".into(),
            format!("Arhivă extrasă și verificată în {}", target.display()),
        ));
    }
    // Verify before creating a destination, including untrusted manifest paths.
    let mut buffer = vec![0; IO_BUFFER_SIZE];
    for entry in &m.entries {
        verify_entry(&folder, entry, &mut buffer).await?;
    }
    let staging = parent.join(format!(
        ".Straja-Restaurare-incompleta-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&staging).await?;
    let total = m.entries.iter().try_fold(0u64, |sum, e| {
        sum.checked_add(e.size)
            .context("Dimensiune prea mare în manifest")
    })?;
    let (mut files, mut bytes) = (0, 0);
    for entry in &m.entries {
        let source = checked_path(&folder, &entry.path).await?;
        let target = staging.join(entry.path.strip_prefix(FILES)?);
        if entry.directory {
            fs::create_dir_all(&target).await?;
        } else {
            fs::create_dir_all(target.parent().context("Folder lipsă")?).await?;
            let (hash, size, _) = copy_file(
                engine,
                &source,
                &source,
                &target,
                (files, bytes, total),
                &mut buffer,
            )
            .await?;
            ensure!(
                hash == entry.sha256 && size == entry.size,
                "Copia s-a modificat în timpul restaurării."
            );
            files += 1;
            bytes += size;
        }
    }
    let target = parent.join(format!("Straja-Restaurare-{}", uuid::Uuid::new_v4()));
    fs::rename(&staging, &target).await?;
    Ok((
        "Reușit".into(),
        format!("Fișiere restaurate și verificate în {}", target.display()),
    ))
}

// Enumerate and validate the entire tree before deleting anything; never recursively remove
// arbitrary directories. Extra user files and reparse points stop cleanup.
async fn remove_known_tree(
    folder: &Path,
    m: &Manifest,
    mode: BackupMode,
    staging_only: bool,
    dry_run: bool,
) -> Result<()> {
    let mut allowed = std::collections::HashMap::new();
    if let Some(archive) = mode.archive_name() {
        allowed.insert(PathBuf::from(archive), false);
    } else {
        allowed.insert(PathBuf::from(FILES), true);
        for e in &m.entries {
            allowed.insert(e.path.clone(), e.directory);
        }
    }
    if !staging_only {
        allowed.insert(PathBuf::from(MANIFEST), false);
    }
    let mut stack = if staging_only {
        vec![PathBuf::from(FILES)]
    } else {
        let mut children = fs::read_dir(folder).await?;
        let mut paths = Vec::new();
        while let Some(child) = children.next_entry().await? {
            paths.push(PathBuf::from(child.file_name()));
        }
        paths
    };
    let mut files = Vec::new();
    let mut directories = Vec::new();
    while let Some(relative) = stack.pop() {
        let path = checked_path(folder, &relative).await?;
        let meta = fs::symlink_metadata(&path).await?;
        ensure!(
            allowed.get(&relative) == Some(&meta.is_dir()),
            "Curățare oprită: conținut necunoscut în {}",
            path.display()
        );
        if meta.is_dir() {
            let mut children = fs::read_dir(&path).await?;
            while let Some(child) = children.next_entry().await? {
                stack.push(relative.join(child.file_name()));
            }
            directories.push(relative);
        } else {
            files.push(relative);
        }
    }
    ensure!(
        files.len() + directories.len() == allowed.len(),
        "Curățare oprită: copie incompletă."
    );
    if dry_run {
        return Ok(());
    }
    // Manifest last, so an interrupted deletion remains detectable as a damaged old copy.
    files.sort_by_key(|p| p == Path::new(MANIFEST));
    for relative in files {
        fs::remove_file(checked_path(folder, &relative).await?).await?;
    }
    directories.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for relative in directories {
        fs::remove_dir(checked_path(folder, &relative).await?).await?;
    }
    if !staging_only {
        fs::remove_dir(folder).await?;
    }
    Ok(())
}

pub async fn remove_snapshot(plan: &Plan, id: &str) -> Result<()> {
    let (folder, m) = manifest(plan, id).await?;
    let root = repository(plan).await?;
    ensure!(
        folder.parent() == Some(root.as_path()),
        "Cale în afara destinației."
    );
    remove_known_tree(&folder, &m, plan.mode, false, false).await
}

pub async fn validate_removal(plan: &Plan, id: &str) -> Result<()> {
    let (folder, m) = manifest(plan, id).await?;
    remove_known_tree(&folder, &m, plan.mode, false, true).await
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    fn header() -> Manifest {
        Manifest {
            mode: BackupMode::Plain,
            version: 1,
            plan_id: "large-plan".into(),
            id: "Backup-large".into(),
            time: Utc::now(),
            sources: vec![r"C:\Source".into()],
            entries: Vec::new(),
        }
    }

    #[test]
    fn large_manifest_roundtrip_exceeds_old_limit() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join(MANIFEST);
        let mut m = header();
        let segment = "a".repeat(80);
        let prefix = format!("Fisiere/{segment}/{segment}/{segment}/{segment}");
        for i in 0..180_000 {
            m.entries.push(Entry {
                path: PathBuf::from(format!("{prefix}/file-{i:06}.txt")),
                directory: false,
                size: i,
                sha256: "0".repeat(64),
                modified: m.time,
            });
        }
        crate::metadata::write(&path, &m, crate::metadata::MANIFEST_LIMIT)?;
        assert!(std::fs::metadata(&path)?.len() > 64 * 1024 * 1024);
        drop(m);
        let loaded: Manifest = crate::metadata::read(&path, crate::metadata::MANIFEST_LIMIT)?;
        assert_eq!(loaded.entries.len(), 180_000);
        assert_eq!(loaded.entries.last().unwrap().size, 179_999);
        let plan = Plan {
            remote: None,
            mode: BackupMode::Plain,
            id: "large-plan".into(),
            name: "test".into(),
            sources: vec![],
            repository: temp.path().into(),
            excluded_paths: vec![],
            excludes: vec![],
            keep_last: None,
            use_vss: false,
            schedule: Schedule::Manual,
            enabled: false,
            next_due: None,
            last_success: None,
        };
        validate_manifest(&loaded, &plan, "Backup-large")?;
        Ok(())
    }

    #[test]
    fn scan_budget_covers_escaped_paths_and_rejects_excess_early() -> Result<()> {
        let mut m = header();
        let mut budget = ManifestBudget::new(&m, crate::metadata::MANIFEST_LIMIT)?;
        for (path, directory) in [(r"Fisiere\Grüße", true), (r"Fisiere\Grüße\雪.txt", false)] {
            let path = PathBuf::from(path);
            budget.add(&path, directory, m.time)?;
            m.entries.push(Entry {
                path,
                directory,
                size: if directory { 0 } else { 42 },
                sha256: if directory {
                    String::new()
                } else {
                    "f".repeat(64)
                },
                modified: m.time,
            });
        }
        let mut actual =
            crate::metadata::BudgetWriter::new(std::io::sink(), crate::metadata::MANIFEST_LIMIT);
        serde_json::to_writer(&mut actual, &m)?;
        assert!(actual.used() <= budget.0.used());
        let mut small = ManifestBudget::new(&header(), 300)?;
        assert!(
            small
                .add(Path::new("Fisiere/file.txt"), false, m.time)
                .is_err()
        );
        Ok(())
    }
}
