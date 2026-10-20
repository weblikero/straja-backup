//! Native login keychain holds an AES wrapping key; SQLite contains authenticated ciphertext.
//! No plaintext fallback when the keychain is absent or locked.
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use anyhow::{Context, Result, ensure};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use zeroize::Zeroizing;

static KEY_LOCK: Mutex<()> = Mutex::new(());
const PREFIX: &[u8] = b"STRAJA-AES1";

pub fn private_directory(path: &Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder.create(path)?;
    let meta = std::fs::symlink_metadata(path)?;
    ensure!(
        meta.is_dir()
            && !meta.file_type().is_symlink()
            && meta.uid() == unsafe { libc::geteuid() }
            && meta.mode() & 0o077 == 0,
        "Straja's private directory must be owned by this user with permissions 0700: {}",
        path.display()
    );
    Ok(())
}
pub fn socket_path() -> Result<PathBuf> {
    let root = crate::security::data_dir()?;
    private_directory(&root)?;
    let path = root.join("agent.sock");
    ensure!(
        path.as_os_str().len() < 104,
        "Straja data directory is too long for a local socket."
    );
    Ok(path)
}

fn key(create: bool) -> Result<Zeroizing<Vec<u8>>> {
    let _lock = KEY_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Keychain lock failed"))?;
    let entry = keyring::Entry::new("com.weblike.straja-backup", "database-wrapping-key-v1")?;
    let bytes = match entry.get_secret() {
        Ok(bytes) => bytes,
        Err(keyring::Error::NoEntry) if create => {
            let mut key = Zeroizing::new(vec![0u8; 32]);
            OsRng.fill_bytes(&mut key);
            entry.set_secret(&key).context("Unlock your login keychain to save backup credentials")?;
            return Ok(key);
        }
        Err(error) => return Err(error).context("Cannot unlock Straja credentials. Unlock your login keychain; on Ubuntu a Secret Service (for example GNOME Keyring) is required."),
    };
    ensure!(bytes.len() == 32, "Invalid Straja keychain key");
    Ok(Zeroizing::new(bytes))
}

pub fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    let key = key(true)?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow::anyhow!("Invalid encryption key"))?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), bytes)
        .map_err(|_| anyhow::anyhow!("Credential encryption failed"))?;
    Ok([PREFIX, &nonce, &encrypted].concat())
}
pub fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        bytes.starts_with(PREFIX) && bytes.len() >= PREFIX.len() + 12 + 16,
        "Unsupported saved credential format"
    );
    let key = key(false)?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow::anyhow!("Invalid encryption key"))?;
    let nonce = &bytes[PREFIX.len()..PREFIX.len() + 12];
    cipher
        .decrypt(Nonce::from_slice(nonce), &bytes[PREFIX.len() + 12..])
        .map_err(|_| anyhow::anyhow!("Saved credentials failed authentication"))
}
