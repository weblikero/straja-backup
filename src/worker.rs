use crate::{
    engine::Engine,
    model::*,
    protocol::{self, Request, Response},
    store::Store,
};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::watch, task::JoinSet};

#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

#[cfg(windows)]
fn create_pipe(first: bool, name: &str) -> Result<NamedPipeServer> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SECURITY_ATTRIBUTES,
        },
    };
    let sid = crate::security::user_sid()?;
    let sddl: Vec<u16> = format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})\0")
        .encode_utf16()
        .collect();
    unsafe {
        let mut descriptor = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor);
        Ok(result?)
    }
}

struct Worker {
    update_lease: Option<(String, std::time::Instant)>,
    shutdown_requested: bool,
    engine: Engine,
    service_mode: bool,
    tasks: JoinSet<()>,
}

impl Worker {
    fn update_reserved(&self) -> bool {
        self.update_lease
            .as_ref()
            .is_some_and(|(_, deadline)| *deadline > std::time::Instant::now())
    }
    fn valid_update_token(&self, token: &str) -> bool {
        self.update_reserved() && self.update_lease.as_ref().is_some_and(|(t, _)| t == token)
    }
    fn schedule_due(&mut self) -> Result<()> {
        let idle = self.engine.live.lock().unwrap().is_none();
        if !idle || self.update_reserved() {
            return Ok(());
        }
        if let Some(plan) = self
            .engine
            .store
            .plans()?
            .into_iter()
            .find(|p| p.enabled && p.next_due.is_some_and(|due| due <= Utc::now()))
        {
            self.start(&plan.id, "Backup", None)?;
        }
        Ok(())
    }
    fn state(&self) -> Result<State> {
        Ok(State {
            agent_version: env!("CARGO_PKG_VERSION").into(),
            plans: self.engine.store.plans()?,
            history: self.engine.store.history()?,
            active: self.engine.live.lock().unwrap().clone(),
            service_mode: self.service_mode,
            engine_available: self.engine.binary.is_file(),
            vss_available: crate::vss::available(),
        })
    }
    fn start(
        &mut self,
        id: &str,
        operation: &str,
        restore: Option<(String, PathBuf)>,
    ) -> Result<Response> {
        if self.engine.live.lock().unwrap().is_some() {
            bail!("O operație este deja în curs. Așteaptă finalizarea ei.");
        }
        let plan = self.engine.store.plan(id)?;
        let run = Run {
            id: uuid::Uuid::new_v4().to_string(),
            plan_id: plan.id.clone(),
            plan_name: plan.name.clone(),
            operation: operation.into(),
            started: Utc::now(),
            finished: None,
            outcome: "În curs".into(),
            detail: String::new(),
        };
        self.engine.store.put_run(&run)?;
        *self.engine.live.lock().unwrap() = Some(Progress {
            plan_id: plan.id.clone(),
            operation: operation.into(),
            detail: "Pornire…".into(),
            ..Progress::default()
        });
        let accepted = run.clone();
        let engine = self.engine.clone();
        self.tasks.spawn(async move {
            let mut run = run;
            let result = match run.operation.as_str() {
                "Backup" => engine.backup(&plan).await,
                "Restaurare" => match restore {
                    Some((snapshot, parent)) => engine.restore(&plan, &snapshot, &parent).await,
                    None => unreachable!(),
                },
                _ => engine.check(&plan).await,
            };
            let (outcome, detail) = match result {
                Ok(pair) => pair,
                Err(e) => ("Eroare".into(), format!("{e:#}")),
            };
            run.outcome = outcome;
            run.detail = detail;
            run.finished = Some(Utc::now());
            if run.operation == "Backup"
                && let Err(e) = engine.store.finish_backup(
                    &plan.id,
                    matches!(run.outcome.as_str(), "Reușit" | "Reușit cu avertisment"),
                )
            {
                run.detail
                    .push_str(&format!(" Eroare la salvarea programării: {e}"));
                run.outcome = "Eroare".into();
            }
            if let Err(e) = engine.store.put_run(&run) {
                eprintln!("Istoricul nu a putut fi salvat: {e}");
            }
            *engine.live.lock().unwrap() = None;
        });
        Ok(Response::Started(accepted))
    }
    async fn handle(&mut self, request: Request) -> Result<Response> {
        if self.update_reserved()
            && !matches!(
                &request,
                Request::State
                    | Request::Status
                    | Request::PrepareUpdate(_)
                    | Request::CancelUpdate(_)
                    | Request::ShutdownForUpdate(_)
            )
        {
            bail!("Actualizarea este pregătită. Operațiile noi sunt suspendate temporar.");
        }
        match request {
            Request::PrepareUpdate(token) => {
                anyhow::ensure!(
                    uuid::Uuid::parse_str(&token).is_ok(),
                    "Invalid update token"
                );
                anyhow::ensure!(
                    self.engine.live.lock().unwrap().is_none(),
                    "Așteaptă terminarea operației înainte de actualizare."
                );
                anyhow::ensure!(
                    !self.update_reserved() || self.valid_update_token(&token),
                    "Another update is pending"
                );
                self.update_lease =
                    Some((token, std::time::Instant::now() + Duration::from_secs(120)));
                Ok(Response::UpdateReady {
                    agent_executable: std::env::current_exe()?,
                    data_dir: self.engine.store.root.clone(),
                    service_mode: self.service_mode,
                })
            }
            Request::CancelUpdate(token) => {
                if self.valid_update_token(&token) {
                    self.update_lease = None;
                }
                Ok(Response::Ok("Update cancelled".into()))
            }
            Request::ShutdownForUpdate(token) => {
                anyhow::ensure!(
                    self.valid_update_token(&token),
                    "Update reservation expired"
                );
                anyhow::ensure!(
                    self.engine.live.lock().unwrap().is_none(),
                    "Așteaptă terminarea operației înainte de actualizare."
                );
                self.shutdown_requested = true;
                Ok(Response::Ok("Agent ready for update".into()))
            }
            Request::Shutdown => {
                if self.service_mode {
                    bail!("Serviciul se oprește din Windows Services.");
                }
                if self.engine.live.lock().unwrap().is_some() {
                    bail!("O operație este în curs. Agentul poate fi oprit după finalizarea ei.");
                }
                self.shutdown_requested = true;
                Ok(Response::Ok("Agentul a fost oprit în siguranță.".into()))
            }
            Request::State => Ok(Response::State(self.state()?)),
            Request::Status => {
                let (plan_count, last_outcome) = self.engine.store.status_summary()?;
                Ok(Response::Status(AgentStatus {
                    agent_version: env!("CARGO_PKG_VERSION").into(),
                    plan_count,
                    last_outcome,
                    active: self.engine.live.lock().unwrap().clone(),
                    service_mode: self.service_mode,
                }))
            }
            Request::Add(draft) => {
                let p = self.engine.store.add(draft)?;
                Ok(Response::Ok(format!(
                    "Planul «{}» a fost salvat. Poți porni primul backup.",
                    p.name
                )))
            }
            Request::Update { id, changes } => {
                if self.engine.live.lock().unwrap().is_some() {
                    bail!("Așteaptă finalizarea operației curente înainte de a edita planul.");
                }
                let plan = self.engine.store.update(&id, changes)?;
                Ok(Response::Ok(format!(
                    "Planul «{}» a fost actualizat.",
                    plan.name
                )))
            }
            Request::Toggle(id) => {
                self.engine.store.toggle(&id)?;
                Ok(Response::Ok("Programare actualizată.".into()))
            }
            Request::Delete(id) => {
                if self.engine.live.lock().unwrap().is_some() {
                    bail!("Așteaptă finalizarea operației curente înainte de a șterge planul.");
                }
                let plan = self.engine.store.delete(&id)?;
                Ok(Response::Ok(format!(
                    "Planul «{}» a fost șters și programarea a fost eliminată. Istoricul, fișierele sursă și copiile de backup au fost păstrate.",
                    plan.name
                )))
            }
            Request::TestRemote {
                destination,
                credentials,
                plan,
            } => {
                anyhow::ensure!(
                    self.engine.live.lock().unwrap().is_none(),
                    "Așteaptă terminarea operației curente."
                );
                let credentials = match credentials {
                    Some(value) => value,
                    None => {
                        let id = plan
                            .as_deref()
                            .context("Conectează destinația online înainte de salvare.")?;
                        let saved = self.engine.store.plan(id)?;
                        anyhow::ensure!(
                            saved
                                .remote
                                .as_ref()
                                .is_some_and(|r| r.same_location(&destination)),
                            "Pentru altă destinație online creează un plan nou."
                        );
                        self.engine.store.remote_secret(id)?
                    }
                };
                let id = plan.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                crate::remote::list(
                    &self
                        .engine
                        .binary
                        .with_file_name(crate::platform::executable("rclone")),
                    &destination,
                    &credentials,
                    &id,
                )
                .await?;
                Ok(Response::Ok(
                    "Conexiune reușită. Accesul de scriere va fi verificat la primul backup."
                        .into(),
                ))
            }
            Request::Backup(id) => self.start(&id, "Backup", None),
            Request::Check(id) => self.start(&id, "Verificare", None),
            Request::Restore {
                plan,
                snapshot,
                parent,
            } => self.start(&plan, "Restaurare", Some((snapshot, parent))),
            Request::Snapshots(id) => {
                if self.engine.live.lock().unwrap().is_some() {
                    bail!(
                        "Așteaptă finalizarea operației curente înainte de a încărca versiunile."
                    );
                }
                let plan = self.engine.store.plan(&id)?;
                // Bounded read keeps service shutdown and health checks responsive.
                let snapshots =
                    tokio::time::timeout(Duration::from_secs(30), self.engine.snapshots(&plan))
                        .await??;
                Ok(Response::Snapshots(snapshots))
            }
        }
    }
}

pub async fn run(
    root: PathBuf,
    binary: PathBuf,
    service_mode: bool,
    stop: watch::Receiver<bool>,
) -> Result<()> {
    run_on_pipe(
        root,
        binary,
        service_mode,
        stop,
        crate::security::pipe_name()?,
    )
    .await
}

pub async fn run_on_pipe(
    root: PathBuf,
    binary: PathBuf,
    service_mode: bool,
    mut stop: watch::Receiver<bool>,
    pipe_name: String,
) -> Result<()> {
    // Claim the per-user pipe before opening/recovering the database: only one agent may own it.
    #[cfg(windows)]
    let mut listener = create_pipe(true, &pipe_name)?;
    #[cfg(unix)]
    let listener = crate::unix_transport::Listener::bind(&pipe_name)?;
    let store = Store::open(&root)?;
    store.recover_interrupted()?;
    let engine = Engine {
        binary,
        store,
        live: Arc::new(Mutex::new(None)),
    };
    let mut worker = Worker {
        update_lease: None,
        shutdown_requested: false,
        engine,
        service_mode,
        tasks: JoinSet::new(),
    };
    let mut tick = tokio::time::interval(Duration::from_secs(15));
    loop {
        tokio::select! {
            _ = stop.changed() => { break; }
            _ = tick.tick() => {
                while let Some(result) = worker.tasks.try_join_next() {
                    if let Err(e) = result { eprintln!("Operație întreruptă: {e}"); *worker.engine.live.lock().unwrap() = None; worker.engine.store.recover_interrupted()?; }
                }
                if let Err(e) = worker.schedule_due() { eprintln!("Programare: {e}"); }
            }
            connection = async {
                #[cfg(windows)] { listener.connect().await }
                #[cfg(unix)] { listener.accept().await }
            } => {
                #[cfg(windows)]
                connection?;
                // Keep a listening instance alive while serving the connected client.
                #[cfg(windows)]
                let mut pipe = std::mem::replace(&mut listener, create_pipe(false, &pipe_name)?);
                #[cfg(unix)]
                let mut pipe = connection?;
                let request = tokio::time::timeout(Duration::from_secs(3), protocol::read::<Request>(&mut pipe)).await;
                let response = match request {
                    Ok(Ok(request)) => match worker.handle(request).await { Ok(r) => r, Err(e) => Response::Error(format!("{e:#}")) },
                    _ => Response::Error("Cerere invalidă sau incompletă.".into()),
                };
                let _ = tokio::time::timeout(Duration::from_secs(3), protocol::write(&mut pipe, &response)).await;
                if worker.shutdown_requested { break; }
                // Dropping disconnects the pipe; no blocking FlushFileBuffers call.
            }
        }
    }
    worker.tasks.abort_all();
    while worker.tasks.join_next().await.is_some() {}
    worker.engine.store.recover_interrupted()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn update_lease_preserves_work_blocks_new_tasks_and_expires() {
        let root = tempfile::tempdir().unwrap();
        let mut worker = Worker {
            update_lease: None,
            shutdown_requested: false,
            service_mode: true,
            tasks: JoinSet::new(),
            engine: Engine {
                binary: PathBuf::new(),
                store: Store::open(root.path()).unwrap(),
                live: Arc::new(Mutex::new(None)),
            },
        };
        let token = uuid::Uuid::new_v4().to_string();
        let other = uuid::Uuid::new_v4().to_string();
        for operation in ["Backup", "Restaurare", "Verificare"] {
            *worker.engine.live.lock().unwrap() = Some(Progress {
                operation: operation.into(),
                ..Progress::default()
            });
            assert!(
                worker
                    .handle(Request::PrepareUpdate(token.clone()))
                    .await
                    .is_err()
            );
            assert!(!worker.update_reserved());
            assert!(!worker.shutdown_requested);
        }
        *worker.engine.live.lock().unwrap() = None;
        assert!(
            worker
                .handle(Request::PrepareUpdate("bad-token".into()))
                .await
                .is_err()
        );
        assert!(matches!(
            worker
                .handle(Request::PrepareUpdate(token.clone()))
                .await
                .unwrap(),
            Response::UpdateReady {
                service_mode: true,
                ..
            }
        ));
        assert!(worker.handle(Request::State).await.is_ok());
        assert!(worker.handle(Request::Status).await.is_ok());
        assert!(
            worker
                .handle(Request::Backup("any-plan".into()))
                .await
                .unwrap_err()
                .to_string()
                .contains("suspendate")
        );
        assert!(
            worker
                .handle(Request::Delete("any-plan".into()))
                .await
                .is_err()
        );
        assert!(
            worker
                .handle(Request::PrepareUpdate(other.clone()))
                .await
                .is_err()
        );
        assert!(
            worker
                .handle(Request::ShutdownForUpdate(other.clone()))
                .await
                .is_err()
        );
        worker.handle(Request::CancelUpdate(other)).await.unwrap();
        assert!(worker.update_reserved());
        worker
            .handle(Request::CancelUpdate(token.clone()))
            .await
            .unwrap();
        assert!(!worker.update_reserved());
        worker.update_lease = Some((
            token.clone(),
            std::time::Instant::now() - Duration::from_secs(1),
        ));
        assert!(!worker.update_reserved());
        assert!(
            worker
                .handle(Request::ShutdownForUpdate(token.clone()))
                .await
                .is_err()
        );
        worker
            .handle(Request::PrepareUpdate(token.clone()))
            .await
            .unwrap();
        worker.schedule_due().unwrap();
        assert!(worker.engine.live.lock().unwrap().is_none());
        assert!(matches!(
            worker
                .handle(Request::ShutdownForUpdate(token))
                .await
                .unwrap(),
            Response::Ok(_)
        ));
        assert!(
            worker.shutdown_requested,
            "idle service can drain without a forced SCM stop"
        );
        assert!(worker.engine.store.history().unwrap().is_empty());
    }
    #[tokio::test]
    async fn compact_status_matches_history_without_transferring_details() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(tmp.path()).unwrap();
        let completed = Run {
            id: "done".into(),
            plan_id: "removed-plan".into(),
            plan_name: "Example".into(),
            operation: "Backup".into(),
            started: Utc::now() - chrono::Duration::seconds(1),
            finished: Some(Utc::now()),
            outcome: "Eroare".into(),
            detail: "private-source-path".repeat(10000),
        };
        store.put_run(&completed).unwrap();
        let mut current = completed.clone();
        current.id = "running".into();
        current.started = Utc::now();
        current.finished = None;
        store.put_run(&current).unwrap();
        let mut worker = Worker {
            update_lease: None,
            shutdown_requested: false,
            service_mode: false,
            tasks: JoinSet::new(),
            engine: Engine {
                binary: PathBuf::new(),
                store,
                live: Arc::new(Mutex::new(Some(Progress::default()))),
            },
        };
        let response = worker.handle(Request::Status).await.unwrap();
        let bytes = serde_json::to_vec(&response).unwrap();
        assert!(bytes.len() < 1024);
        assert!(
            !String::from_utf8(bytes)
                .unwrap()
                .contains("private-source-path")
        );
        let Response::Status(status) = response else {
            panic!("compact status expected")
        };
        assert_eq!(status.last_outcome.unwrap().outcome, "Eroare");
        assert!(status.active.is_some());
        assert_eq!(status.plan_count, 0);
        assert_eq!(worker.engine.store.history().unwrap().len(), 2);
    }
    #[tokio::test]
    async fn delete_refuses_any_active_operation_and_removes_schedule_when_idle() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&repo).unwrap();
        let store = Store::open(&tmp.path().join("app")).unwrap();
        let plan = store
            .add(NewPlan {
                remote: None,
                remote_credentials: None,
                keep_last: None,
                use_vss: false,
                mode: BackupMode::Plain,
                name: "Delete test".into(),
                sources: vec![source],
                repository: repo,
                excluded_paths: vec![],
                excludes: vec![],
                schedule: Schedule::Interval { minutes: 60 },
                password: String::new(),
            })
            .unwrap();
        let mut worker = Worker {
            update_lease: None,
            shutdown_requested: false,
            engine: Engine {
                binary: PathBuf::new(),
                store,
                live: Arc::new(Mutex::new(None)),
            },
            service_mode: false,
            tasks: JoinSet::new(),
        };
        for operation in ["Backup", "Restaurare", "Verificare"] {
            *worker.engine.live.lock().unwrap() = Some(Progress {
                operation: operation.into(),
                plan_id: "another-plan".into(),
                ..Progress::default()
            });
            let error = worker
                .handle(Request::Delete(plan.id.clone()))
                .await
                .unwrap_err();
            assert!(error.to_string().contains("operației curente"));
            assert!(worker.engine.store.plan(&plan.id).is_ok());
        }
        *worker.engine.live.lock().unwrap() = None;
        assert!(matches!(
            worker.handle(Request::Delete(plan.id.clone())).await,
            Ok(Response::Ok(_))
        ));
        worker.schedule_due().unwrap();
        assert!(worker.engine.store.plans().unwrap().is_empty());
        assert!(worker.engine.store.history().unwrap().is_empty());
        assert!(worker.engine.live.lock().unwrap().is_none());
    }
    #[tokio::test]
    async fn tray_shutdown_refuses_active_operations_and_services() {
        let root = tempfile::tempdir().unwrap();
        let mut worker = Worker {
            update_lease: None,
            shutdown_requested: false,
            engine: Engine {
                binary: PathBuf::new(),
                store: Store::open(root.path()).unwrap(),
                live: Arc::new(Mutex::new(Some(Progress::default()))),
            },
            service_mode: false,
            tasks: JoinSet::new(),
        };
        assert!(worker.handle(Request::Shutdown).await.is_err());
        assert!(!worker.shutdown_requested);
        *worker.engine.live.lock().unwrap() = None;
        worker.service_mode = true;
        assert!(worker.handle(Request::Shutdown).await.is_err());
        assert!(!worker.shutdown_requested);
        worker.service_mode = false;
        assert!(matches!(
            worker.handle(Request::Shutdown).await,
            Ok(Response::Ok(_))
        ));
        assert!(worker.shutdown_requested);
    }
}
