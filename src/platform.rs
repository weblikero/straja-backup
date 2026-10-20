//! Small platform boundaries shared by the GUI, worker and packaging.
use std::path::Path;

pub const fn executable(name: &'static str) -> &'static str {
    match name.as_bytes() {
        b"restic" if cfg!(windows) => "restic.exe",
        b"rclone" if cfg!(windows) => "rclone.exe",
        b"straja-agent" if cfg!(windows) => "straja-agent.exe",
        _ => name,
    }
}
pub const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub const FILE_MANAGER: &str = if cfg!(windows) {
    "explorer.exe"
} else if cfg!(target_os = "macos") {
    "open"
} else {
    "xdg-open"
};

pub fn open_read(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x00200000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Do not follow a replaced leaf symlink or block indefinitely on a FIFO.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    options.open(path)
}

pub fn path_key(path: &Path) -> String {
    let value = path.to_string_lossy();
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value.into_owned()
    }
}
