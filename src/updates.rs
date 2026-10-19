//! GitHub release discovery and bounded, verified installer downloads.
use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

pub const REPOSITORY: &str = "weblikero/straja-backup";
const API_URL: &str = "https://api.github.com/repos/weblikero/straja-backup/releases/latest";
const MAX_RESPONSE: usize = 512 * 1024;
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Update {
    pub version: String,
    pub notes: String,
    pub page_url: String,
    pub installer: Option<Installer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installer {
    pub url: String,
    pub size: u64,
    pub sha256: String,
}
pub const MAX_INSTALLER: u64 = 256 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedUpdate {
    pub update: Update,
    pub path: std::path::PathBuf,
}
#[derive(Debug, Default)]
pub struct DownloadProgress {
    pub received: std::sync::atomic::AtomicU64,
    pub cancelled: std::sync::atomic::AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckError {
    Network,
    RateLimited,
    InvalidRelease,
}
impl CheckError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Network => {
                "Verificarea actualizărilor nu a reușit. Verifică conexiunea și încearcă din nou."
            }
            Self::RateLimited => "GitHub a limitat verificările. Încearcă din nou peste o oră.",
            Self::InvalidRelease => {
                "Versiunea publicată nu are un pachet Straja valid pentru Windows x64."
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    state: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    assets: Vec<Asset>,
}
#[derive(Deserialize, Serialize)]
struct Cache {
    repository: String,
    checked_at: DateTime<Utc>,
    release: Option<Release>,
}

fn evaluate(release: Option<&Release>, installed: &str) -> Result<Option<Update>, CheckError> {
    let Some(release) = release else {
        return Ok(None);
    };
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let tag = &release.tag_name;
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|_| CheckError::InvalidRelease)?;
    let installed = Version::parse(installed).map_err(|_| CheckError::InvalidRelease)?;
    if !version.pre.is_empty() || version.cmp_precedence(&installed).is_le() {
        return Ok(None);
    }
    // Only stable, simple tags; construct URLs ourselves, never open API-supplied HTML URLs.
    if !version.build.is_empty() {
        return Err(CheckError::InvalidRelease);
    }
    let base = format!("https://github.com/{REPOSITORY}/releases");
    let installer = format!("StrajaBackup-{version}-windows-x64-setup.exe");
    let portable = format!("StrajaBackup-{version}-windows-x64-portable.zip");
    if !release.assets.iter().any(|asset| {
        (asset.name == installer || asset.name == portable)
            && asset.state == "uploaded"
            && asset.size > 0
            && asset.browser_download_url == format!("{base}/download/{tag}/{}", asset.name)
    }) {
        return Err(CheckError::InvalidRelease);
    }
    Ok(Some(Update {
        installer: release
            .assets
            .iter()
            .find(|a| {
                a.name == installer
                    && a.state == "uploaded"
                    && a.size > 0
                    && a.size <= MAX_INSTALLER
                    && a.browser_download_url == format!("{base}/download/{tag}/{installer}")
            })
            .and_then(|a| {
                let hash = a.digest.as_deref()?.strip_prefix("sha256:")?;
                valid_hash(hash).then(|| Installer {
                    url: a.browser_download_url.clone(),
                    size: a.size,
                    sha256: hash.to_ascii_lowercase(),
                })
            }),
        version: version.to_string(),
        notes: release
            .body
            .as_deref()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
            .take(4000)
            .collect(),
        page_url: format!("{base}/tag/{tag}"),
    }))
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn download_host_allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|p| p == 443)
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

pub fn update_directory() -> anyhow::Result<std::path::PathBuf> {
    Ok(crate::security::data_dir()?.join("updates"))
}

/// No whole-installer buffers. Only a complete, size/hash-verified file gets an .exe name.
async fn download_response(
    mut response: reqwest::Response,
    target: &Path,
    asset: &Installer,
    progress: &DownloadProgress,
) -> anyhow::Result<()> {
    use anyhow::ensure;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::Ordering;
    use tokio::io::AsyncWriteExt;
    ensure!(
        response.status() == reqwest::StatusCode::OK,
        "Download HTTP {}",
        response.status()
    );
    ensure!(
        asset.size > 0 && asset.size <= MAX_INSTALLER && valid_hash(&asset.sha256),
        "Invalid installer metadata"
    );
    if let Some(length) = response.content_length() {
        ensure!(length == asset.size, "Installer size changed");
    }
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .await?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            !progress.cancelled.load(Ordering::Relaxed),
            "Download cancelled"
        );
        size += chunk.len() as u64;
        ensure!(
            size <= asset.size && size <= MAX_INSTALLER,
            "Installer exceeds declared size"
        );
        hash.update(&chunk);
        file.write_all(&chunk).await?;
        progress.received.store(size, Ordering::Relaxed);
    }
    ensure!(
        !progress.cancelled.load(Ordering::Relaxed),
        "Download cancelled"
    );
    ensure!(
        size == asset.size && format!("{:x}", hash.finalize()) == asset.sha256,
        "Installer SHA-256 verification failed"
    );
    file.sync_all().await?;
    Ok(())
}

pub async fn download(
    update: Update,
    progress: std::sync::Arc<DownloadProgress>,
) -> anyhow::Result<PreparedUpdate> {
    use anyhow::{Context, ensure};
    let fresh = check(true)
        .await
        .map_err(|e| anyhow::anyhow!(e.message()))?
        .context("Release no longer available")?;
    ensure!(
        fresh.version == update.version && fresh.installer == update.installer,
        "Release changed. Check for updates again."
    );
    let asset = update
        .installer
        .as_ref()
        .context("No verified Windows installer is published")?;
    let client = reqwest::Client::builder()
        .https_only(true)
        .user_agent(concat!("StrajaBackup/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(30 * 60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !download_host_allowed(attempt.url()) {
                attempt.error("Untrusted download redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()?;
    ensure!(
        download_host_allowed(&reqwest::Url::parse(&asset.url)?),
        "Untrusted download URL"
    );
    let root = update_directory()?;
    tokio::fs::create_dir_all(&root).await?;
    let folder = root.join(uuid::Uuid::new_v4().to_string());
    tokio::fs::create_dir(&folder).await?;
    let partial = folder.join("installer.part");
    let result = async {
        let response = client.get(&asset.url).send().await?;
        download_response(response, &partial, asset, &progress).await
    }
    .await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&partial).await;
        let _ = tokio::fs::remove_dir(&folder).await;
        return Err(error);
    }
    let path = folder.join("installer.exe");
    tokio::fs::rename(partial, &path).await?;
    Ok(PreparedUpdate { update, path })
}

fn cached(path: &Path, now: DateTime<Utc>) -> Option<Option<Release>> {
    if std::fs::metadata(path).ok()?.len() > MAX_RESPONSE as u64 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > MAX_RESPONSE {
        return None;
    }
    let cache: Cache = serde_json::from_slice(&bytes).ok()?;
    let age = now.signed_duration_since(cache.checked_at).num_seconds();
    (cache.repository == REPOSITORY && age >= 0 && age < CHECK_INTERVAL.as_secs() as i64)
        .then_some(cache.release)
}

async fn fetch(client: &reqwest::Client, url: &str) -> Result<Option<Release>, CheckError> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| CheckError::Network)?;
    match response.status().as_u16() {
        404 => return Ok(None), // No public stable release yet.
        403 | 429 => return Err(CheckError::RateLimited),
        200 => {}
        _ => return Err(CheckError::Network),
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| CheckError::Network)? {
        if body.len() + chunk.len() > MAX_RESPONSE {
            return Err(CheckError::InvalidRelease);
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|_| CheckError::InvalidRelease)
}

pub async fn check(manual: bool) -> Result<Option<Update>, CheckError> {
    let path = crate::security::data_dir()
        .ok()
        .map(|p| p.join("update-cache.json"));
    if !manual && let Some(release) = path.as_deref().and_then(|p| cached(p, Utc::now())) {
        return evaluate(release.as_ref(), env!("CARGO_PKG_VERSION"));
    }
    let client = reqwest::Client::builder()
        .user_agent(concat!("StrajaBackup/", env!("CARGO_PKG_VERSION")))
        .default_headers(
            [
                (
                    reqwest::header::ACCEPT,
                    "application/vnd.github+json".parse().unwrap(),
                ),
                (
                    reqwest::header::HeaderName::from_static("x-github-api-version"),
                    "2022-11-28".parse().unwrap(),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| CheckError::Network)?;
    let release = fetch(&client, API_URL).await?;
    let result = evaluate(release.as_ref(), env!("CARGO_PKG_VERSION"))?;
    // A cache failure must not prevent checking, or affect plans and backups.
    if let Some(path) = path
        && let Some(parent) = path.parent()
    {
        let cache = Cache {
            repository: REPOSITORY.into(),
            checked_at: Utc::now(),
            release,
        };
        if let Ok(bytes) = serde_json::to_vec(&cache) {
            let _ = std::fs::create_dir_all(parent);
            let _ = std::fs::write(path, bytes);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_install_requires_bounded_installer_and_sha256() {
        let mut r = release("v1.0.0");
        assert!(
            evaluate(Some(&r), "0.5.0")
                .unwrap()
                .unwrap()
                .installer
                .is_some()
        );
        for digest in [
            None,
            Some("sha256:bad".into()),
            Some(format!("sha512:{}", "a".repeat(64))),
        ] {
            r.assets[0].digest = digest;
            assert!(
                evaluate(Some(&r), "0.5.0")
                    .unwrap()
                    .unwrap()
                    .installer
                    .is_none()
            );
        }
        r.assets[0].digest = Some(format!("sha256:{}", "A".repeat(64)));
        assert_eq!(
            evaluate(Some(&r), "0.5.0")
                .unwrap()
                .unwrap()
                .installer
                .unwrap()
                .sha256,
            "a".repeat(64)
        );
        r.assets[0].size = MAX_INSTALLER + 1;
        assert!(
            evaluate(Some(&r), "0.5.0")
                .unwrap()
                .unwrap()
                .installer
                .is_none()
        );
    }
    #[test]
    fn redirects_cannot_escape_github_https_hosts() {
        for url in [
            "https://github.com/a",
            "https://release-assets.githubusercontent.com/a?token=123",
            "https://objects.githubusercontent.com/a",
        ] {
            assert!(download_host_allowed(&url.parse().unwrap()));
        }
        for url in [
            "http://github.com/a",
            "https://github.com.evil.test/a",
            "https://user@github.com/a",
            "https://github.com:444/a",
            "https://127.0.0.1/a",
            "https://evil.test/github.com",
        ] {
            assert!(!download_host_allowed(&url.parse().unwrap()));
        }
    }
    #[tokio::test]
    async fn streamed_installer_requires_complete_matching_content() {
        use sha2::{Digest, Sha256};
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        // Close-delimited bodies exercise the byte cap even without Content-Length.
        for (body, expected_size, correct_hash, cancelled, success) in [
            ("installer", 9, true, false, true),
            ("installer", 9, false, false, false),
            ("short", 9, true, false, false),
            ("installer-too-big", 9, true, false, false),
            ("installer", 9, true, true, false),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("installer.part");
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 4096];
                let _ = socket.read(&mut buf).await;
                socket
                    .write_all(
                        format!("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{body}").as_bytes(),
                    )
                    .await
                    .unwrap();
            });
            let response = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap()
                .get(&url)
                .send()
                .await
                .unwrap();
            let progress = DownloadProgress::default();
            progress
                .cancelled
                .store(cancelled, std::sync::atomic::Ordering::Relaxed);
            let asset = Installer {
                url,
                size: expected_size,
                sha256: if correct_hash {
                    format!("{:x}", Sha256::digest(b"installer"))
                } else {
                    "0".repeat(64)
                },
            };
            assert_eq!(
                download_response(response, &target, &asset, &progress)
                    .await
                    .is_ok(),
                success
            );
            assert!(!target.with_extension("exe").exists());
            if success {
                assert_eq!(std::fs::read(&target).unwrap(), b"installer");
                assert_eq!(
                    progress.received.load(std::sync::atomic::Ordering::Relaxed),
                    9
                );
            }
            server.await.unwrap();
        }
    }
    fn release(tag: &str) -> Release {
        let version = tag.trim_start_matches('v');
        let name = format!("StrajaBackup-{version}-windows-x64-setup.exe");
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            body: Some("Changes".into()),
            assets: vec![Asset {
                browser_download_url: format!(
                    "https://github.com/{REPOSITORY}/releases/download/{tag}/{name}"
                ),
                name,
                state: "uploaded".into(),
                size: 1234,
                digest: Some(format!("sha256:{}", "a".repeat(64))),
            }],
        }
    }
    #[test]
    fn numeric_versions_and_no_downgrades() {
        let r = release("v0.10.0");
        assert_eq!(
            evaluate(Some(&r), "0.9.0").unwrap().unwrap().version,
            "0.10.0"
        );
        assert!(evaluate(Some(&r), "0.10.0").unwrap().is_none());
        assert!(evaluate(Some(&r), "1.0.0").unwrap().is_none());
        assert!(evaluate(None, "0.4.0").unwrap().is_none());
    }
    #[test]
    fn ignores_drafts_and_prereleases() {
        let mut r = release("v1.0.0");
        r.draft = true;
        assert!(evaluate(Some(&r), "0.4.0").unwrap().is_none());
        r.draft = false;
        r.prerelease = true;
        assert!(evaluate(Some(&r), "0.4.0").unwrap().is_none());
        assert!(
            evaluate(Some(&release("v1.0.0-rc.1")), "0.4.0")
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn rejects_wrong_product_host_and_missing_assets() {
        for change in 0..5 {
            let mut r = release("v1.0.0");
            match change {
                0 => r.assets[0].name = "OtherApp.exe".into(),
                1 => r.assets[0].browser_download_url = "https://example.org/setup.exe".into(),
                2 => r.assets[0].size = 0,
                3 => r.assets[0].state = "new".into(),
                _ => r.assets.clear(),
            }
            assert_eq!(evaluate(Some(&r), "0.4.0"), Err(CheckError::InvalidRelease));
        }
    }
    #[test]
    fn portable_only_release_and_bounded_notes() {
        let mut r = release("v1.0.0");
        r.assets[0].name = "StrajaBackup-1.0.0-windows-x64-portable.zip".into();
        r.assets[0].browser_download_url = format!(
            "https://github.com/{REPOSITORY}/releases/download/v1.0.0/{}",
            r.assets[0].name
        );
        r.body = Some("é\0".repeat(5000));
        let update = evaluate(Some(&r), "0.4.0").unwrap().unwrap();
        assert_eq!(update.notes.chars().count(), 4000);
        assert!(!update.notes.contains('\0'));
        assert_eq!(
            update.page_url,
            format!("https://github.com/{REPOSITORY}/releases/tag/v1.0.0")
        );
    }
    #[test]
    fn malformed_and_url_like_tags_are_rejected() {
        for tag in ["latest", "v1.0.0/../../other", "v1.0.0+build"] {
            assert_eq!(
                evaluate(Some(&release(tag)), "0.4.0"),
                Err(CheckError::InvalidRelease)
            );
        }
    }
    #[test]
    fn cache_expires_and_is_bound_to_product() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.json");
        let now = Utc::now();
        for (repo, age, valid) in [
            (REPOSITORY, 60, true),
            (REPOSITORY, 86400, false),
            (REPOSITORY, -60, false),
            ("other/app", 0, false),
        ] {
            let cache = Cache {
                repository: repo.into(),
                checked_at: now - chrono::Duration::seconds(age),
                release: Some(release("v1.0.0")),
            };
            std::fs::write(&path, serde_json::to_vec(&cache).unwrap()).unwrap();
            assert_eq!(cached(&path, now).is_some(), valid);
        }
        std::fs::write(&path, "corrupt").unwrap();
        assert!(cached(&path, now).is_none());
    }
    #[tokio::test]
    async fn handles_http_errors_and_invalid_responses() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        for (status, body, expected) in [
            (404, "", Ok(None)),
            (429, "", Err(CheckError::RateLimited)),
            (500, "", Err(CheckError::Network)),
            (200, "bad json", Err(CheckError::InvalidRelease)),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 4096];
                let _ = socket.read(&mut buf).await.unwrap();
                socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            });
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap();
            let result = fetch(&client, &url)
                .await
                .and_then(|r| evaluate(r.as_ref(), "0.4.0"));
            assert_eq!(result, expected);
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn rejects_oversized_http_body() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 4096];
            let _ = socket.read(&mut buf).await;
            let body = vec![b' '; MAX_RESPONSE + 1];
            let _ = socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await;
            let _ = socket.write_all(&body).await;
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        assert!(matches!(
            fetch(&client, &url).await,
            Err(CheckError::InvalidRelease)
        ));
        server.await.unwrap();
    }

    #[tokio::test]
    #[ignore = "Read-only live GitHub API smoke test; requires internet"]
    async fn live_public_repository_and_release_api() {
        let client = reqwest::Client::builder()
            .https_only(true)
            .user_agent("StrajaBackup-test")
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        let response = client
            .get(format!("https://api.github.com/repos/{REPOSITORY}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let release = fetch(&client, API_URL).await.unwrap();
        evaluate(release.as_ref(), env!("CARGO_PKG_VERSION")).unwrap();
        // Exercise discovery as an older installed client, without writing its cache.
        if let Ok(installed) = std::env::var("STRAJA_UPDATE_TEST_INSTALLED") {
            let update = evaluate(release.as_ref(), &installed)
                .unwrap()
                .expect("published release must be newer than the simulated client");
            assert!(
                update.installer.is_some(),
                "in-app installer must be available"
            );
            println!(
                "Client {installed} detects {} with a verified installer digest",
                update.version
            );
        }
    }
}
