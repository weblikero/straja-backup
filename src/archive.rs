//! Standard, unencrypted archives. Extraction only writes manifest-approved regular files.
use crate::{
    engine::Engine,
    model::BackupMode,
    plain::{Entry, FILES, MANIFEST, Manifest, not_link},
};
use anyhow::{Context, Result, bail, ensure};
use chrono::{Datelike, Timelike};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

fn open_input(path: &Path) -> Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x00200000)
        .open(path)?;
    not_link(&file.metadata()?, path)?;
    ensure!(
        file.metadata()?.is_file(),
        "Fișier așteptat: {}",
        path.display()
    );
    Ok(file)
}
fn name(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .context("Nume Unicode nevalid")?
        .replace('\\', "/"))
}
fn safe_child(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in relative.components() {
        ensure!(
            matches!(part, std::path::Component::Normal(_)),
            "Cale nesigură."
        );
        path.push(part);
        not_link(&fs::symlink_metadata(&path)?, &path)?;
    }
    Ok(path)
}

pub(crate) async fn create(
    engine: &Engine,
    mode: BackupMode,
    folder: &Path,
    m: Manifest,
) -> Result<()> {
    let (engine, folder) = (engine.clone(), folder.to_path_buf());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let target = folder.join(mode.archive_name().context("Format de arhivă lipsă")?);
        let output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let mut paths = vec![PathBuf::from(MANIFEST), PathBuf::from(FILES)];
        paths.extend(m.entries.iter().map(|e| e.path.clone()));
        match mode {
            BackupMode::Zip => {
                let mut writer = zip::ZipWriter::new(output);
                for relative in paths {
                    engine.update(&format!("Comprim ZIP: {}", relative.display()));
                    let path = safe_child(&folder, &relative)?;
                    let meta = fs::symlink_metadata(&path)?;
                    let modified: chrono::DateTime<chrono::Local> = meta.modified()?.into();
                    let timestamp = zip::DateTime::from_date_and_time(
                        modified.year().clamp(1980, 2107) as u16,
                        modified.month() as u8,
                        modified.day() as u8,
                        modified.hour() as u8,
                        modified.minute() as u8,
                        modified.second() as u8,
                    )
                    .unwrap_or_default();
                    let options = zip::write::SimpleFileOptions::default()
                        .last_modified_time(timestamp)
                        .compression_method(zip::CompressionMethod::Deflated)
                        .large_file(meta.len() >= u32::MAX as u64);
                    if meta.is_dir() {
                        writer.add_directory(name(&relative)?, options)?;
                    } else {
                        writer.start_file(name(&relative)?, options)?;
                        io::copy(&mut open_input(&path)?, &mut writer)?;
                    }
                }
                writer.finish()?.sync_all()?;
            }
            BackupMode::TarGz => {
                let encoder = flate2::write::GzEncoder::new(output, flate2::Compression::default());
                let mut writer = tar::Builder::new(encoder);
                writer.follow_symlinks(false);
                for relative in paths {
                    engine.update(&format!("Comprim TAR.GZ: {}", relative.display()));
                    let path = safe_child(&folder, &relative)?;
                    if path.is_dir() {
                        writer.append_dir(name(&relative)?, &path)?;
                    } else {
                        writer.append_file(name(&relative)?, &mut open_input(&path)?)?;
                    }
                }
                writer.into_inner()?.finish()?.sync_all()?;
            }
            BackupMode::SevenZip => {
                let mut writer = sevenz_rust2::ArchiveWriter::new(output)?;
                for relative in paths {
                    engine.update(&format!("Comprim 7z: {}", relative.display()));
                    let path = safe_child(&folder, &relative)?;
                    let entry = sevenz_rust2::ArchiveEntry::from_path(&path, name(&relative)?);
                    let input = if path.is_dir() {
                        None
                    } else {
                        Some(open_input(&path)?)
                    };
                    writer.push_archive_entry(entry, input)?;
                }
                writer.finish()?.sync_all()?;
            }
            _ => bail!("Format necomprimat."),
        }
        verify_sync(&engine, mode, &folder, &m, None)
    })
    .await?
}

pub(crate) async fn verify(
    engine: &Engine,
    mode: BackupMode,
    folder: &Path,
    m: Manifest,
    output: Option<PathBuf>,
) -> Result<()> {
    let (engine, folder) = (engine.clone(), folder.to_path_buf());
    tokio::task::spawn_blocking(move || verify_sync(&engine, mode, &folder, &m, output.as_deref()))
        .await?
}

fn verify_sync(
    engine: &Engine,
    mode: BackupMode,
    folder: &Path,
    m: &Manifest,
    output: Option<&Path>,
) -> Result<()> {
    let archive = open_input(&folder.join(mode.archive_name().context("Format lipsă")?))?;
    let mut manifest_bytes = Vec::new();
    open_input(&folder.join(MANIFEST))?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut manifest_bytes)?;
    ensure!(
        manifest_bytes.len() <= 64 * 1024 * 1024,
        "Manifest prea mare."
    );
    let entries: HashMap<String, &Entry> = m
        .entries
        .iter()
        .map(|e| Ok((name(&e.path)?, e)))
        .collect::<Result<_>>()?;
    let mut seen = HashSet::new();
    // Never delegate extraction to an archive library: reject extras, links, duplicates and traversal.
    let mut consume =
        |raw_name: &str, directory: bool, size: u64, reader: &mut dyn Read| -> Result<()> {
            let key = raw_name.trim_end_matches('/');
            ensure!(
                seen.insert(key.to_owned()),
                "Intrare duplicată în arhivă: {key}"
            );
            engine.update(&format!(
                "{}: {key}",
                if output.is_some() {
                    "Extrag și verific"
                } else {
                    "Verific arhiva"
                }
            ));
            if key == MANIFEST {
                ensure!(
                    !directory && size == manifest_bytes.len() as u64,
                    "Manifest arhivat nevalid."
                );
                let mut bytes = Vec::new();
                reader.take(size + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes == manifest_bytes,
                    "Manifestul arhivei diferă de metadatele copiei."
                );
                return Ok(());
            }
            if key == FILES {
                ensure!(directory && size == 0, "Rădăcină nevalidă în arhivă.");
                return Ok(());
            }
            let e = entries
                .get(key)
                .with_context(|| format!("Intrare neautorizată în arhivă: {key}"))?;
            ensure!(
                e.directory == directory && e.size == size,
                "Metadate diferite în arhivă: {key}"
            );
            let target = output
                .map(|root| root.join(e.path.strip_prefix(FILES).expect("validated manifest")));
            if directory {
                if let Some(target) = target {
                    fs::create_dir_all(target)?;
                }
                return Ok(());
            }
            let mut file = if let Some(target) = &target {
                fs::create_dir_all(target.parent().context("Folder lipsă")?)?;
                Some(
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(target)?,
                )
            } else {
                None
            };
            let mut hash = Sha256::new();
            let mut count = 0u64;
            let mut buffer = vec![0; 1024 * 1024];
            let mut bounded = reader.take(size.checked_add(1).context("Dimensiune prea mare")?);
            loop {
                let n = bounded.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                count += n as u64;
                ensure!(count <= size, "Date în exces în arhivă: {key}");
                hash.update(&buffer[..n]);
                if let Some(file) = file.as_mut() {
                    file.write_all(&buffer[..n])?;
                }
            }
            ensure!(
                count == size && format!("{:x}", hash.finalize()) == e.sha256,
                "Fișier corupt în arhivă: {key}"
            );
            if let Some(file) = file {
                file.sync_all()?;
                file.set_times(fs::FileTimes::new().set_modified(e.modified.into()))?;
                drop(file);
                // Verify persisted bytes as well, not just the decompressed stream.
                let mut input = open_input(target.as_ref().unwrap())?;
                let mut hash = Sha256::new();
                io::copy(&mut input, &mut hash)?;
                ensure!(
                    format!("{:x}", hash.finalize()) == e.sha256,
                    "Verificarea restaurării a eșuat: {key}"
                );
            }
            Ok(())
        };
    match mode {
        BackupMode::Zip => {
            let mut reader = zip::ZipArchive::new(archive)?;
            ensure!(
                reader.len() == entries.len() + 2,
                "Număr nevalid de intrări ZIP."
            );
            for i in 0..reader.len() {
                let mut entry = reader.by_index(i)?;
                ensure!(!entry.is_symlink(), "Legătură neacceptată în ZIP.");
                let entry_name = entry.name().to_string();
                consume(&entry_name, entry.is_dir(), entry.size(), &mut entry)?;
            }
        }
        BackupMode::TarGz => {
            let decoder = flate2::read::GzDecoder::new(archive);
            let mut reader = tar::Archive::new(decoder);
            for entry in reader.entries()? {
                let mut entry = entry?;
                let kind = entry.header().entry_type();
                ensure!(kind.is_file() || kind.is_dir(), "Tip neacceptat în TAR.");
                consume(
                    &name(&entry.path()?)?,
                    kind.is_dir(),
                    entry.size(),
                    &mut entry,
                )?;
            }
            // Force gzip trailer/checksum validation after the TAR end marker.
            io::copy(&mut reader.into_inner(), &mut io::sink())?;
        }
        BackupMode::SevenZip => {
            let mut reader =
                sevenz_rust2::ArchiveReader::new(archive, sevenz_rust2::Password::empty())?;
            reader.for_each_entries(|entry, stream| {
                consume(&entry.name, entry.is_directory, entry.size, stream)
                    .map(|_| true)
                    .map_err(|e| sevenz_rust2::Error::from(io::Error::other(e.to_string())))
            })?;
        }
        _ => bail!("Format necomprimat."),
    }
    ensure!(seen.len() == entries.len() + 2, "Fișiere lipsă în arhivă.");
    Ok(())
}
