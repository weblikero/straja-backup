//! Per-user Unix socket. A held file lock serializes stale-socket recovery.
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use std::{fs::File, path::PathBuf};
use tokio::net::{UnixListener, UnixStream};

pub fn verify_peer(stream: &UnixStream) -> Result<()> {
    ensure!(
        stream.peer_cred()?.uid() == unsafe { libc::geteuid() },
        "Rejected a connection from another account"
    );
    Ok(())
}
pub struct Listener {
    inner: UnixListener,
    path: PathBuf,
    _lock: File,
}
impl Listener {
    pub fn bind(name: &str) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let path = PathBuf::from(name);
        crate::unix_security::private_directory(path.parent().context("Invalid IPC path")?)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.with_extension("lock"))?;
        lock.try_lock_exclusive()
            .context("Another Straja agent is already running")?;
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            use std::os::unix::fs::FileTypeExt;
            ensure!(
                meta.file_type().is_socket(),
                "Refusing to replace a non-socket IPC path"
            );
            std::fs::remove_file(&path)?;
        }
        let inner = UnixListener::bind(&path)?;
        Ok(Self {
            inner,
            path,
            _lock: lock,
        })
    }
    pub async fn accept(&self) -> std::io::Result<UnixStream> {
        loop {
            let (stream, _) = self.inner.accept().await?;
            if verify_peer(&stream).is_ok() {
                return Ok(stream);
            }
        }
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
