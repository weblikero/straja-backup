//! VSS is a Windows facility. Never silently claim a consistent snapshot elsewhere.
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
pub const RIGHTS_ERROR: &str = "Windows VSS is not available on this operating system.";
pub fn available() -> bool {
    false
}
pub fn require_privileges() -> Result<()> {
    bail!(RIGHTS_ERROR)
}
pub fn validate_sources(_: &[PathBuf]) -> Result<()> {
    bail!(RIGHTS_ERROR)
}
pub fn validate_restic_mounts(_: &crate::model::Plan) -> Result<()> {
    bail!(RIGHTS_ERROR)
}
pub struct SnapshotSet;
impl SnapshotSet {
    pub async fn create(_: Vec<PathBuf>) -> Result<Self> {
        bail!(RIGHTS_ERROR)
    }
    pub fn resolve(&self, _: &Path) -> Result<PathBuf> {
        bail!(RIGHTS_ERROR)
    }
    pub async fn close(self) -> Result<()> {
        Ok(())
    }
}
