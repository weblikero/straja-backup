//! Bounded, streaming JSON I/O. The parsed index is still held in memory, but
//! serialized manifests and archived manifests never need an additional full copy.
use anyhow::{Context, Result, ensure};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{self, BufReader, BufWriter, Read, Write},
    path::Path,
};

pub(crate) const MANIFEST_LIMIT: u64 = 512 * 1024 * 1024;
pub(crate) const REPOSITORY_LIMIT: u64 = 64 * 1024 * 1024;
pub(crate) const LIMIT_ERROR: &str = "Metadatele copiei depășesc limita acceptată.";
const BUFFER_SIZE: usize = 256 * 1024;

pub(crate) struct BudgetWriter<W> {
    inner: W,
    remaining: u64,
    used: u64,
}
impl<W> BudgetWriter<W> {
    pub(crate) fn new(inner: W, limit: u64) -> Self {
        Self {
            inner,
            remaining: limit,
            used: 0,
        }
    }
    #[cfg(test)]
    pub(crate) fn used(&self) -> u64 {
        self.used
    }
}
impl<W: Write> Write for BudgetWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(io::Error::other(LIMIT_ERROR));
        }
        let count = self.inner.write(bytes)?;
        self.remaining -= count as u64;
        self.used += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn read<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    let file = crate::platform::open_read(path)?;
    let meta = file.metadata()?;
    crate::plain::not_link(&meta, path)?;
    ensure!(meta.is_file() && meta.len() <= limit, LIMIT_ERROR);
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file.take(limit + 1));
    let result = serde_json::from_reader(&mut reader);
    ensure!(limit + 1 - reader.get_ref().limit() <= limit, LIMIT_ERROR);
    result.context("Metadate de backup nevalide")
}

pub(crate) fn write<T: Serialize>(path: &Path, value: &T, limit: u64) -> Result<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = BudgetWriter::new(BufWriter::with_capacity(BUFFER_SIZE, file), limit);
    // Compact JSON is fully compatible with the existing version-1 reader.
    serde_json::to_writer(&mut writer, value)?;
    writer.flush()?;
    writer.inner.get_ref().sync_all()?;
    Ok(())
}

pub(crate) fn fingerprint(reader: impl Read, limit: u64) -> Result<(u64, [u8; 32])> {
    let mut reader = reader.take(limit + 1);
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = vec![0; BUFFER_SIZE];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        ensure!(bytes <= limit, LIMIT_ERROR);
        hash.update(&buffer[..count]);
    }
    Ok((bytes, hash.finalize().into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn budgets_reject_excess_without_truncating_valid_json() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("metadata.json");
        let value = vec!["Unicode Grüße", "second"];
        let mut counter = BudgetWriter::new(io::sink(), 1024);
        serde_json::to_writer(&mut counter, &value)?;
        let size = counter.used();
        write(&path, &value, size)?;
        assert_eq!(read::<Vec<String>>(&path, size)?, value);
        assert!(read::<Vec<String>>(&path, size - 1).is_err());
        assert!(write(&temp.path().join("too-small.json"), &value, size - 1).is_err());
        assert!(fingerprint(File::open(&path)?, size - 1).is_err());
        assert_eq!(fingerprint(File::open(&path)?, size)?.0, size);
        // A valid JSON prefix followed by another value must not be accepted.
        std::fs::write(&path, b"[1] [2]")?;
        assert!(read::<Vec<u64>>(&path, 1024).is_err());
        Ok(())
    }
}
