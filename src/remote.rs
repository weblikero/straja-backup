//! Online destinations. Credentials are DPAPI protected by Store and passed only
//! to owned child processes, never in command arguments or plaintext config files.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    #[default]
    Local,
    Ftp,
    GoogleDrive,
    Dropbox,
    S3,
}
impl Provider {
    pub const ALL: [Self; 5] = [
        Self::Local,
        Self::Ftp,
        Self::GoogleDrive,
        Self::Dropbox,
        Self::S3,
    ];
    pub fn backend(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Ftp => "ftp",
            Self::GoogleDrive => "drive",
            Self::Dropbox => "dropbox",
            Self::S3 => "s3",
        }
    }
}
impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::i18n::tr(match self {
            Self::Local => "Disc local / rețea",
            Self::Ftp => "FTP / FTPS",
            Self::GoogleDrive => "Google Drive",
            Self::Dropbox => "Dropbox",
            Self::S3 => "Amazon S3",
        }))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Destination {
    pub provider: Provider,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub bucket: String,
    pub region: String,
    pub path: String,
    pub ftp_tls: bool,
    pub bandwidth_kib: u32,
}
impl Default for Destination {
    fn default() -> Self {
        Self {
            provider: Provider::Local,
            host: String::new(),
            port: 21,
            user: String::new(),
            bucket: String::new(),
            region: "eu-central-1".into(),
            path: "StrajaBackup".into(),
            ftp_tls: true,
            bandwidth_kib: 10240,
        }
    }
}
impl Destination {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.provider != Provider::Local,
            "Alege o destinație online."
        );
        for value in [
            &self.host,
            &self.user,
            &self.bucket,
            &self.region,
            &self.path,
        ] {
            ensure!(
                value.len() <= 1024 && !value.chars().any(char::is_control),
                "Configurația destinației online nu este validă."
            );
        }
        ensure!(
            !self.path.is_empty()
                && self
                    .path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != "..")
                && !self.path.contains(['\\', ':']),
            "Alege un subfolder online relativ, fără .. sau caractere speciale."
        );
        if self.provider == Provider::Ftp {
            ensure!(
                !self.host.is_empty()
                    && !self.host.contains(['/', '\\', '@', ':', ' '])
                    && self.port > 0
                    && !self.user.is_empty(),
                "Completează serverul FTP, portul și utilizatorul."
            );
        }
        if self.provider == Provider::S3 {
            ensure!(
                (3..=63).contains(&self.bucket.len())
                    && self.bucket.bytes().all(|c| c.is_ascii_lowercase()
                        || c.is_ascii_digit()
                        || c == b'-'
                        || c == b'.')
                    && !self.region.is_empty()
                    && self
                        .region
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
                "Completează bucketul Amazon S3 și regiunea."
            );
        }
        ensure!(
            self.bandwidth_kib <= 1_048_576,
            "Limita de transfer este prea mare."
        );
        Ok(())
    }
    pub fn root(&self, id: &str) -> Result<String> {
        self.validate()?;
        uuid::Uuid::parse_str(id).context("Identificator de plan nevalid")?;
        Ok(format!(
            "straja:{}{}/plan-{id}",
            if self.provider == Provider::S3 {
                format!("{}/", self.bucket)
            } else {
                String::new()
            },
            self.path
        ))
    }
    pub fn label(&self) -> String {
        format!(
            "{} · {}{}",
            self.provider,
            if self.provider == Provider::S3 {
                format!("{}/", self.bucket)
            } else if self.provider == Provider::Ftp {
                format!("{}/", self.host)
            } else {
                String::new()
            },
            self.path
        )
    }
    pub fn same_location(&self, other: &Self) -> bool {
        let mut a = self.clone();
        let mut b = other.clone();
        a.bandwidth_kib = 0;
        b.bandwidth_kib = 0;
        a == b
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Credentials {
    pub password: String,
    pub access_key: String,
    pub secret_key: String,
    pub token: String,
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credentials([redacted])")
    }
}
impl Drop for Credentials {
    fn drop(&mut self) {
        self.password.zeroize();
        self.access_key.zeroize();
        self.secret_key.zeroize();
        self.token.zeroize();
    }
}
impl Credentials {
    pub fn validate(&self, p: Provider) -> Result<()> {
        ensure!(
            [
                &self.password,
                &self.access_key,
                &self.secret_key,
                &self.token
            ]
            .iter()
            .all(|s| s.len() < 65536 && !s.contains(['\0', '\r', '\n'])),
            "Datele de autentificare nu sunt valide."
        );
        match p {
            Provider::Ftp => ensure!(!self.password.is_empty(), "Introdu parola FTP."),
            Provider::S3 => ensure!(
                !self.access_key.is_empty() && !self.secret_key.is_empty(),
                "Introdu Access Key ID și Secret Access Key."
            ),
            Provider::GoogleDrive | Provider::Dropbox => {
                let v: serde_json::Value = serde_json::from_str(&self.token)
                    .context("Conectează contul online din nou.")?;
                ensure!(
                    v.get("refresh_token")
                        .and_then(|v| v.as_str())
                        .is_some_and(|s| !s.is_empty()),
                    "Conectează contul online din nou."
                );
            }
            Provider::Local => anyhow::bail!("Alege o destinație online."),
        };
        Ok(())
    }
}
pub fn bundled() -> Result<PathBuf> {
    Ok(std::env::current_exe()?
        .parent()
        .context("Folder executabil inexistent")?
        .join(crate::platform::executable("rclone")))
}
fn clean(command: &mut Command) {
    for (key, _) in std::env::vars_os() {
        let k = key.to_string_lossy();
        if k.starts_with("RCLONE_") || k.starts_with("AWS_") {
            command.env_remove(key);
        }
    }
    command
        .env("RCLONE_CONFIG", crate::platform::NULL_DEVICE)
        .env("RCLONE_TRANSFERS", "2")
        .env("RCLONE_CHECKERS", "2")
        .env("RCLONE_BUFFER_SIZE", "1Mi")
        .env("RCLONE_MULTI_THREAD_STREAMS", "0")
        .env("RCLONE_S3_UPLOAD_CONCURRENCY", "2")
        .env("RCLONE_CONTIMEOUT", "15s")
        .env("RCLONE_TIMEOUT", "60s")
        .env("RCLONE_RETRIES", "2")
        .env("RCLONE_LOW_LEVEL_RETRIES", "3");
    crate::resources::restic_budget(command);
}
async fn bounded(mut reader: impl AsyncRead + Unpin) -> Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    (&mut reader)
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(bytes.len() <= 1024 * 1024, "Răspuns online prea mare.");
    Ok(bytes)
}
async fn run(
    mut cmd: Command,
    input: Option<&str>,
    timeout: Duration,
) -> Result<(i32, Zeroizing<Vec<u8>>)> {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    tokio::time::timeout(timeout, async {
        let mut child = cmd
            .spawn()
            .context("rclone.exe lipsește sau nu poate porni.")?;
        let mut stdin = child.stdin.take().context("Lipsește stdin")?;
        if let Some(input) = input {
            stdin.write_all(input.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
        }
        drop(stdin);
        let (out, _err, status) = tokio::try_join!(
            bounded(child.stdout.take().unwrap()),
            bounded(child.stderr.take().unwrap()),
            async { Ok::<_, anyhow::Error>(child.wait().await?) }
        )?;
        Ok((status.code().unwrap_or(-1), out))
    })
    .await
    .context("Conexiunea online a expirat. Încearcă din nou.")?
}
pub async fn configure(
    cmd: &mut Command,
    binary: &Path,
    d: &Destination,
    c: &Credentials,
) -> Result<()> {
    d.validate()?;
    c.validate(d.provider)?;
    ensure!(
        binary.is_file(),
        "rclone.exe lipsește. Reinstalează Straja."
    );
    clean(cmd);
    cmd.env("RCLONE_CONFIG_STRAJA_TYPE", d.provider.backend());
    if d.bandwidth_kib > 0 {
        cmd.env("RCLONE_BWLIMIT", format!("{}k", d.bandwidth_kib));
    }
    match d.provider {
        Provider::Ftp => {
            let mut obscure = Command::new(binary);
            clean(&mut obscure);
            obscure.args(["obscure", "-"]);
            let (code, out) = run(obscure, Some(&c.password), Duration::from_secs(15)).await?;
            ensure!(code == 0, "Pregătirea autentificării FTP a eșuat.");
            let password = Zeroizing::new(String::from_utf8(out.to_vec())?);
            cmd.env("RCLONE_CONFIG_STRAJA_HOST", &d.host)
                .env("RCLONE_CONFIG_STRAJA_PORT", d.port.to_string())
                .env("RCLONE_CONFIG_STRAJA_USER", &d.user)
                .env("RCLONE_CONFIG_STRAJA_PASS", password.trim())
                .env(
                    "RCLONE_CONFIG_STRAJA_EXPLICIT_TLS",
                    if d.ftp_tls { "true" } else { "false" },
                )
                .env("RCLONE_CONFIG_STRAJA_CONCURRENCY", "2");
        }
        Provider::S3 => {
            cmd.env("RCLONE_CONFIG_STRAJA_PROVIDER", "AWS")
                .env("RCLONE_CONFIG_STRAJA_ENV_AUTH", "false")
                .env("RCLONE_CONFIG_STRAJA_REGION", &d.region)
                .env("RCLONE_CONFIG_STRAJA_ACCESS_KEY_ID", &c.access_key)
                .env("RCLONE_CONFIG_STRAJA_SECRET_ACCESS_KEY", &c.secret_key)
                .env("RCLONE_CONFIG_STRAJA_NO_CHECK_BUCKET", "true");
        }
        Provider::GoogleDrive | Provider::Dropbox => {
            cmd.env("RCLONE_CONFIG_STRAJA_TOKEN", &c.token);
        }
        Provider::Local => unreachable!(),
    };
    Ok(())
}
pub async fn list(
    binary: &Path,
    d: &Destination,
    c: &Credentials,
    id: &str,
) -> Result<Vec<String>> {
    let mut cmd = Command::new(binary);
    configure(&mut cmd, binary, d, c).await?;
    cmd.args([
        "lsf",
        "--max-depth",
        "1",
        "--format",
        "p",
        "--",
        &d.root(id)?,
    ]);
    let (code, out) = run(cmd, None, Duration::from_secs(45)).await?;
    if code == 3 {
        return Ok(vec![]);
    }
    ensure!(
        code == 0,
        "Conexiunea online a eșuat. Verifică autentificarea, permisiunile și rețeaua."
    );
    Ok(std::str::from_utf8(&out)?
        .lines()
        .map(str::to_owned)
        .collect())
}
pub async fn authorize(provider: Provider) -> Result<Credentials> {
    ensure!(
        matches!(provider, Provider::GoogleDrive | Provider::Dropbox),
        "Acest serviciu nu folosește autentificare în browser."
    );
    let mut cmd = Command::new(bundled()?);
    clean(&mut cmd);
    cmd.args(["authorize", provider.backend()]);
    let (code, out) = run(cmd, None, Duration::from_secs(180)).await?;
    ensure!(code == 0, "Autentificarea a fost anulată sau a eșuat.");
    parse_authorization(std::str::from_utf8(&out)?, provider)
}
fn parse_authorization(output: &str, provider: Provider) -> Result<Credentials> {
    for (start, _) in output.match_indices('{') {
        if let Some(Ok(value)) = serde_json::Deserializer::from_str(&output[start..])
            .into_iter::<serde_json::Value>()
            .next()
        {
            let c = Credentials {
                token: serde_json::to_string(&value)?,
                password: String::new(),
                access_key: String::new(),
                secret_key: String::new(),
            };
            if c.validate(provider).is_ok() {
                return Ok(c);
            }
        }
    }
    anyhow::bail!("Conectează contul online din nou.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destination_validation_and_namespaces() {
        let mut d = Destination {
            provider: Provider::GoogleDrive,
            ..Default::default()
        };
        let id = uuid::Uuid::new_v4().to_string();
        assert!(d.root(&id).unwrap().ends_with(&format!("/plan-{id}")));
        for path in [
            "",
            "/",
            "../data",
            "data/../other",
            "x:y",
            "a\\b",
            "a//b",
            "a\n",
        ] {
            d.path = path.into();
            assert!(d.validate().is_err(), "{path}");
        }
        d.path = "My backups/Grüße".into();
        assert!(d.validate().is_ok());
        assert!(d.root("../bad").is_err());
        d.provider = Provider::S3;
        assert!(d.validate().is_err());
        d.bucket = "my-bucket".into();
        assert!(d.validate().is_ok());
        d.provider = Provider::Ftp;
        d.host = "ftp.example.org".into();
        d.user = "user".into();
        assert!(d.validate().is_ok());
        d.host = "ftp://user:secret@example.org".into();
        assert!(d.validate().is_err());
    }
    #[test]
    fn oauth_output_is_parsed_without_logging_tokens() {
        let text = "Paste token below:\n{\n\"access_token\":\"fake-access\",\n\"refresh_token\":\"fake-refresh\",\n\"expiry\":\"2026-10-19T00:00:00Z\"\n}\nEnd token\n";
        for provider in [Provider::GoogleDrive, Provider::Dropbox] {
            let c = parse_authorization(text, provider).unwrap();
            assert!(c.token.contains("fake-refresh"));
            assert_eq!(format!("{c:?}"), "Credentials([redacted])");
            assert!(parse_authorization("{\"access_token\":\"no-refresh\"}", provider).is_err());
        }
    }
    #[tokio::test]
    async fn cloud_credentials_use_child_environment_not_command_arguments() -> Result<()> {
        let binary = std::env::current_exe()?;
        for provider in [Provider::GoogleDrive, Provider::Dropbox, Provider::S3] {
            let d = Destination {
                provider,
                bucket: "test-bucket".into(),
                ..Default::default()
            };
            let c = Credentials {
                password: String::new(),
                access_key: "fake-key".into(),
                secret_key: "fake-secret".into(),
                token: r#"{"access_token":"fake-access","refresh_token":"fake-refresh"}"#.into(),
            };
            let mut cmd = Command::new("unused");
            configure(&mut cmd, &binary, &d, &c).await?;
            assert_eq!(cmd.as_std().get_args().count(), 0);
            let env: std::collections::HashMap<_, _> = cmd
                .as_std()
                .get_envs()
                .filter_map(|(k, v)| {
                    v.map(|v| {
                        (
                            k.to_string_lossy().into_owned(),
                            v.to_string_lossy().into_owned(),
                        )
                    })
                })
                .collect();
            assert_eq!(env["RCLONE_CONFIG"], crate::platform::NULL_DEVICE);
            assert_eq!(env["RCLONE_CONFIG_STRAJA_TYPE"], provider.backend());
            assert_eq!(env["RCLONE_TRANSFERS"], "2");
            assert_eq!(env["RCLONE_BWLIMIT"], "10240k");
            if provider == Provider::S3 {
                assert_eq!(env["RCLONE_CONFIG_STRAJA_SECRET_ACCESS_KEY"], "fake-secret");
            } else {
                assert_eq!(env["RCLONE_CONFIG_STRAJA_TOKEN"], c.token);
            }
        }
        Ok(())
    }
}
