//! Literal path selections, separate from user-entered wildcard rules.
use anyhow::{Result, ensure};
use std::path::{Component, Path, PathBuf};

pub fn normalized(path: &Path) -> String {
    let text = path
        .components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .replace('\\', "/");
    let text = if let Some(unc) = text.strip_prefix("//?/UNC/") {
        format!("//{unc}")
    } else {
        text.strip_prefix("//?/").unwrap_or(&text).to_owned()
    };
    text.trim_end_matches('/').to_lowercase()
}

fn under(path: &str, parent: &str) -> bool {
    path == parent || path.starts_with(&format!("{parent}/"))
}

pub struct PathMatcher(std::collections::HashSet<String>);
impl PathMatcher {
    pub fn new(paths: &[PathBuf]) -> Self {
        Self(paths.iter().map(|p| normalized(p)).collect())
    }
    pub fn contains(&self, candidate: &Path) -> bool {
        if self.0.is_empty() {
            return false;
        }
        let candidate = normalized(candidate);
        let mut parent = candidate.as_str();
        loop {
            if self.0.contains(parent) {
                return true;
            }
            let Some(index) = parent.rfind('/') else {
                return false;
            };
            parent = &parent[..index];
        }
    }
}

pub fn validate_paths(sources: &[PathBuf], paths: &[PathBuf]) -> Result<()> {
    ensure!(
        paths.len() <= 100,
        "Poți exclude cel mult 100 de fișiere sau foldere selectate."
    );
    for path in paths {
        let text = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Cale Unicode nevalidă."))?;
        ensure!(path.is_absolute() && !text.contains(['\0', '\r', '\n'])
            && !path.components().any(|part| matches!(part, Component::ParentDir)
                || matches!(part, Component::Normal(name) if name.to_string_lossy().contains(['*', '?']))),
            "Alege o cale completă, fără .. sau caractere wildcard, pentru excludere.");
        let excluded = normalized(path);
        ensure!(
            sources.iter().any(|source| {
                let source = normalized(source);
                excluded != source && under(&excluded, &source)
            }),
            "Excluderea {} trebuie să fie în interiorul unei surse. Alege mai întâi folderul sursă sau elimină excluderea rămasă fără sursă.",
            path.display()
        );
        ensure!(
            !sources
                .iter()
                .any(|source| under(&normalized(source), &excluded)),
            "Excluderea {} ar elimina o sursă întreagă. Elimină acea sursă din lista de backup.",
            path.display()
        );
        // Missing exclusions stay valid: a deleted folder can reappear on the next backup.
    }
    Ok(())
}

pub fn restic_pattern(path: &Path) -> String {
    // Go filepath.Match on Windows treats backslashes as separators, not escapes.
    // '[' needs a single-character class; ']' outside a class is already literal.
    normalized(path).replace('[', "[[]")
}
