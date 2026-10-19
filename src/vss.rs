//! File-level VSS snapshots. A dedicated thread owns COM; dropping the lease releases
//! only this backup's nonpersistent snapshot set, including cancellation/error paths.
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::mpsc,
};

const CAPACITY: usize = 1024;
pub const RIGHTS_ERROR: &str = "VSS necesită un agent pornit ca administrator, sub același cont Windows. Folosește butonul Activează agentul VSS sau serviciul Straja configurat cu acest cont.";

unsafe extern "C" {
    fn straja_vss_admin() -> i32;
    fn straja_vss_mounts(root: *const u16, result: *mut u16, capacity: u32) -> i32;
    fn straja_vss_volume(
        source: *const u16,
        root: *mut u16,
        volume: *mut u16,
        capacity: u32,
    ) -> i32;
    fn straja_vss_begin(
        volumes: *const *const u16,
        count: u32,
        devices: *mut u16,
        capacity: u32,
        output: *mut *mut std::ffi::c_void,
    ) -> i32;
    fn straja_vss_end(handle: *mut std::ffi::c_void) -> i32;
}

pub fn available() -> bool {
    unsafe { straja_vss_admin() != 0 }
}
pub fn require_privileges() -> Result<()> {
    ensure!(available(), RIGHTS_ERROR);
    Ok(())
}

fn checked(code: i32) -> Result<()> {
    if code < 0 {
        bail!("Windows VSS: HRESULT 0x{:08X}", code as u32);
    }
    Ok(())
}
fn wide(path: &Path) -> Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
    ensure!(!value.contains(&0), "VSS: invalid path");
    value.push(0);
    Ok(value)
}
fn decode(value: &[u16]) -> Result<String> {
    Ok(String::from_utf16(
        &value[..value
            .iter()
            .position(|c| *c == 0)
            .context("VSS: invalid device path")?],
    )?)
}

// Reject UNC/device paths and path traversal before asking Windows about the volume.
fn local_path(path: &Path) -> Result<PathBuf> {
    let value = path
        .to_str()
        .context("VSS: invalid Unicode path")?
        .replace('/', "\\");
    let value = value.strip_prefix(r"\\?\").unwrap_or(&value);
    let bytes = value.as_bytes();
    ensure!(
        bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\'
            && !value[3..].contains(':')
            && !value.split('\\').any(|p| p == ".." || p == "."),
        "VSS acceptă doar căi absolute pe discuri locale NTFS (de exemplu C:\\Date). Căile de rețea și volumele montate în foldere nu sunt acceptate."
    );
    Ok(PathBuf::from(value))
}

fn volumes(sources: &[PathBuf]) -> Result<BTreeMap<PathBuf, String>> {
    ensure!(
        !sources.is_empty() && sources.len() <= 100,
        "VSS: invalid source count"
    );
    let mut volumes = BTreeMap::new();
    for source in sources {
        let path = local_path(source)?;
        // An ancestor junction could escape the snapshotted volume. Never follow it.
        for ancestor in path.ancestors() {
            use std::os::windows::fs::MetadataExt;
            let metadata = std::fs::symlink_metadata(ancestor)
                .with_context(|| format!("VSS: {}", ancestor.display()))?;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "Legătură/junction/reparse point neacceptat: {}. Alege folderul real sau exclude legătura.",
                ancestor.display()
            );
        }
        let mut root = [0u16; CAPACITY];
        let mut volume = [0u16; CAPACITY];
        checked(unsafe {
            straja_vss_volume(
                wide(&path)?.as_ptr(),
                root.as_mut_ptr(),
                volume.as_mut_ptr(),
                CAPACITY as u32,
            )
        })
        .with_context(|| {
            format!(
                "VSS: sursa trebuie să fie pe un disc local NTFS: {}",
                source.display()
            )
        })?;
        volumes.insert(
            PathBuf::from(decode(&root)?.to_ascii_uppercase()),
            decode(&volume)?,
        );
    }
    Ok(volumes)
}

pub fn validate_sources(sources: &[PathBuf]) -> Result<()> {
    volumes(sources).map(|_| ())
}

pub fn validate_restic_mounts(plan: &crate::model::Plan) -> Result<()> {
    let excluded = crate::exclusions::PathMatcher::new(&plan.excluded_paths);
    let patterns = crate::plain::exclusions(&plan.excludes)?;
    for root in volumes(&plan.sources)?.keys() {
        let mut buffer = vec![0u16; 32768];
        checked(unsafe {
            straja_vss_mounts(
                wide(root)?.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        })?;
        for value in buffer.split(|c| *c == 0).take_while(|v| !v.is_empty()) {
            let mount = root.join(String::from_utf16(value)?);
            for source in &plan.sources {
                let included = crate::exclusions::PathMatcher::new(std::slice::from_ref(source));
                if !included.contains(&mount) || excluded.contains(&mount) {
                    continue;
                }
                let normalized_mount = mount.to_string_lossy().replace('\\', "/");
                let normalized_source = local_path(source)?.to_string_lossy().replace('\\', "/");
                let relative = normalized_mount
                    .get(normalized_source.trim_end_matches('/').len()..)
                    .unwrap_or("")
                    .trim_matches('/');
                if patterns.is_match(relative)
                    || patterns.is_match(mount.file_name().unwrap_or_default())
                    || patterns.is_match(normalized_mount.trim_end_matches('/'))
                {
                    continue;
                }
                bail!(
                    "Legătură/junction/reparse point neacceptat: {}. Alege folderul real sau exclude legătura.",
                    mount.display()
                );
            }
        }
    }
    Ok(())
}

pub struct SnapshotSet {
    roots: Vec<(PathBuf, PathBuf)>,
    release: Option<mpsc::Sender<()>>,
    finished: Option<tokio::sync::oneshot::Receiver<Result<()>>>,
}
impl SnapshotSet {
    pub async fn create(sources: Vec<PathBuf>) -> Result<Self> {
        require_privileges()?;
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("straja-vss".into())
            .spawn(move || {
                let setup = (|| -> Result<_> {
                    let volumes = volumes(&sources)?;
                    let names: Vec<Vec<u16>> = volumes
                        .values()
                        .map(|v| wide(Path::new(v)))
                        .collect::<Result<_>>()?;
                    let pointers: Vec<_> = names.iter().map(|v| v.as_ptr()).collect();
                    let mut devices = vec![0u16; CAPACITY * names.len()];
                    let mut handle = std::ptr::null_mut();
                    checked(unsafe {
                        straja_vss_begin(
                            pointers.as_ptr(),
                            pointers.len() as u32,
                            devices.as_mut_ptr(),
                            CAPACITY as u32,
                            &mut handle,
                        )
                    })?;
                    // Keep a same-thread guard even if decoding or channel delivery fails.
                    struct Guard(*mut std::ffi::c_void);
                    impl Drop for Guard {
                        fn drop(&mut self) {
                            if !self.0.is_null() {
                                unsafe {
                                    straja_vss_end(self.0);
                                }
                            }
                        }
                    }
                    let guard = Guard(handle);
                    let roots = volumes
                        .keys()
                        .zip(devices.chunks(CAPACITY))
                        .map(|(root, device)| {
                            Ok((
                                root.clone(),
                                PathBuf::from(format!("{}\\", decode(device)?)),
                            ))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Ok((guard, roots))
                })();
                match setup {
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                    Ok((mut guard, roots)) => {
                        if ready_tx.send(Ok(roots)).is_ok() {
                            let _ = release_rx.recv();
                        }
                        let result = checked(unsafe { straja_vss_end(guard.0) });
                        guard.0 = std::ptr::null_mut();
                        let _ = finished_tx.send(result);
                    }
                }
            })?;
        // If this future is cancelled, release_tx drops; the COM thread cleans up.
        let roots = ready_rx.await.context("VSS: snapshot worker stopped")??;
        Ok(Self {
            roots,
            release: Some(release_tx),
            finished: Some(finished_rx),
        })
    }
    pub fn resolve(&self, source: &Path) -> Result<PathBuf> {
        let path = local_path(source)?;
        let text = path.to_string_lossy();
        for (root, device) in &self.roots {
            let prefix = root.to_string_lossy();
            if text
                .get(..prefix.len())
                .is_some_and(|s| s.eq_ignore_ascii_case(&prefix))
            {
                return Ok(device.join(&text[prefix.len()..]));
            }
        }
        bail!("VSS: sursa nu aparține snapshotului: {}", source.display())
    }
    pub async fn close(mut self) -> Result<()> {
        self.release.take();
        self.finished
            .take()
            .context("VSS: missing worker")?
            .await
            .context("VSS: cleanup worker stopped")??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_is_local_bounded_and_case_insensitive() {
        let set = SnapshotSet {
            roots: vec![(
                PathBuf::from(r"C:\"),
                PathBuf::from(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy123\"),
            )],
            release: None,
            finished: None,
        };
        assert_eq!(
            set.resolve(Path::new("c:/Folder/Grüße.txt")).unwrap(),
            PathBuf::from(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy123\Folder\Grüße.txt")
        );
        for path in [
            r"D:\folder",
            r"\\server\share",
            r"C:\..\escape",
            r"C:relative",
            r"C:\file:stream",
            r"\\?\GLOBALROOT\Device\x",
        ] {
            assert!(set.resolve(Path::new(path)).is_err(), "{path}");
        }
    }
}
