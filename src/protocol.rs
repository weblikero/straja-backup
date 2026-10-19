use crate::model::*;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    State,
    Status,
    Shutdown,
    PrepareUpdate(String),
    CancelUpdate(String),
    ShutdownForUpdate(String),
    Add(NewPlan),
    Delete(String),
    Update {
        id: String,
        changes: PlanChanges,
    },
    Backup(String),
    Snapshots(String),
    Restore {
        plan: String,
        snapshot: String,
        parent: PathBuf,
    },
    Check(String),
    Toggle(String),
    TestRemote {
        destination: crate::remote::Destination,
        credentials: Option<crate::remote::Credentials>,
        plan: Option<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    UpdateReady {
        agent_executable: PathBuf,
        data_dir: PathBuf,
        service_mode: bool,
    },
    State(State),
    Status(AgentStatus),
    Snapshots(Vec<Snapshot>),
    Ok(String),
    Started(Run),
    Error(String),
}

pub async fn read<T: serde::de::DeserializeOwned>(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<T> {
    let size = stream.read_u32_le().await?;
    if size > 16 * 1024 * 1024 {
        bail!("Mesaj IPC prea mare.");
    }
    let mut bytes = zeroize::Zeroizing::new(vec![0; size as usize]);
    stream.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
pub async fn write<T: Serialize>(stream: &mut (impl AsyncWrite + Unpin), value: &T) -> Result<()> {
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(value)?);
    if bytes.len() > 16 * 1024 * 1024 {
        bail!("Mesaj IPC prea mare.");
    }
    stream.write_u32_le(bytes.len().try_into()?).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}
pub async fn call(request: Request) -> Result<Response> {
    tokio::time::timeout(std::time::Duration::from_secs(60), async {
        let name = crate::security::pipe_name()?;
        let mut attempt = 0;
        let mut pipe = loop {
            match tokio::net::windows::named_pipe::ClientOptions::new().open(&name) {
                Ok(pipe) => break pipe,
                Err(e) if e.raw_os_error() == Some(231) && attempt < 50 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Err(e) => return Err(e).context(
                    "Agentul nu este conectat. Pornește-l din aplicație sau din Windows Services.",
                ),
            }
        };
        write(&mut pipe, &request).await?;
        read(&mut pipe).await
    })
    .await
    .context("Agentul nu a răspuns în 60 de secunde.")?
}
