use chrono::Local;
use iced::widget::rule::horizontal as horizontal_rule;
use iced::{
    Color, Element, Fill, Length, Task, Theme,
    widget::{
        button, checkbox, column, container, pick_list, progress_bar, row, scrollable, svg,
        text as raw_text, text_input, tooltip,
    },
};
use std::{path::PathBuf, time::Duration};
use straja::i18n::{self, Language, tr};
use straja::remote::{self, Provider};
use straja::updates::{self, CheckError, Update};
use straja::{
    model::*,
    protocol::{self, Request, Response},
};
use zeroize::Zeroize;

fn text<'a>(value: impl AsRef<str>) -> iced::widget::Text<'a> {
    raw_text(tr(value))
}

struct GuiExecutor(tokio::runtime::Runtime);
impl iced::Executor for GuiExecutor {
    fn new() -> std::io::Result<Self> {
        straja::resources::runtime().map(Self)
    }
    fn spawn(&self, future: impl std::future::Future<Output = ()> + Send + 'static) {
        drop(self.0.spawn(future));
    }
    fn enter<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = self.0.enter();
        f()
    }
    fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        self.0.block_on(future)
    }
}

const INK: Color = Color::from_rgb(0.10, 0.16, 0.25);
const MUTED: Color = Color::from_rgb(0.40, 0.45, 0.53);
const BLUE: Color = Color::from_rgb(0.18, 0.35, 0.86);
const CREATOR_URL: &str = "https://weblike.ro/";

fn creator_logo(width: f32) -> Element<'static, Message> {
    static LOGO: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    let logo = svg(LOGO
        .get_or_init(|| {
            svg::Handle::from_memory(include_bytes!("../installer/assets/weblike-pro.svg"))
        })
        .clone())
    .width(width)
    .height(width * 54.0 / 301.0);
    button(container(logo).padding(10).style(|_| container::Style {
        background: Some(Color::from_rgb8(12, 23, 48).into()),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }))
    .padding(0)
    .style(button::text)
    .on_press(Message::OpenCreatorSite)
    .into()
}

#[cfg(windows)]
fn open_site(address: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let url: Vec<u16> = address.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    // A fixed HTTPS URL, dispatched through the user's default browser association.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    } as isize;
    if result <= 32 {
        Err(format!(
            "Site-ul nu a putut fi deschis (cod {result}). Adresa: {address}"
        ))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn launch_vss_agent() -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("straja-agent.exe");
    let sid = straja::security::user_sid().map_err(|e| e.to_string())?;
    let path: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let args: Vec<u16> = format!("--vss-start --expected-user-sid {sid}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            path.as_ptr(),
            args.as_ptr(),
            std::ptr::null(),
            0,
        )
    } as isize;
    if result <= 32 {
        Err(tr(
            "Pornirea agentului VSS a fost anulată sau refuzată de Windows.",
        ))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn open_site(address: &str) -> Result<(), String> {
    std::process::Command::new(straja::platform::FILE_MANAGER)
        .arg(address)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
#[cfg(unix)]
fn launch_vss_agent() -> Result<(), String> {
    Err(straja::vss::RIGHTS_ERROR.into())
}

fn app_theme(_: &App) -> Theme {
    Theme::custom(
        "Straja",
        iced::theme::Palette {
            background: Color::from_rgb(0.95, 0.96, 0.98),
            text: INK,
            primary: BLUE,
            success: Color::from_rgb(0.12, 0.55, 0.40),
            danger: Color::from_rgb(0.78, 0.22, 0.25),
            warning: Color::from_rgb(0.80, 0.55, 0.10),
        },
    )
}

const WINDOW_TITLE: &str = concat!("Straja Backup — v", env!("CARGO_PKG_VERSION"));

fn window_settings() -> iced::window::Settings {
    iced::window::Settings {
        size: (1120., 780.).into(),
        icon: Some(
            iced::window::icon::from_rgba(
                include_bytes!("../installer/assets/straja.rgba").to_vec(),
                128,
                128,
            )
            .expect("Embedded Straja icon must contain 128 × 128 RGBA pixels"),
        ),
        ..Default::default()
    }
}

pub fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .executor::<GuiExecutor>()
        .title(WINDOW_TITLE)
        .theme(app_theme)
        .subscription(|app: &App| iced::time::every(app.poll_interval()).map(|_| Message::Tick))
        .window(window_settings())
        .run()
}

// Debug-only visual fixture: no IPC, tray, real settings, file pickers or backup operations.
#[cfg(any(debug_assertions, feature = "ui-preview"))]
pub fn preview() -> iced::Result {
    iced::application(
        || {
            let (mut app, _) = App::new();
            app.connected = true;
            app.polling = false;
            app.state.agent_version = env!("CARGO_PKG_VERSION").into();
            app.state.engine_available = true;
            app.page = Page::Plans;
            if std::env::args().any(|arg| arg == "--preview-updates") {
                app.page = Page::About;
                app.update_checking = false;
                app.update_attempt = std::time::Instant::now() - Duration::from_secs(61);
                app.update_result = Some(Ok(Some(Update {
                    version: "0.6.0".into(),
                    installer: Some(updates::Installer {url: String::new(), size: 1234, sha256: "a".repeat(64)}),
                    notes: "Preview release notes\n• Update notifications\n• Backup improvements".into(),
                    page_url: "https://github.com/weblikero/straja-backup/releases".into(),
                })));
                if std::env::args().any(|arg| arg == "--preview-update-confirmation") {
                    let update = app.update_result.as_ref().unwrap().as_ref().unwrap().as_ref().unwrap().clone();
                    app.prepared_update = Some(updates::PreparedUpdate {update, path: PathBuf::from("preview-only.exe")});
                    app.install_confirmation = true;
                }
            }
            app.state.plans.push(Plan {
            remote: None,
                mode: BackupMode::Plain, id: "preview".into(), name: "My projects · Grüße".into(),
                sources: vec![PathBuf::from(r"C:\Example\Projects")],
                repository: PathBuf::from(r"E:\Backups\Projects"),
                excluded_paths: vec![PathBuf::from(r"C:\Example\Projects\cache")],
                excludes: vec!["*.tmp".into()], keep_last: Some(7), use_vss: false,
                schedule: Schedule::Daily { hour:19, minute:0 }, enabled: true,
                next_due: Some(chrono::Utc::now()), last_success: None,
            });
            app.state.history.push(Run {
                id: "preview-run".into(), plan_id: "preview".into(), plan_name: "My projects · Grüße".into(),
                operation: "Backup".into(), started: chrono::Utc::now(), finished: Some(chrono::Utc::now()),
                outcome: "Eroare".into(),
                detail: r"Backup nefinalizat. Copia parțială se află în E:\Backups\Projects\.in-progress-test; versiunile anterioare sunt intacte.: Legătură/junction/reparse point neacceptat: C:\Example\Projects\public\build. Alege folderul real sau exclude legătura.".into(),
            });
            if std::env::args().any(|arg| arg == "--preview-plans") {
                app.state.plans[0].excluded_paths = (0..60).map(|n| PathBuf::from(format!(r"C:\Example\Projects\project-{n}\cache"))).collect();
                app.state.plans[0].sources.push(PathBuf::from(r"C:\Example\Shared"));
                let mut second = app.state.plans[0].clone();
                second.id = "preview-photos".into();
                second.name = "Photos".into();
                second.sources = vec![PathBuf::from(r"C:\Example\Pictures")];
                second.repository = PathBuf::from(r"E:\Backups\Pictures");
                second.last_success = Some(chrono::Utc::now());
                second.excluded_paths.clear();
                second.excludes.clear();
                second.enabled = false;
                app.state.plans.push(second);
                let mut third = app.state.plans[1].clone();
                third.id = "preview-documents".into();
                third.name = "Documents".into();
                third.sources = vec![PathBuf::from(r"C:\Example\A very long folder name with Unicode Grüße\Customer documents\Archive\Documents")];
                third.repository = PathBuf::from(r"E:\Backups\A very long destination folder name\Documents");
                third.enabled = true;
                third.last_success = None;
                third.next_due = None;
                third.schedule = Schedule::Manual;
                app.state.plans.push(third);
            }
            if let Some(output) = std::env::args_os().skip_while(|a| a != "--scroll-probe").nth(1) {
                app.page = Page::History;
                let run = app.state.history[0].clone();
                app.state.history = (0..100).map(|n| Run {id: format!("probe-{n}"), ..run.clone()}).collect();
                app.scroll_probe = Some(ScrollProbe {output: output.into(), frames: 0, previous: None, intervals: Vec::new()});
            }
            if std::env::args().any(|arg| arg == "--preview-plan-details") {
                app.expanded_plan = Some("preview".into());
            }
            if let Some(code) = std::env::args().skip_while(|arg| arg != "--preview-language").nth(1) {
                i18n::set(Language::from_code(&code));
            }
            if std::env::args().any(|arg| arg == "--preview-vss") {
                let mut wizard = Wizard::from_plan(&app.state.plans[0]);
                wizard.use_vss = true;
                wizard.step = 2;
                app.wizard = Some(wizard);
                app.page = Page::Plans;
            }
            if std::env::args().any(|arg|arg=="--preview-remote") {
                let mut wizard=Wizard::default();wizard.step=1;wizard.name="Online backup".into();
                wizard.remote.provider=Provider::Ftp;wizard.remote.host="ftp.example.com".into();wizard.remote.user="demo".into();
                app.wizard=Some(wizard);app.page=Page::Plans;
            }
            (app, Task::none())
        },
        |app: &mut App, message| {
            match message {
                Message::ProbeFrame(at) => {
                    if let Some(probe) = &mut app.scroll_probe {
                        return probe.frame(at);
                    }
                }
                Message::Language(language) => i18n::set(language),
                Message::Next => { if let Some(w) = &mut app.wizard { w.step = (w.step+1).min(3); } }
                message @ (Message::Page(_) | Message::Edit(_) | Message::TogglePlanDetails(_) | Message::New | Message::CloseWizard
                    | Message::Back | Message::ExcludeErrorPath(_) | Message::ManualExclusion(_)
                    | Message::EditExclusion(_) | Message::CancelExclusion | Message::SaveExclusion
                    | Message::RemoveExclusion(_) | Message::Name(_) | Message::Mode(_) | Message::Frequency(_)
                    | Message::Day(_) | Message::Time(_) | Message::Interval(_) | Message::KeepAll(_) | Message::UseVss(_)
                    | Message::KeepCount(_) | Message::AskInstallUpdate | Message::LaterUpdate
                    | Message::RemoteProvider(_) | Message::RemoteField(_,_) | Message::RemoteTls(_)) => { return app.update(message); }
                _ => {}
            }
            Task::none()
        }, App::view)
        .executor::<GuiExecutor>()
        .title(concat!("Straja Backup — v", env!("CARGO_PKG_VERSION"), " — Preview"))
        .theme(app_theme)
        .subscription(|app: &App| if app.scroll_probe.is_some() { iced::window::frames().map(Message::ProbeFrame) } else { iced::Subscription::none() })
        .window(window_settings())
        .run()
}

// Synthetic scroll cadence only; never enabled in the installed release or normal preview.
#[cfg(any(debug_assertions, feature = "ui-preview"))]
struct ScrollProbe {
    output: PathBuf,
    frames: usize,
    previous: Option<std::time::Instant>,
    intervals: Vec<f64>,
}
#[cfg(any(debug_assertions, feature = "ui-preview"))]
impl ScrollProbe {
    fn frame(&mut self, at: std::time::Instant) -> Task<Message> {
        self.frames += 1;
        if let Some(previous) = self.previous
            && self.frames > 120
        {
            self.intervals
                .push(at.saturating_duration_since(previous).as_secs_f64() * 1000.0);
        }
        self.previous = Some(at);
        if self.intervals.len() >= 180 {
            self.intervals.sort_by(f64::total_cmp);
            let report = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "requested_backend": std::env::var("ICED_BACKEND").unwrap_or_else(|_| "automatic".into()),
                "sample_frames": self.intervals.len(), "warmup_frames": 120, "history_rows": 100,
                "mean_frame_ms": self.intervals.iter().sum::<f64>() / self.intervals.len() as f64,
                "p95_frame_ms": self.intervals[170], "max_frame_ms": self.intervals.last(),
            });
            std::fs::write(
                &self.output,
                serde_json::to_vec_pretty(&report).expect("probe JSON"),
            )
            .expect("write isolated scroll probe report");
            return iced::exit();
        }
        let phase = self.frames % 240;
        let y = if phase < 120 { phase } else { 240 - phase } as f32 * 40.0;
        iced::widget::operation::scroll_to("main-content", scrollable::AbsoluteOffset { x: 0.0, y })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Page {
    Home,
    Plans,
    Restore,
    History,
    About,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frequency {
    Manual,
    Interval,
    Daily,
    Weekly,
}
impl std::fmt::Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&tr(match self {
            Self::Manual => "Doar manual",
            Self::Interval => "La un interval",
            Self::Daily => "Zilnic",
            Self::Weekly => "Săptămânal",
        }))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Day {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}
impl std::fmt::Display for Day {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&tr([
            "Luni",
            "Marți",
            "Miercuri",
            "Joi",
            "Vineri",
            "Sâmbătă",
            "Duminică",
        ][*self as usize]))
    }
}
#[derive(Debug, Clone)]
enum Message {
    RemoteProvider(Provider),
    RemoteField(u8, String),
    RemoteTls(bool),
    ConnectRemote,
    RemoteConnected(Result<remote::Credentials, String>),
    TestRemote,
    RemoteTested(Result<Response, String>),
    #[cfg(any(debug_assertions, feature = "ui-preview"))]
    ProbeFrame(std::time::Instant),
    DownloadUpdate,
    CancelDownload,
    UpdateDownloaded(Result<updates::PreparedUpdate, String>),
    AskInstallUpdate,
    ConfirmInstallUpdate,
    LaterUpdate,
    UpdaterLaunched(Result<(), String>),
    CheckUpdates,
    UpdatesChecked(Result<Option<Update>, CheckError>),
    Language(Language),
    OpenProductSite,
    OpenReleasePage,
    #[cfg(unix)]
    EnableAutostart,
    CopyText(String),
    ExcludeErrorPath(String),
    AskDelete(String),
    CancelDelete,
    DeleteAcknowledged(bool),
    ConfirmDelete,
    Deleted(String, Result<Response, String>),
    OpenCreatorSite,
    KeepAll(bool),
    UseVss(bool),
    KeepCount(String),
    Mode(BackupMode),
    OpenFolder(PathBuf),
    Tick,
    Page(Page),
    Refresh(Result<Response, String>),
    Reply(Result<Response, String>),
    New,
    Edit(String),
    TogglePlanDetails(String),
    CloseWizard,
    Next,
    Back,
    Name(String),
    Password(String),
    ConfirmPassword(String),
    Excludes(String),
    BrowseExclusions(bool),
    ExcludedPaths(Vec<PathBuf>),
    RemoveExclusion(usize),
    ManualExclusion(String),
    SaveExclusion,
    EditExclusion(usize),
    CancelExclusion,
    BrowseSources(bool),
    Sources(Vec<PathBuf>),
    RemoveSource(usize),
    BrowseRepo,
    Repo(Option<PathBuf>),
    Frequency(Frequency),
    Day(Day),
    Time(String),
    Interval(String),
    Acknowledge(bool),
    Save,
    Saved(Result<Response, String>),
    Backup(String),
    Toggle(String),
    Check(String),
    LoadSnapshots(String),
    LoadedSnapshots(String, Result<Response, String>),
    BrowseRestore,
    RestoreParent(Option<PathBuf>),
    SelectSnapshot(String),
    Restore,
    StartAgent,
    StartVssAgent,
    AgentStarted(Result<(), String>),
}

struct Wizard {
    remote: remote::Destination,
    remote_credentials: remote::Credentials,
    remote_credentials_changed: bool,
    remote_notice: String,
    remote_port: String,
    remote_bandwidth: String,
    keep_all: bool,
    use_vss: bool,
    keep_count: String,
    mode: BackupMode,
    editing: Option<String>,
    original_repository: Option<PathBuf>,
    step: u8,
    name: String,
    sources: Vec<PathBuf>,
    repository: Option<PathBuf>,
    password: String,
    confirm_password: String,
    excluded_paths: Vec<PathBuf>,
    exclusion_input: String,
    exclusion_edit: Option<usize>,
    excludes: String,
    frequency: Frequency,
    day: Day,
    time: String,
    interval: String,
    acknowledged: bool,
    error: String,
}
impl Default for Wizard {
    fn default() -> Self {
        Self {
            remote: remote::Destination::default(),
            remote_credentials: remote::Credentials::default(),
            remote_credentials_changed: false,
            remote_notice: String::new(),
            remote_port: "21".into(),
            remote_bandwidth: "10240".into(),
            keep_all: true,
            use_vss: false,
            keep_count: "7".into(),
            mode: BackupMode::Encrypted,
            editing: None,
            original_repository: None,
            step: 0,
            name: String::new(),
            sources: vec![],
            repository: None,
            password: String::new(),
            confirm_password: String::new(),
            excluded_paths: vec![],
            exclusion_input: String::new(),
            exclusion_edit: None,
            excludes: String::new(),
            frequency: Frequency::Daily,
            day: Day::Mon,
            time: "19:00".into(),
            interval: "60".into(),
            acknowledged: false,
            error: String::new(),
        }
    }
}
impl Drop for Wizard {
    fn drop(&mut self) {
        self.password.zeroize();
        self.confirm_password.zeroize();
    }
}
impl Wizard {
    fn from_plan(p: &Plan) -> Self {
        let mut w = Self::default();
        w.remote = p.remote.clone().unwrap_or_default();
        w.remote_port = w.remote.port.to_string();
        w.remote_bandwidth = w.remote.bandwidth_kib.to_string();
        w.mode = p.mode;
        w.use_vss = p.use_vss;
        w.keep_all = p.keep_last.is_none();
        w.keep_count = p.keep_last.unwrap_or(7).to_string();
        w.editing = Some(p.id.clone());
        w.original_repository = Some(p.repository.clone());
        w.name = p.name.clone();
        w.sources = p.sources.clone();
        w.repository = Some(p.repository.clone());
        w.excludes = p.excludes.join("; ");
        w.excluded_paths = p.excluded_paths.clone();
        w.acknowledged = true;
        match p.schedule {
            Schedule::Manual => w.frequency = Frequency::Manual,
            Schedule::Interval { minutes } => {
                w.frequency = Frequency::Interval;
                w.interval = minutes.to_string();
            }
            Schedule::Daily { hour, minute } => {
                w.frequency = Frequency::Daily;
                w.time = format!("{hour:02}:{minute:02}");
            }
            Schedule::Weekly {
                weekday,
                hour,
                minute,
            } => {
                w.frequency = Frequency::Weekly;
                w.day = [
                    Day::Mon,
                    Day::Tue,
                    Day::Wed,
                    Day::Thu,
                    Day::Fri,
                    Day::Sat,
                    Day::Sun,
                ][weekday.min(6) as usize];
                w.time = format!("{hour:02}:{minute:02}");
            }
        }
        w
    }
    fn save_exclusion(&mut self) -> Result<(), String> {
        let input = self.exclusion_input.trim();
        let input = input
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(input);
        if input.is_empty() {
            return Err("Introdu calea completă a fișierului sau folderului exclus.".into());
        }
        // Keep the literal link path: never canonicalize or resolve junctions here.
        let path = PathBuf::from(input);
        if self.excluded_paths.iter().enumerate().any(|(i, p)| {
            Some(i) != self.exclusion_edit
                && straja::exclusions::normalized(p) == straja::exclusions::normalized(&path)
        }) {
            return Err("Această cale este deja în lista de excluderi.".into());
        }
        let mut candidates = self.excluded_paths.clone();
        if let Some(i) = self.exclusion_edit {
            let entry = candidates
                .get_mut(i)
                .ok_or("Excluderea nu mai există. Anulează editarea și încearcă din nou.")?;
            *entry = path;
        } else {
            candidates.push(path);
        }
        straja::exclusions::validate_paths(&self.sources, &candidates)
            .map_err(|e| e.to_string())?;
        self.excluded_paths = candidates;
        self.exclusion_input.clear();
        self.exclusion_edit = None;
        Ok(())
    }
    fn retention(&self) -> Result<Option<u32>, String> {
        let keep = if self.keep_all {
            None
        } else {
            Some(
                self.keep_count
                    .parse()
                    .map_err(|_| "Introdu un număr întreg de copii.")?,
            )
        };
        validate_retention(keep).map_err(|e| e.to_string())?;
        Ok(keep)
    }
    fn schedule(&self) -> Result<Schedule, String> {
        let parse_time = || -> Result<(u32, u32), String> {
            let (h, m) = self
                .time
                .split_once(':')
                .ok_or("Introdu ora în format HH:MM.")?;
            Ok((
                h.parse().map_err(|_| "Oră invalidă.")?,
                m.parse().map_err(|_| "Minute invalide.")?,
            ))
        };
        let s = match self.frequency {
            Frequency::Manual => Schedule::Manual,
            Frequency::Interval => Schedule::Interval {
                minutes: self
                    .interval
                    .parse()
                    .map_err(|_| "Introdu un număr de minute.")?,
            },
            Frequency::Daily => {
                let (hour, minute) = parse_time()?;
                Schedule::Daily { hour, minute }
            }
            Frequency::Weekly => {
                let (hour, minute) = parse_time()?;
                Schedule::Weekly {
                    weekday: self.day as u32,
                    hour,
                    minute,
                }
            }
        };
        s.validate().map_err(|e| e.to_string())?;
        Ok(s)
    }
    fn validate_step(&self) -> Result<(), String> {
        if self.step == 1 && self.remote.provider != Provider::Local {
            self.remote.validate().map_err(|e| e.to_string())?;
            if self.editing.is_none() || self.remote_credentials_changed {
                self.remote_credentials
                    .validate(self.remote.provider)
                    .map_err(|e| e.to_string())?;
            }
        }
        match self.step {
            0 if self.name.trim().is_empty() || self.sources.is_empty() => {
                Err("Dă un nume planului și alege cel puțin o sursă.".into())
            }
            0 if !self.exclusion_input.trim().is_empty() || self.exclusion_edit.is_some() => {
                Err("Adaugă/salvează calea de excludere introdusă sau apasă Anulează înainte de a continua.".into())
            }
            0 => straja::exclusions::validate_paths(&self.sources, &self.excluded_paths)
                .map_err(|e| e.to_string()),
            1 if self.remote.provider==Provider::Local && self.repository.is_none() => {
                Err("Alege folderul în care se vor salva copiile.".into())
            }
            1 if self.mode == BackupMode::Encrypted
                && self.editing.is_none()
                && self.password.len() < 8 =>
            {
                Err("Folosește o parolă cu minimum 8 caractere.".into())
            }
            1 if self.mode == BackupMode::Encrypted
                && self.editing.is_none()
                && self.password != self.confirm_password =>
            {
                Err("Cele două parole nu coincid.".into())
            }
            1 => {
                if self.remote.provider!=Provider::Local {return Ok(())}
                let repo = self.repository.as_ref().unwrap();
                if self.editing.is_some()
                    && self.original_repository.as_ref() == Some(repo)
                    && (repo.join("config").is_file() || repo.join(straja::plain::MARKER).is_file())
                {
                    Ok(())
                } else {
                    validate_empty_repository(repo).map_err(|e| format!("{e:#}"))
                }
            }
            2 => self.schedule().and_then(|_| self.retention()).map(|_| ()),
            3 if self.mode == BackupMode::Encrypted && !self.acknowledged => {
                Err("Confirmă că ai păstrat parola într-un loc sigur.".into())
            }
            _ => Ok(()),
        }
    }
}

struct App {
    #[cfg(any(debug_assertions, feature = "ui-preview"))]
    scroll_probe: Option<ScrollProbe>,
    download_progress: Option<std::sync::Arc<updates::DownloadProgress>>,
    prepared_update: Option<updates::PreparedUpdate>,
    update_error: Option<String>,
    install_waiting: bool,
    install_starting: bool,
    install_confirmation: bool,
    update_dismissed: bool,
    update_checking: bool,
    update_result: Option<Result<Option<Update>, CheckError>>,
    update_attempt: std::time::Instant,
    delete_plan: Option<String>,
    expanded_plan: Option<String>,
    delete_acknowledged: bool,
    pending_run: Option<Run>,
    notice_run: Option<Run>,
    page: Page,
    state: State,
    connected: bool,
    polling: bool,
    busy: bool,
    notice: String,
    error: bool,
    wizard: Option<Wizard>,
    restore_plan: Option<String>,
    snapshots: Vec<Snapshot>,
    selected_snapshot: Option<String>,
    restore_parent: Option<PathBuf>,
}
fn rpc(request: Request) -> Task<Message> {
    Task::perform(
        async move { protocol::call(request).await.map_err(|e| format!("{e:#}")) },
        Message::Reply,
    )
}
impl App {
    fn poll_interval(&self) -> Duration {
        if self.state.active.is_some()
            || self.pending_run.is_some()
            || self.busy
            || self.download_progress.is_some()
            || self.install_waiting
            || self.install_starting
        {
            straja::resources::ACTIVE_POLL
        } else if !self.connected {
            Duration::from_secs(5)
        } else {
            straja::resources::IDLE_UI_POLL
        }
    }
    fn new() -> (Self, Task<Message>) {
        let mut app = Self {
            #[cfg(any(debug_assertions, feature = "ui-preview"))]
            scroll_probe: None,
            download_progress: None,
            prepared_update: None,
            update_error: None,
            install_waiting: false,
            install_starting: false,
            install_confirmation: false,
            update_dismissed: false,
            update_checking: true,
            update_result: None,
            update_attempt: std::time::Instant::now(),
            delete_plan: None,
            expanded_plan: None,
            delete_acknowledged: false,
            pending_run: None,
            notice_run: None,
            page: Page::Home,
            state: State::default(),
            connected: false,
            polling: false,
            busy: false,
            notice: String::new(),
            error: false,
            wizard: None,
            restore_plan: None,
            snapshots: vec![],
            selected_snapshot: None,
            restore_parent: None,
        };
        let task = Task::batch([
            app.poll(),
            Task::perform(updates::check(false), Message::UpdatesChecked),
        ]);
        (app, task)
    }
    fn poll(&mut self) -> Task<Message> {
        if self.polling {
            return Task::none();
        }
        self.polling = true;
        Task::perform(
            async {
                protocol::call(Request::State)
                    .await
                    .map_err(|e| format!("{e:#}"))
            },
            Message::Refresh,
        )
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        // Once the helper owns the handoff, never open a new unsaved form or start work.
        if self.install_starting
            && !matches!(
                &message,
                Message::UpdaterLaunched(_) | Message::Tick | Message::Refresh(_)
            )
        {
            return Task::none();
        }
        match message {
            #[cfg(any(debug_assertions, feature = "ui-preview"))]
            Message::ProbeFrame(_) => {}
            Message::DownloadUpdate => {
                if self.download_progress.is_none()
                    && !self.install_starting
                    && let Some(Ok(Some(update))) = &self.update_result
                    && update.installer.is_some()
                {
                    let update = update.clone();
                    let progress = std::sync::Arc::new(updates::DownloadProgress::default());
                    self.download_progress = Some(progress.clone());
                    self.update_error = None;
                    return Task::perform(
                        async move {
                            updates::download(update, progress)
                                .await
                                .map_err(|e| format!("{e:#}"))
                        },
                        Message::UpdateDownloaded,
                    );
                }
            }
            Message::CancelDownload => {
                if let Some(progress) = &self.download_progress {
                    progress
                        .cancelled
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Message::UpdateDownloaded(result) => {
                self.download_progress = None;
                match result {
                    Ok(prepared) => self.prepared_update = Some(prepared),
                    Err(error) => self.update_error = Some(error),
                }
            }
            Message::AskInstallUpdate => {
                if self.prepared_update.is_some() && self.wizard.is_none() && !self.install_starting
                {
                    self.install_confirmation = true;
                    self.update_dismissed = false;
                }
            }
            Message::ConfirmInstallUpdate => {
                if self.install_confirmation && self.wizard.is_none() {
                    self.install_confirmation = false;
                    self.install_waiting = true;
                    return self.start_update();
                }
            }
            Message::LaterUpdate => {
                if !self.install_starting {
                    self.install_waiting = false;
                    self.install_confirmation = false;
                    self.update_dismissed = true;
                }
            }
            Message::UpdaterLaunched(result) => match result {
                Ok(()) => return iced::exit(),
                Err(error) => {
                    self.install_starting = false;
                    self.install_waiting = false;
                    self.update_error = Some(error);
                }
            },
            Message::RemoteProvider(provider) => {
                if self.busy {
                    return Task::none();
                }
                if let Some(w) = &mut self.wizard
                    && w.editing.is_none()
                {
                    w.remote.provider = provider;
                    w.remote_credentials = Default::default();
                    w.remote_credentials_changed = false;
                    w.remote_notice.clear();
                    if provider != Provider::Local {
                        w.mode = BackupMode::Encrypted;
                    }
                }
            }
            Message::RemoteField(field, value) => {
                if let Some(w) = &mut self.wizard {
                    w.remote_notice.clear();
                    match field {
                        0 => w.remote.host = value,
                        1 => {
                            w.remote.port = value.parse().unwrap_or(0);
                            w.remote_port = value;
                        }
                        2 => w.remote.user = value,
                        3 => {
                            w.remote_credentials.password = value;
                            w.remote_credentials_changed = true;
                        }
                        4 => w.remote.bucket = value,
                        5 => w.remote.region = value,
                        6 => w.remote.path = value,
                        7 => {
                            w.remote_credentials.access_key = value;
                            w.remote_credentials_changed = true;
                        }
                        8 => {
                            w.remote_credentials.secret_key = value;
                            w.remote_credentials_changed = true;
                        }
                        9 => {
                            w.remote.bandwidth_kib = value.parse().unwrap_or(u32::MAX);
                            w.remote_bandwidth = value;
                        }
                        _ => {}
                    }
                }
            }
            Message::RemoteTls(value) => {
                if let Some(w) = &mut self.wizard {
                    w.remote.ftp_tls = value;
                }
            }
            Message::ConnectRemote => {
                if self.busy {
                    return Task::none();
                }
                if let Some(w) = &mut self.wizard {
                    let provider = w.remote.provider;
                    w.remote_notice = tr("Termină autentificarea în browser (maximum 3 minute).");
                    self.busy = true;
                    return Task::perform(
                        async move { remote::authorize(provider).await.map_err(|e| e.to_string()) },
                        Message::RemoteConnected,
                    );
                }
            }
            Message::RemoteConnected(result) => {
                self.busy = false;
                if let Some(w) = &mut self.wizard {
                    match result {
                        Ok(credentials) => {
                            w.remote_credentials = credentials;
                            w.remote_credentials_changed = true;
                            w.remote_notice = tr("Cont conectat.");
                            w.error.clear();
                        }
                        Err(e) => {
                            w.error = e;
                            w.remote_notice.clear();
                        }
                    }
                }
            }
            Message::TestRemote => {
                if !self.available() {
                    return Task::none();
                }
                if let Some(w) = &mut self.wizard {
                    if let Err(e) = w.remote.validate() {
                        w.error = e.to_string();
                        return Task::none();
                    }
                    let credentials = (w.editing.is_none() || w.remote_credentials_changed)
                        .then(|| w.remote_credentials.clone());
                    let request = Request::TestRemote {
                        destination: w.remote.clone(),
                        credentials,
                        plan: w.editing.clone(),
                    };
                    self.busy = true;
                    w.remote_notice = tr("Testez conexiunea…");
                    w.error.clear();
                    return Task::perform(
                        async move { protocol::call(request).await.map_err(|e| e.to_string()) },
                        Message::RemoteTested,
                    );
                }
            }
            Message::RemoteTested(result) => {
                self.busy = false;
                if let Some(w) = &mut self.wizard {
                    w.remote_notice.clear();
                    match result {
                        Ok(Response::Ok(s)) => w.remote_notice = tr(s),
                        Ok(Response::Error(e)) | Err(e) => w.error = e,
                        _ => w.error = tr("Conexiunea online a eșuat."),
                    }
                }
            }
            Message::CheckUpdates => {
                if !self.update_checking
                    && self.download_progress.is_none()
                    && !self.install_waiting
                    && !self.install_starting
                    && self.update_attempt.elapsed() >= Duration::from_secs(60)
                {
                    self.prepared_update = None;
                    self.update_error = None;
                    self.update_dismissed = false;
                    self.update_checking = true;
                    self.update_attempt = std::time::Instant::now();
                    return Task::perform(updates::check(true), Message::UpdatesChecked);
                }
            }
            Message::UpdatesChecked(result) => {
                self.update_checking = false;
                self.update_result = Some(result);
            }
            Message::Language(language) => {
                if i18n::save(language).is_err() {
                    self.notice_run = None;
                    self.notice = "Preferința de limbă nu a putut fi salvată.".into();
                    self.error = true;
                }
            }
            Message::OpenReleasePage => {
                if let Some(Ok(Some(update))) = &self.update_result
                    && let Err(error) = open_site(&update.page_url)
                {
                    self.update_error = Some(error);
                }
            }
            #[cfg(unix)]
            Message::EnableAutostart => {
                if self.busy || self.state.active.is_some() {
                    return Task::none();
                }
                self.busy = true;
                return Task::perform(
                    async {
                        let script = std::env::current_exe()
                            .map_err(|e| e.to_string())?
                            .with_file_name("enable-autostart.sh");
                        let output = tokio::process::Command::new("/bin/bash")
                            .arg(script)
                            .output()
                            .await
                            .map_err(|e| e.to_string())?;
                        if output.status.success() {
                            Ok(())
                        } else {
                            Err(String::from_utf8_lossy(&output.stderr)
                                .chars()
                                .take(2000)
                                .collect())
                        }
                    },
                    Message::AgentStarted,
                );
            }
            Message::OpenProductSite => {
                if let Err(e) = open_site("https://strajabackup.com/") {
                    self.notice_run = None;
                    self.notice = e;
                    self.error = true;
                }
            }

            Message::CopyText(value) => return iced::clipboard::write(value),
            Message::ExcludeErrorPath(id) => {
                if !self.available() || self.wizard.is_some() {
                    return Task::none();
                }
                let run = self
                    .state
                    .history
                    .iter()
                    .find(|r| r.id == id)
                    .or_else(|| self.notice_run.as_ref().filter(|r| r.id == id));
                if let Some(run) = run
                    && let Some(path) = backup_error_path(run)
                    && let Some(plan) = self.state.plans.iter().find(|p| p.id == run.plan_id)
                    && straja::exclusions::validate_paths(
                        &plan.sources,
                        std::slice::from_ref(&path),
                    )
                    .is_ok()
                {
                    let mut wizard = Wizard::from_plan(plan);
                    wizard.exclusion_edit = wizard.excluded_paths.iter().position(|p| {
                        straja::exclusions::normalized(p) == straja::exclusions::normalized(&path)
                    });
                    wizard.exclusion_input = path.display().to_string();
                    self.wizard = Some(wizard);
                    self.page = Page::Plans;
                    self.delete_plan = None;
                    self.notice_run = None;
                    self.notice = "Calea din eroare este completată mai jos. O poți modifica, apoi apasă Adaugă excluderea / Salvează calea și salvează planul. Se aplică backupurilor viitoare.".into();
                    self.error = false;
                }
            }
            Message::AskDelete(id) => {
                if self.available() && self.state.plans.iter().any(|p| p.id == id) {
                    self.delete_plan = Some(id);
                    self.delete_acknowledged = false;
                    self.notice_run = None;
                    self.notice.clear();
                }
            }
            Message::CancelDelete => {
                if !self.busy {
                    self.delete_plan = None;
                    self.delete_acknowledged = false;
                }
            }
            Message::DeleteAcknowledged(value) => self.delete_acknowledged = value,
            Message::ConfirmDelete => {
                if self.available()
                    && let Some(plan) = self
                        .state
                        .plans
                        .iter()
                        .find(|p| Some(&p.id) == self.delete_plan.as_ref())
                    && (plan.mode != BackupMode::Encrypted || self.delete_acknowledged)
                {
                    let id = plan.id.clone();
                    self.busy = true;
                    return Task::perform(
                        async move {
                            let result = protocol::call(Request::Delete(id.clone()))
                                .await
                                .map_err(|e| format!("{e:#}"));
                            (id, result)
                        },
                        |(id, result)| Message::Deleted(id, result),
                    );
                }
            }
            Message::Deleted(id, result) => {
                self.busy = false;
                match result {
                    Ok(Response::Ok(message)) => {
                        self.state.plans.retain(|p| p.id != id);
                        self.delete_plan = None;
                        self.delete_acknowledged = false;
                        if self.restore_plan.as_ref() == Some(&id) {
                            self.restore_plan = None;
                            self.snapshots.clear();
                            self.selected_snapshot = None;
                            self.restore_parent = None;
                        }
                        if self
                            .wizard
                            .as_ref()
                            .is_some_and(|w| w.editing.as_ref() == Some(&id))
                        {
                            self.wizard = None;
                        }
                        self.notice_run = None;
                        self.notice = message;
                        self.error = false;
                    }
                    Ok(Response::Error(error)) | Err(error) => {
                        self.notice_run = None;
                        self.notice = error;
                        self.error = true;
                    }
                    _ => {
                        self.notice_run = None;
                        self.notice = "Agentul a trimis un răspuns neașteptat la ștergere.".into();
                        self.error = true;
                    }
                }
                return self.poll();
            }
            Message::OpenCreatorSite => {
                if let Err(error) = open_site(CREATOR_URL) {
                    self.notice_run = None;
                    self.notice = error;
                    self.error = true;
                }
            }
            Message::Mode(mode) => {
                if let Some(w) = &mut self.wizard
                    && w.editing.is_none()
                {
                    w.mode = mode;
                    w.password.zeroize();
                    w.confirm_password.zeroize();
                    w.acknowledged = false;
                    w.error.clear();
                }
            }
            Message::OpenFolder(path) => {
                if let Err(e) = (|| -> Result<(), String> {
                    if !path.is_dir() {
                        return Err("Folderul nu este disponibil.".into());
                    }
                    std::process::Command::new(straja::platform::FILE_MANAGER)
                        .arg(path)
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    Ok(())
                })() {
                    self.notice_run = None;
                    self.notice = e;
                    self.error = true;
                }
            }
            Message::Tick => {
                let poll = self.poll();
                if !self.update_checking
                    && self.download_progress.is_none()
                    && self.prepared_update.is_none()
                    && self.update_attempt.elapsed() >= updates::CHECK_INTERVAL
                {
                    self.update_checking = true;
                    self.update_attempt = std::time::Instant::now();
                    return Task::batch([
                        poll,
                        Task::perform(updates::check(false), Message::UpdatesChecked),
                    ]);
                }
                return poll;
            }
            Message::Refresh(result) => {
                self.polling = false;
                match result {
                    Ok(Response::State(state)) => {
                        self.state = state;
                        self.connected = true;
                        self.update_run_notice();
                        if self.install_waiting {
                            return self.start_update();
                        }
                    }
                    _ => {
                        self.connected = false;
                    }
                }
            }
            Message::Page(page) => {
                self.page = page;
                self.wizard = None;
                self.delete_plan = None;
                self.delete_acknowledged = false;
            }
            Message::TogglePlanDetails(id) => {
                self.expanded_plan = if self.expanded_plan.as_ref() == Some(&id) {
                    None
                } else if self.state.plans.iter().any(|p| p.id == id) {
                    Some(id)
                } else {
                    self.expanded_plan.take()
                };
            }
            Message::Reply(result) => {
                self.busy = false;
                match result {
                    Ok(Response::Started(run)) => {
                        self.notice_run = None;
                        self.notice = format!(
                            "{} · {}: cerere acceptată, se pregătește operația…",
                            run.operation, run.plan_name
                        );
                        self.error = false;
                        self.pending_run = Some(run);
                        self.update_run_notice();
                    }
                    Ok(Response::Ok(message)) => {
                        self.notice_run = None;
                        self.notice = message;
                        self.error = false;
                    }
                    Ok(Response::Error(e)) | Err(e) => {
                        self.notice_run = None;
                        self.notice = e;
                        self.error = true;
                    }
                    _ => {}
                }
                return self.poll();
            }
            Message::StartAgent => {
                self.busy = true;
                return Task::perform(
                    async {
                        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                        let mut cmd = std::process::Command::new(
                            exe.with_file_name(straja::platform::executable("straja-agent")),
                        );
                        #[cfg(windows)]
                        use std::os::windows::process::CommandExt;
                        #[cfg(windows)]
                        cmd.creation_flags(0x08000000);
                        cmd.stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null());
                        cmd.spawn().map_err(|e| e.to_string())?;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        protocol::call(Request::State)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok(())
                    },
                    Message::AgentStarted,
                );
            }
            Message::StartVssAgent => {
                if !self.available() || self.state.service_mode {
                    return Task::none();
                }
                self.busy = true;
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(launch_vss_agent)
                            .await
                            .map_err(|e| e.to_string())??;
                        for _ in 0..30 {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            if let Ok(Ok(Response::State(state))) = tokio::time::timeout(
                                Duration::from_secs(2),
                                protocol::call(Request::State),
                            )
                            .await
                                && state.vss_available
                            {
                                return Ok(());
                            }
                        }
                        Err(tr(
                            "Agentul VSS nu s-a conectat. Verifică mesajul Windows și folosește același cont de utilizator.",
                        ))
                    },
                    Message::AgentStarted,
                );
            }
            Message::AgentStarted(result) => {
                self.busy = false;
                match result {
                    Ok(()) => {
                        self.notice_run = None;
                        self.notice =
                            "Agentul rulează în fundal. Va continua și după închiderea ferestrei."
                                .into();
                        self.error = false;
                    }
                    Err(e) => {
                        self.notice_run = None;
                        self.notice = e;
                        self.error = true;
                    }
                }
                return self.poll();
            }
            Message::New => {
                self.delete_plan = None;
                self.wizard = Some(Wizard::default());
                self.notice_run = None;
                self.notice.clear();
            }
            Message::Edit(id) => {
                self.delete_plan = None;
                if self.available()
                    && let Some(plan) = self.state.plans.iter().find(|p| p.id == id)
                {
                    self.wizard = Some(Wizard::from_plan(plan));
                    self.notice_run = None;
                    self.notice.clear();
                }
            }
            Message::CloseWizard => self.wizard = None,
            Message::Back => {
                if let Some(w) = &mut self.wizard {
                    w.step = w.step.saturating_sub(1);
                    w.error.clear();
                }
            }
            Message::Next => {
                if let Some(w) = &mut self.wizard {
                    match w.validate_step() {
                        Ok(()) => {
                            w.step = (w.step + 1).min(3);
                            w.error.clear();
                        }
                        Err(e) => w.error = e,
                    }
                }
            }
            Message::Name(s) => {
                if let Some(w) = &mut self.wizard {
                    w.name = s;
                }
            }
            Message::Password(s) => {
                if let Some(w) = &mut self.wizard {
                    w.password.zeroize();
                    w.password = s;
                }
            }
            Message::ConfirmPassword(s) => {
                if let Some(w) = &mut self.wizard {
                    w.confirm_password.zeroize();
                    w.confirm_password = s;
                }
            }
            Message::BrowseExclusions(folders) => {
                let Some(w) = &self.wizard else {
                    return Task::none();
                };
                let directory = w.sources.first().cloned();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let mut picker = rfd::FileDialog::new()
                                .set_title(tr("Alege ce excluzi din sursele de backup"));
                            if let Some(path) = directory {
                                picker = picker.set_directory(if path.is_dir() {
                                    path
                                } else {
                                    path.parent().unwrap_or(&path).to_path_buf()
                                });
                            }
                            if folders {
                                picker.pick_folders().unwrap_or_default()
                            } else {
                                picker.pick_files().unwrap_or_default()
                            }
                        })
                        .await
                        .unwrap_or_default()
                    },
                    Message::ExcludedPaths,
                );
            }
            Message::ExcludedPaths(paths) => {
                if let Some(w) = &mut self.wizard {
                    w.error.clear();
                    for path in paths {
                        if w.excluded_paths.iter().any(|p| {
                            straja::exclusions::normalized(p)
                                == straja::exclusions::normalized(&path)
                        }) {
                            continue;
                        }
                        let mut candidates = w.excluded_paths.clone();
                        candidates.push(path);
                        match straja::exclusions::validate_paths(&w.sources, &candidates) {
                            Ok(()) => w.excluded_paths = candidates,
                            Err(e) => w.error = e.to_string(),
                        }
                    }
                }
            }
            Message::ManualExclusion(value) => {
                if let Some(w) = &mut self.wizard {
                    w.exclusion_input = value;
                    w.error.clear();
                }
            }
            Message::SaveExclusion => {
                if let Some(w) = &mut self.wizard {
                    w.error = w.save_exclusion().err().unwrap_or_default();
                }
            }
            Message::EditExclusion(i) => {
                if let Some(w) = &mut self.wizard
                    && i < w.excluded_paths.len()
                {
                    if !w.exclusion_input.trim().is_empty() || w.exclusion_edit.is_some() {
                        w.error =
                            "Salvează sau anulează calea introdusă înainte de a edita alta.".into();
                    } else {
                        w.exclusion_input = w.excluded_paths[i].display().to_string();
                        w.exclusion_edit = Some(i);
                        w.error.clear();
                    }
                }
            }
            Message::CancelExclusion => {
                if let Some(w) = &mut self.wizard {
                    w.exclusion_input.clear();
                    w.exclusion_edit = None;
                    w.error.clear();
                }
            }
            Message::RemoveExclusion(i) => {
                if let Some(w) = &mut self.wizard
                    && i < w.excluded_paths.len()
                {
                    w.excluded_paths.remove(i);
                    if w.exclusion_edit == Some(i) {
                        w.exclusion_edit = None;
                        w.exclusion_input.clear();
                    } else if let Some(edit) = w.exclusion_edit
                        && edit > i
                    {
                        w.exclusion_edit = Some(edit - 1);
                    }
                    w.error.clear();
                }
            }
            Message::Excludes(s) => {
                if let Some(w) = &mut self.wizard {
                    w.excludes = s;
                }
            }
            Message::UseVss(value) => {
                if let Some(w) = self.wizard.as_mut() {
                    w.use_vss = value;
                }
            }
            Message::KeepAll(value) => {
                if let Some(w) = self.wizard.as_mut() {
                    w.keep_all = value;
                }
            }
            Message::KeepCount(value) => {
                if let Some(w) = self.wizard.as_mut() {
                    w.keep_count = value;
                }
            }
            Message::Frequency(f) => {
                if let Some(w) = &mut self.wizard {
                    w.frequency = f;
                }
            }
            Message::Day(d) => {
                if let Some(w) = &mut self.wizard {
                    w.day = d;
                }
            }
            Message::Time(s) => {
                if let Some(w) = &mut self.wizard {
                    w.time = s;
                }
            }
            Message::Interval(s) => {
                if let Some(w) = &mut self.wizard {
                    w.interval = s;
                }
            }
            Message::Acknowledge(b) => {
                if let Some(w) = &mut self.wizard {
                    w.acknowledged = b;
                }
            }
            Message::BrowseSources(folders) => {
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let picker =
                                rfd::FileDialog::new().set_title(tr("Alege sursele pentru backup"));
                            if folders {
                                picker.pick_folders().unwrap_or_default()
                            } else {
                                picker.pick_files().unwrap_or_default()
                            }
                        })
                        .await
                        .unwrap_or_default()
                    },
                    Message::Sources,
                );
            }
            Message::Sources(paths) => {
                if let Some(w) = &mut self.wizard {
                    for p in paths {
                        if !w.sources.contains(&p) {
                            w.sources.push(p);
                        }
                    }
                }
            }
            Message::RemoveSource(i) => {
                if let Some(w) = &mut self.wizard
                    && i < w.sources.len()
                {
                    w.sources.remove(i);
                }
            }
            Message::BrowseRepo => {
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            rfd::FileDialog::new()
                                .set_title(tr("Alege un folder gol dedicat backupului"))
                                .pick_folder()
                        })
                        .await
                        .unwrap_or(None)
                    },
                    Message::Repo,
                );
            }
            Message::Repo(p) => {
                if let Some(w) = &mut self.wizard
                    && p.is_some()
                {
                    w.repository = p;
                }
            }
            Message::Save => {
                let Some(w) = &mut self.wizard else {
                    return Task::none();
                };
                if let Err(e) = w.validate_step() {
                    w.error = e;
                    return Task::none();
                }
                let schedule = match w.schedule() {
                    Ok(s) => s,
                    Err(e) => {
                        w.error = e;
                        return Task::none();
                    }
                };
                let Some(repository) = w
                    .repository
                    .clone()
                    .or_else(|| (w.remote.provider != Provider::Local).then(PathBuf::new))
                else {
                    return Task::none();
                };
                let keep_last = match w.retention() {
                    Ok(keep) => keep,
                    Err(e) => {
                        w.error = e;
                        return Task::none();
                    }
                };
                let changes = PlanChanges {
                    remote: (w.remote.provider != Provider::Local).then(|| w.remote.clone()),
                    remote_credentials: (w.remote.provider != Provider::Local
                        && (w.editing.is_none() || w.remote_credentials_changed))
                        .then(|| w.remote_credentials.clone()),
                    keep_last,
                    use_vss: w.use_vss,
                    name: w.name.clone(),
                    sources: w.sources.clone(),
                    repository,
                    excluded_paths: w.excluded_paths.clone(),
                    excludes: w
                        .excludes
                        .split(';')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect(),
                    schedule,
                };
                let request = if let Some(id) = &w.editing {
                    Request::Update {
                        id: id.clone(),
                        changes,
                    }
                } else {
                    Request::Add(NewPlan {
                        remote: changes.remote,
                        remote_credentials: changes.remote_credentials,
                        mode: w.mode,
                        name: changes.name,
                        sources: changes.sources,
                        repository: changes.repository,
                        excluded_paths: changes.excluded_paths,
                        excludes: changes.excludes,
                        keep_last: changes.keep_last,
                        use_vss: changes.use_vss,
                        schedule: changes.schedule,
                        password: w.password.clone(),
                    })
                };
                self.busy = true;
                return Task::perform(
                    async move { protocol::call(request).await.map_err(|e| format!("{e:#}")) },
                    Message::Saved,
                );
            }
            Message::Saved(result) => {
                self.busy = false;
                match result {
                    Ok(Response::Ok(s)) => {
                        self.wizard = None;
                        self.page = Page::Plans;
                        self.notice_run = None;
                        self.notice = s;
                        self.error = false;
                    }
                    Ok(Response::Error(e)) | Err(e) => {
                        if let Some(w) = &mut self.wizard {
                            w.error = e;
                        }
                    }
                    _ => {}
                }
                return self.poll();
            }
            Message::Backup(id) => {
                self.busy = true;
                self.notice_run = None;
                self.notice = "Trimit cererea de backup către agent…".into();
                self.error = false;
                return rpc(Request::Backup(id));
            }
            Message::Toggle(id) => {
                self.busy = true;
                return rpc(Request::Toggle(id));
            }
            Message::Check(id) => {
                self.busy = true;
                return rpc(Request::Check(id));
            }
            Message::LoadSnapshots(id) => {
                self.page = Page::Restore;
                self.snapshots.clear();
                self.selected_snapshot = None;
                self.restore_plan = Some(id.clone());
                self.busy = true;
                return Task::perform(
                    async move {
                        let response = protocol::call(Request::Snapshots(id.clone()))
                            .await
                            .map_err(|e| format!("{e:#}"));
                        (id, response)
                    },
                    |(id, r)| Message::LoadedSnapshots(id, r),
                );
            }
            Message::LoadedSnapshots(id, result) => {
                self.busy = false;
                if self.restore_plan.as_ref() == Some(&id) {
                    match result {
                        Ok(Response::Snapshots(list)) => {
                            self.selected_snapshot = list.first().map(|s| s.id.clone());
                            self.snapshots = list;
                            self.notice_run = None;
                            self.notice.clear();
                        }
                        Ok(Response::Error(e)) | Err(e) => {
                            self.notice_run = None;
                            self.notice = e;
                            self.error = true;
                        }
                        _ => {}
                    }
                }
            }
            Message::BrowseRestore => {
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            rfd::FileDialog::new()
                                .set_title(tr("Alege unde se creează folderul de restaurare"))
                                .pick_folder()
                        })
                        .await
                        .unwrap_or(None)
                    },
                    Message::RestoreParent,
                );
            }
            Message::RestoreParent(parent) => {
                if parent.is_some() {
                    self.restore_parent = parent;
                }
            }
            Message::SelectSnapshot(id) => self.selected_snapshot = Some(id),
            Message::Restore => {
                if let (Some(plan), Some(snapshot), Some(parent)) = (
                    &self.restore_plan,
                    &self.selected_snapshot,
                    &self.restore_parent,
                ) {
                    self.busy = true;
                    return rpc(Request::Restore {
                        plan: plan.clone(),
                        snapshot: snapshot.clone(),
                        parent: parent.clone(),
                    });
                }
            }
        }
        Task::none()
    }
    fn vss_activation(&self) -> Element<'_, Message> {
        let mut content =
            column![text("VSS necesită agent cu drepturi de administrator.").size(15)];
        if self.state.service_mode {
            content = content.push(text("Configurează serviciul Straja cu un cont administrator, apoi repornește serviciul din Windows Services.").size(14));
        } else {
            content = content.push(text("Activarea repornește agentul cu aprobarea Windows, sub același cont. Pentru programare fără autentificare, configurează serviciul Straja.").size(14).color(MUTED))
                .push(button(text("Activează agentul VSS")).on_press_maybe(self.available().then_some(Message::StartVssAgent)));
        }
        content.spacing(10).into()
    }
    fn available(&self) -> bool {
        self.compatible()
            && !self.busy
            && !self.install_waiting
            && !self.install_starting
            && self.state.active.is_none()
            && self.pending_run.is_none()
    }
    fn start_update(&mut self) -> Task<Message> {
        if !self.install_waiting
            || self.install_starting
            || self.busy
            || self.state.active.is_some()
            || self.pending_run.is_some()
            || self.wizard.is_some()
        {
            return Task::none();
        }
        if self.connected && !self.compatible() {
            self.install_waiting = false;
            self.update_error =
                Some("Agent version mismatch. Restart the matching agent first.".into());
            return Task::none();
        }
        if let Some(prepared) = self.prepared_update.clone() {
            self.install_starting = true;
            self.update_error = None;
            return Task::perform(
                async move {
                    #[cfg(windows)]
                    {
                        straja::updater::launch(prepared)
                            .await
                            .map_err(|e| format!("{e:#}"))
                    }
                    #[cfg(unix)]
                    {
                        let _ = prepared;
                        Err("Install the native package to update this platform.".to_owned())
                    }
                },
                Message::UpdaterLaunched,
            );
        }
        Task::none()
    }
    fn compatible(&self) -> bool {
        self.connected && self.state.agent_version == env!("CARGO_PKG_VERSION")
    }
    fn update_run_notice(&mut self) {
        if let Some(pending) = &self.pending_run
            && let Some(run) = self
                .state
                .history
                .iter()
                .find(|r| r.id == pending.id && r.finished.is_some())
        {
            self.notice = format!(
                "{} · {}: {}. {}",
                run.operation, run.plan_name, run.outcome, run.detail
            );
            self.error = run.outcome != "Reușit";
            self.notice_run = Some(run.clone());
            self.pending_run = None;
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let nav = |title, page| {
            button(text(title).size(16))
                .width(Fill)
                .padding([13, 17])
                .style(if self.page == page {
                    button::primary
                } else {
                    button::text
                })
                .on_press(Message::Page(page))
        };
        let sidebar = container(
            column![
                text("STRAJA").size(29).color(BLUE),
                text(if cfg!(windows) {
                    "BACKUP PENTRU WINDOWS"
                } else {
                    "BACKUP PENTRU FIȘIERELE TALE"
                })
                .size(11)
                .color(MUTED),
                iced::widget::Space::new().height(28),
                nav("Privire de ansamblu", Page::Home),
                nav("Planurile mele", Page::Plans),
                nav("Restaurare", Page::Restore),
                nav("Istoric", Page::History),
                iced::widget::Space::new().height(Fill),
                nav("Despre aplicație", Page::About),
                text("Limbă").size(12).color(MUTED),
                pick_list(Language::ALL, Some(i18n::current()), Message::Language).width(Fill),
                horizontal_rule(1),
                text("Datele tale. Copiile tale.").size(13).color(MUTED),
                creator_logo(178.0),
                text("Creată de Weblike PRO\nUn proiect Open-Source")
                    .size(12)
                    .color(MUTED),
                button(text("weblike.ro").size(12))
                    .padding(0)
                    .style(button::text)
                    .on_press(Message::OpenCreatorSite),
                text(format!("v{} · Previzualizare", env!("CARGO_PKG_VERSION")))
                    .size(12)
                    .color(MUTED)
            ]
            .spacing(10)
            .height(Fill),
        )
        .width(245)
        .height(Fill)
        .padding(22)
        .style(container::rounded_box);
        let (title, subtitle) = match self.page {
            Page::Home => (
                "Un loc sigur pentru fișierele tale",
                "Vezi starea copiilor și pornește un backup când ai nevoie.",
            ),
            Page::Plans => (
                "Planurile mele",
                "Alege ce păstrezi și lasă programarea în grija aplicației.",
            ),
            Page::Restore => (
                "Recuperează ce contează",
                "Alege o copie și restaurează fișierele într-un folder nou.",
            ),
            Page::History => (
                "Istoricul operațiilor",
                "Rezultate reale pentru backup, verificare și restaurare.",
            ),
            Page::About => (
                "Straja Backup",
                "O aplicație deschisă, cu control local asupra copiilor tale.",
            ),
        };
        let badge = if !self.connected {
            "Agent deconectat"
        } else if self.state.service_mode {
            "Serviciu Windows conectat"
        } else {
            "Agent în fundal conectat"
        };
        let mut body = column![
            row![
                column![text(title).size(29), text(subtitle).size(14).color(MUTED)]
                    .spacing(8)
                    .width(Fill),
                text(badge).size(12).color(if self.connected {
                    Color::from_rgb(0.12, 0.55, 0.40)
                } else {
                    MUTED
                })
            ]
            .spacing(15)
        ]
        .spacing(23);
        if self.page != Page::About
            && !self.update_dismissed
            && matches!(self.update_result, Some(Ok(Some(_))))
        {
            body = body.push(self.update_card(false));
        }
        if !self.connected {
            body = body.push(card(column![text("Pornește agentul pentru a accesa planurile").size(19), text("Agentul execută backupurile și rămâne activ după închiderea ferestrei. Pentru pornire automată la boot, instalează serviciul Windows.").size(14), button(text("Pornește agentul")).on_press_maybe((!self.busy).then_some(Message::StartAgent)).padding([10,16])].spacing(12)));
        } else if !self.compatible() {
            body = body.push(card(text("Rulează încă un agent din versiunea veche. După terminarea operațiilor, închide Straja și oprește straja-agent.exe din Task Manager (sau repornește Windows), apoi deschide aplicația instalată și pornește agentul.").color(Color::from_rgb(0.78, 0.22, 0.25))));
        } else if !self.state.engine_available {
            body = body.push(card(
                text("Motorul Restic lipsește pentru planurile criptate. Modul cu fișiere necriptate nu necesită Restic.")
                    .color(Color::from_rgb(0.78, 0.22, 0.25)),
            ));
        }
        if self.connected && !self.state.vss_available && self.state.plans.iter().any(|p| p.use_vss)
        {
            body = body.push(card(self.vss_activation()));
        }
        if !self.notice.is_empty() {
            let mut notice = column![
                raw_text(if let Some(run) = &self.notice_run {
                    format!(
                        "{} · {}: {}. {}",
                        tr(&run.operation),
                        run.plan_name,
                        tr(&run.outcome),
                        tr(&run.detail)
                    )
                } else {
                    tr(&self.notice)
                })
                .size(14)
                .color(if self.error {
                    Color::from_rgb(0.78, 0.22, 0.25)
                } else {
                    INK
                })
            ]
            .spacing(10);
            if let Some(run) = &self.notice_run {
                notice = notice.push(self.error_actions(run));
            } else if self.error {
                notice = notice.push(
                    button(text("Copiază mesajul"))
                        .style(button::secondary)
                        .on_press(Message::CopyText(tr(&self.notice))),
                );
            }
            body = body.push(card(notice));
        }
        if let Some(active) = &self.state.active {
            body = body.push(card(
                column![
                    row![
                        text(format!("{} în curs", active.operation)).size(19),
                        iced::widget::Space::new().width(Fill),
                        text(format!("{:.0}%", active.percent * 100.))
                    ],
                    progress_bar(0.0..=1.0, active.percent),
                    text(format!(
                        "{} fișiere · {:.1} MB",
                        active.files_done,
                        active.bytes_done as f64 / 1_048_576.
                    ))
                    .size(13)
                    .color(MUTED),
                    text(&active.detail).size(13),
                ]
                .spacing(12),
            ));
        }
        let content = if let Some(w) = &self.wizard {
            self.wizard_view(w)
        } else {
            match self.page {
                Page::Home => self.home(),
                Page::Plans => self.plans(),
                Page::Restore => self.restore(),
                Page::History => self.history(),
                Page::About => self.about(),
            }
        };
        body = body.push(content);
        row![
            sidebar,
            scrollable(container(body).padding(30).width(Fill))
                .id("main-content")
                .width(Fill)
        ]
        .height(Fill)
        .into()
    }
    fn home(&self) -> Element<'_, Message> {
        let complete = self
            .state
            .plans
            .iter()
            .filter(|p| p.last_success.is_some())
            .count();
        let scheduled = self
            .state
            .plans
            .iter()
            .filter(|p| p.enabled && p.next_due.is_some())
            .count();
        let metrics = row![
            metric("PLANURI", self.state.plans.len().to_string()),
            metric("CU BACKUP REUȘIT", complete.to_string()),
            metric("PROGRAMĂRI ACTIVE", scheduled.to_string())
        ]
        .spacing(16);
        let mut content = column![metrics].spacing(24);
        if self.state.plans.is_empty() {
            content = content.push(card(column![text("Primul backup începe cu un plan").size(25), text("Documente, fotografii sau proiecte. Alegi folderele, destinația și programul; Straja se ocupă de copiere.").size(16), button(text("+ Creează primul plan")).padding([13,20]).on_press_maybe((self.compatible() && !self.busy).then_some(Message::New))].spacing(18)));
        } else {
            content = content.push(row![
                text("Planurile tale").size(22),
                iced::widget::Space::new().width(Fill),
                button(text("+ Plan nou"))
                    .on_press_maybe((self.compatible() && !self.busy).then_some(Message::New))
            ]);
            for p in &self.state.plans {
                content = content.push(self.plan_card(p));
            }
        }
        content.push(text("Backupurile programate rulează cât timp PC-ul și agentul sunt pornite. Programările ratate se recuperează la revenire.").size(13).color(MUTED)).into()
    }
    fn plans(&self) -> Element<'_, Message> {
        let mut content = column![row![
            text(format!("{} planuri configurate", self.state.plans.len())).size(18),
            iced::widget::Space::new().width(Fill),
            button(text("+ Plan nou"))
                .padding([10, 16])
                .on_press_maybe((self.compatible() && !self.busy).then_some(Message::New))
        ]]
        .spacing(18);
        for p in &self.state.plans {
            content = content.push(self.plan_card(p));
        }
        if self.state.plans.is_empty() {
            content = content.push(text(
                "Nu ai creat încă un plan. Folosește «Plan nou» pentru configurare.",
            ));
        }
        content.into()
    }
    fn plan_status(&self, p: &Plan) -> (String, Option<chrono::DateTime<chrono::Utc>>, Color) {
        if let Some(active) = self.state.active.as_ref().filter(|a| a.plan_id == p.id) {
            return (
                format!("{} · {}", tr(&active.operation), tr("În curs")),
                None,
                BLUE,
            );
        }
        if let Some(pending) = self.pending_run.as_ref().filter(|r| r.plan_id == p.id) {
            return (
                format!("{} · {}", tr(&pending.operation), tr("Se pregătește…")),
                Some(pending.started),
                BLUE,
            );
        }
        if let Some(run) = self
            .state
            .history
            .iter()
            .find(|r| r.plan_id == p.id && r.operation == "Backup")
        {
            let color = match run.outcome.as_str() {
                "Eroare" | "Incomplet" | "Întrerupt" => Color::from_rgb(0.78, 0.22, 0.25),
                "Reușit cu avertisment" => Color::from_rgb(0.65, 0.40, 0.05),
                "Reușit" => Color::from_rgb(0.12, 0.55, 0.40),
                _ => INK,
            };
            return (
                tr(&run.outcome),
                Some(run.finished.unwrap_or(run.started)),
                color,
            );
        }
        if let Some(at) = p.last_success {
            (tr("Reușit"), Some(at), Color::from_rgb(0.12, 0.55, 0.40))
        } else {
            (tr("Nerulat"), None, MUTED)
        }
    }
    fn plan_card<'a>(&'a self, p: &'a Plan) -> Element<'a, Message> {
        let due = if !p.enabled {
            "Programare oprită".into()
        } else {
            p.next_due
                .map(|d| {
                    d.with_timezone(&Local)
                        .format("%Y-%m-%d · %H:%M")
                        .to_string()
                })
                .unwrap_or("La cerere".into())
        };
        let last = p
            .last_success
            .map(|d| {
                d.with_timezone(&Local)
                    .format("%Y-%m-%d · %H:%M")
                    .to_string()
            })
            .unwrap_or("Niciun backup complet încă".into());
        let latest = self
            .state
            .history
            .iter()
            .find(|r| r.plan_id == p.id && r.operation == "Backup");
        let expanded = self.expanded_plan.as_ref() == Some(&p.id);
        let (status, status_time, status_color) = self.plan_status(p);
        let mut status_cell = column![
            text("ULTIMA STARE").size(11).color(MUTED),
            raw_text(status).size(14).color(status_color),
        ]
        .spacing(4)
        .width(Fill);
        if let Some(at) = status_time {
            status_cell = status_cell.push(
                raw_text(
                    at.with_timezone(&Local)
                        .format("%Y-%m-%d · %H:%M")
                        .to_string(),
                )
                .size(12)
                .color(MUTED),
            );
        }
        let source = p
            .sources
            .first()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let mut source_cell = column![
            text("SURSE INCLUSE").size(11).color(MUTED),
            compact_path(source),
        ]
        .spacing(4)
        .width(Fill);
        if p.sources.len() > 1 {
            source_cell = source_cell.push(
                text(format!("Surse suplimentare: {}", p.sources.len() - 1))
                    .size(12)
                    .color(MUTED),
            );
        }
        let mut body = column![
            raw_text(&p.name).size(21),
            row![
                source_cell,
                column![
                    text("DESTINAȚIE").size(11).color(MUTED),
                    compact_path(p.destination_label())
                ]
                .spacing(4)
                .width(Fill),
            ]
            .spacing(20),
            row![
                column![
                    text("FRECVENȚĂ").size(11).color(MUTED),
                    text(p.schedule.label()).size(14)
                ]
                .spacing(4)
                .width(Fill),
                status_cell,
                column![
                    text("URMĂTORUL BACKUP").size(11).color(MUTED),
                    text(due).size(14)
                ]
                .spacing(4)
                .width(Fill),
            ]
            .spacing(20),
            text(format!(
                "Excluderi: {} căi · {} reguli",
                p.excluded_paths.len(),
                p.excludes.len()
            ))
            .size(12)
            .color(MUTED),
            row![
                button(text("Backup acum"))
                    .padding([8, 12])
                    .on_press_maybe(self.available().then(|| Message::Backup(p.id.clone()))),
                button(text("Editează"))
                    .style(button::secondary)
                    .padding([8, 12])
                    .on_press_maybe(self.available().then(|| Message::Edit(p.id.clone()))),
                button(text("Restaurează"))
                    .style(button::secondary)
                    .padding([8, 12])
                    .on_press_maybe(
                        self.available()
                            .then(|| Message::LoadSnapshots(p.id.clone()))
                    ),
                button(text(if expanded {
                    "Ascunde detaliile"
                } else {
                    "Detalii și acțiuni"
                }))
                .style(button::text)
                .padding([8, 12])
                .on_press(Message::TogglePlanDetails(p.id.clone())),
            ]
            .spacing(8)
            .wrap(),
        ]
        .spacing(12);
        // Do not construct the potentially large lists on every redraw of a collapsed card.
        if expanded {
            let mut sources = column![text("SURSE INCLUSE").size(11).color(MUTED)]
                .spacing(6)
                .width(Fill);
            for path in &p.sources {
                sources = sources.push(raw_text(path.display().to_string()).size(14));
            }
            sources = sources.push(
                text(
                    "Folderele includ toate subfolderele și fișierele, cu excluderile de mai jos.",
                )
                .size(12)
                .color(MUTED),
            );
            sources = sources.push(
                text(format!(
                    "Reguli de excludere: {}",
                    if p.excludes.is_empty() {
                        tr("niciuna")
                    } else {
                        p.excludes.join("; ")
                    }
                ))
                .size(13)
                .color(MUTED),
            );
            for path in &p.excluded_paths {
                sources = sources.push(
                    text(format!("Exclus: {}", path.display()))
                        .size(13)
                        .color(MUTED),
                );
            }
            body = body
                .push(horizontal_rule(1))
                .push(text(format!("MOD: {}", p.mode)).size(14).color(BLUE))
                .push(text(retention_label(p.keep_last)).size(13).color(MUTED))
                .push(
                    text(if p.use_vss {
                        "VSS: activat — fișiere în folosință"
                    } else {
                        "VSS: dezactivat"
                    })
                    .size(13)
                    .color(MUTED),
                )
                .push(text(format!("DESTINAȚIE: {}", p.destination_label())).size(14))
                .push(
                    container(
                        scrollable(sources)
                            .id(format!("plan-details-{}", p.id))
                            .width(Fill)
                            .height(Length::Shrink),
                    )
                    .width(Fill)
                    .max_height(220),
                )
                .push(
                    column![
                        text("ULTIMA REUȘITĂ").size(11).color(MUTED),
                        text(last).size(14)
                    ]
                    .spacing(4),
                );
            if let Some(remote) = &p.remote
                && let Ok(path) = remote.root(&p.id)
            {
                body = body.push(text(path.clone()).size(13));
                body = body.push(
                    button(text("Copiază calea"))
                        .style(button::text)
                        .on_press(Message::CopyText(path)),
                );
            }
            if let Some(run) = latest {
                body = body.push(
                    column![
                        text(format!(
                            "ULTIMA ÎNCERCARE · {} · {}",
                            run.started
                                .with_timezone(&Local)
                                .format("%Y-%m-%d %H:%M:%S"),
                            run.outcome
                        ))
                        .size(13)
                        .color(
                            if matches!(
                                run.outcome.as_str(),
                                "Eroare" | "Incomplet" | "Întrerupt" | "Reușit cu avertisment"
                            ) {
                                Color::from_rgb(0.78, 0.22, 0.25)
                            } else {
                                INK
                            }
                        ),
                        text(&run.detail).size(14),
                        self.error_actions(run)
                    ]
                    .spacing(6),
                );
            }
            body = body.push(
                row![
                    button(text("Verifică"))
                        .style(button::text)
                        .on_press_maybe(self.available().then(|| Message::Check(p.id.clone()))),
                    button(text(if p.enabled {
                        "Oprește programarea"
                    } else {
                        "Activează programarea"
                    }))
                    .style(button::text)
                    .on_press_maybe(self.available().then(|| Message::Toggle(p.id.clone())))
                ]
                .spacing(8)
                .wrap(),
            );
            if p.mode != BackupMode::Encrypted {
                body = body.push(
                    button(text(if cfg!(windows) {
                        "Deschide copiile în Explorer"
                    } else {
                        "Deschide folderul de backup"
                    }))
                    .style(button::text)
                    .on_press(Message::OpenFolder(p.repository.clone())),
                );
            }
        }
        if self.delete_plan.as_ref() == Some(&p.id) {
            let mut confirmation = column![
                horizontal_rule(1),
                text(format!("Ștergi planul «{}»?", p.name)).size(19),
                text("Se elimină planul, programarea și parola salvată în aplicație. Istoricul, fișierele sursă și copiile de pe disc rămân intacte.").size(14),
                text(format!("Copiile rămân în: {}", p.destination_label())).size(14),
                text("După ștergere, planul nu mai apare la Restaurare. Importul planurilor șterse nu este disponibil.").size(14),
            ].spacing(12);
            if p.mode == BackupMode::Encrypted {
                confirmation = confirmation
                    .push(text("Pentru a deschide copiile criptate rămase pe disc vei avea nevoie de Restic și de parola depozitului.").size(14))
                    .push(checkbox(self.delete_acknowledged)
                        .label(tr("Am păstrat separat parola necesară copiilor criptate existente."))
                        .on_toggle(Message::DeleteAcknowledged));
            } else {
                confirmation = confirmation.push(
                    text(
                        "Copiile necriptate pot fi deschise și copiate în continuare din Explorer.",
                    )
                    .size(14),
                );
            }
            confirmation = confirmation.push(
                row![
                    button(text("Anulează"))
                        .style(button::secondary)
                        .on_press_maybe((!self.busy).then_some(Message::CancelDelete)),
                    button(text("Șterge doar planul"))
                        .style(button::danger)
                        .on_press_maybe(
                            (self.available()
                                && (p.mode != BackupMode::Encrypted || self.delete_acknowledged))
                                .then_some(Message::ConfirmDelete)
                        ),
                ]
                .spacing(12),
            );
            body = body.push(confirmation);
        } else if expanded {
            body = body.push(
                button(text("Șterge planul…"))
                    .style(button::danger)
                    .on_press_maybe(self.available().then(|| Message::AskDelete(p.id.clone()))),
            );
        }
        card(body)
    }
    fn restore(&self) -> Element<'_, Message> {
        let mut content = column![text("1. Alege planul").size(20)].spacing(15);
        for p in &self.state.plans {
            content = content.push(
                button(raw_text(&p.name))
                    .style(if self.restore_plan.as_ref() == Some(&p.id) {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .on_press_maybe(
                        self.available()
                            .then(|| Message::LoadSnapshots(p.id.clone())),
                    ),
            );
        }
        if self.busy {
            content = content.push(text("Se încarcă…").color(MUTED));
        }
        if self.restore_plan.is_some() && !self.busy {
            content = content.push(text("2. Alege versiunea").size(20));
            if self.snapshots.is_empty() {
                content = content.push(text("Nu există copii disponibile pentru acest plan."));
            }
            for s in &self.snapshots {
                let selected = self.selected_snapshot.as_ref() == Some(&s.id);
                content = content.push(
                    button(text(format!(
                        "{}    {}    {} surse",
                        if selected { "●" } else { "○" },
                        s.time.with_timezone(&Local).format("%Y-%m-%d · %H:%M:%S"),
                        s.paths.len()
                    )))
                    .style(if selected {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .on_press(Message::SelectSnapshot(s.id.clone()))
                    .padding([10, 16]),
                );
            }
        }
        if !self.snapshots.is_empty() {
            content = content.push(card(column![text("3. Alege locul de restaurare").size(20), text("Se creează un subfolder nou, iar fișierele recuperate sunt verificate. Fișierele existente rămân intacte.").size(14), button(text("Alege folderul…")).on_press(Message::BrowseRestore), text(self.restore_parent.as_ref().map(|p| p.display().to_string()).unwrap_or("Niciun folder selectat".into())).size(14), button(text("Restaurează copia selectată")).padding([12,18]).on_press_maybe((self.available() && self.selected_snapshot.is_some() && self.restore_parent.is_some()).then_some(Message::Restore))].spacing(14)));
        }
        content.into()
    }
    fn error_actions(&self, run: &Run) -> Element<'_, Message> {
        if run.finished.is_none() || run.outcome == "Reușit" {
            return column![].into();
        }
        let mut actions = row![
            button(text("Copiază mesajul"))
                .style(button::secondary)
                .on_press(Message::CopyText(tr(&run.detail)))
        ]
        .spacing(10);
        if let Some(path) = backup_error_path(run) {
            actions = actions.push(
                button(text("Copiază calea"))
                    .style(button::secondary)
                    .on_press(Message::CopyText(path.display().to_string())),
            );
            if self.state.plans.iter().any(|p| {
                p.id == run.plan_id
                    && straja::exclusions::validate_paths(&p.sources, std::slice::from_ref(&path))
                        .is_ok()
            }) {
                actions = actions.push(
                    button(text("Adaugă la excluderi…")).on_press_maybe(
                        (self.available() && self.wizard.is_none())
                            .then(|| Message::ExcludeErrorPath(run.id.clone())),
                    ),
                );
            }
        }
        actions.into()
    }
    fn history(&self) -> Element<'_, Message> {
        let mut content = column![text("Ultimele 100 de operații").color(MUTED)].spacing(14);
        for r in &self.state.history {
            content = content.push(card(
                column![
                    row![
                        raw_text(format!("{} · {}", tr(&r.operation), r.plan_name))
                            .size(18)
                            .width(Fill),
                        text(&r.outcome).color(if r.outcome == "Reușit" {
                            Color::from_rgb(0.12, 0.55, 0.40)
                        } else {
                            INK
                        })
                    ],
                    text(
                        r.started
                            .with_timezone(&Local)
                            .format("%Y-%m-%d · %H:%M:%S")
                            .to_string()
                    )
                    .size(13)
                    .color(MUTED),
                    text(&r.detail).size(14),
                    self.error_actions(r)
                ]
                .spacing(10),
            ));
        }
        if self.state.history.is_empty() {
            content = content.push(text("Operațiile vor apărea aici după primul backup."));
        }
        content.into()
    }
    fn update_card(&self, details: bool) -> Element<'_, Message> {
        let status = if self.update_checking {
            "Se verifică actualizările…".into()
        } else {
            match &self.update_result {
                Some(Ok(Some(update))) => {
                    format!("Straja Backup {} este disponibil.", update.version)
                }
                Some(Ok(None)) => "Nu există o versiune stabilă mai nouă disponibilă.".into(),
                Some(Err(error)) => error.message().into(),
                None => "Actualizări".into(),
            }
        };
        let mut content = column![text(status).size(18)].spacing(12);
        if let Some(error) = &self.update_error {
            content = content
                .push(
                    text("Actualizarea nu a reușit. Poți încerca din nou.")
                        .color(Color::from_rgb(0.78, 0.22, 0.25)),
                )
                .push(raw_text(error).size(12));
        }
        if let Some(Ok(Some(update))) = &self.update_result {
            if self.install_starting {
                content =
                    content.push(text("Se pregătește închiderea și instalarea actualizării…"));
            } else if self.install_waiting {
                content = content
                    .push(text(
                        "Actualizarea va porni după terminarea operației curente.",
                    ))
                    .push(button(text("Mai târziu")).on_press(Message::LaterUpdate));
            } else if self.install_confirmation {
                content = content.push(text("Straja se va închide, va instala actualizarea în același folder și se va redeschide. Planurile și copiile de backup se păstrează. Windows poate cere aprobarea de administrator."))
                    .push(row![button(text(if self.state.active.is_some() || self.busy || self.pending_run.is_some() { "Instalează după operația curentă" } else { "Confirmă închiderea și instalarea" })).on_press(Message::ConfirmInstallUpdate), button(text("Mai târziu")).on_press(Message::LaterUpdate)].spacing(10));
            } else if let Some(progress) = &self.download_progress {
                let received = progress.received.load(std::sync::atomic::Ordering::Relaxed);
                let total = update.installer.as_ref().map_or(1, |a| a.size).max(1);
                content = content
                    .push(text(format!(
                        "Se descarcă actualizarea: {}%",
                        received * 100 / total
                    )))
                    .push(progress_bar(0.0..=1.0, received as f32 / total as f32))
                    .push(button(text("Anulează descărcarea")).on_press(Message::CancelDownload));
            } else if self.prepared_update.is_some() {
                content = content
                    .push(text("Actualizarea a fost descărcată și verificată."))
                    .push(button(text("Închide și instalează")).on_press_maybe(
                        self.wizard.is_none().then_some(Message::AskInstallUpdate),
                    ));
                if self.wizard.is_some() {
                    content = content.push(text(
                        "Salvează sau închide formularul înainte de actualizare.",
                    ));
                }
            } else if update.installer.is_some() {
                content = content.push(
                    row![
                        button(text("Descarcă actualizarea")).on_press(Message::DownloadUpdate),
                        button(text("Mai târziu")).on_press(Message::LaterUpdate)
                    ]
                    .spacing(10),
                );
            } else {
                content = content.push(text(if cfg!(windows) { "Această versiune nu are încă un instalator verificabil pentru actualizare automată." } else { "Instalează pachetul pentru sistemul tău din pagina versiunii. Așteaptă terminarea backupurilor înainte de actualizare." }))
                    .push(button(text("Deschide pagina versiunii")).on_press(Message::OpenReleasePage));
            }
            if details && !update.notes.is_empty() {
                content = content.push(scrollable(raw_text(&update.notes).size(14)).height(180));
            }
        }
        if details {
            content = content.push(text(if cfg!(windows) { "Verificare automată la pornire și la fiecare 24 de ore. Descarcă și instalează direct din aplicație, când alegi tu." } else { "Verificare automată la pornire și la fiecare 24 de ore. Vei fi notificat când apare un pachet nou pentru sistemul tău." }).size(13));
            content = content.push(
                raw_text(format!(
                    "GitHub: {} · v{}",
                    updates::REPOSITORY,
                    env!("CARGO_PKG_VERSION")
                ))
                .size(12)
                .color(MUTED),
            );
            content = content.push(
                button(text("Verifică actualizările")).on_press_maybe(
                    (!self.update_checking
                        && self.download_progress.is_none()
                        && !self.install_starting
                        && !self.install_waiting
                        && self.update_attempt.elapsed() >= Duration::from_secs(60))
                    .then_some(Message::CheckUpdates),
                ),
            );
            content = content.push(
                text("Poți repeta verificarea după un minut.")
                    .size(12)
                    .color(MUTED),
            );
        }
        card(content)
    }
    fn about(&self) -> Element<'_, Message> {
        let content = column![
            self.update_card(true),
            creator_logo(301.0),
            button(text("Vizitează strajabackup.com")).style(button::text).on_press(Message::OpenProductSite),
            text("Creată de Weblike PRO").size(24),
            text("Un proiect Open-Source").size(19).color(BLUE),
            button(text("weblike.ro")).style(button::text).on_press(Message::OpenCreatorSite),
            text("Straja Backup este un proiect open-source, sub licență MIT. Interfața și agentul sunt scrise în Rust; Restic creează și verifică depozitele criptate.").size(16),
            horizontal_rule(1),
            text("Două moduri de rulare").size(19),
            text(if cfg!(windows) { "Agent în fundal: îl pornești din aplicație și continuă după închiderea ferestrei, până la oprirea Windows.\nServiciu Windows: pornește automat cu sistemul, sub același cont Windows. Instalarea este descrisă în README și scripts\\install-service.ps1." } else { "Agentul continuă după închiderea ferestrei. Poți activa pornirea automată la autentificare. Keyring-ul contului trebuie să fie deblocat pentru acces la parole." }).size(14),
            text(if cfg!(windows) { "Parolele sunt protejate prin Windows DPAPI pentru contul tău. Păstrează separat parola fiecărui depozit: ea este necesară pentru recuperare pe alt PC." } else { "Parolele sunt protejate prin keyring-ul contului. Păstrează separat parola depozitului pentru recuperare pe alt calculator." }).size(14),
            text("Numărul de copii păstrate se configurează în fiecare plan. Copiile vechi se elimină numai după un backup nou reușit și verificat. ZIP, TAR.GZ și 7z sunt arhive necriptate. Restaurarea recuperează întreaga copie selectată. Fișierele blocate sunt raportate; VSS nu este activat în această versiune.").size(14),
        ].spacing(18);
        #[cfg(unix)]
        let content = content.push(
            button(text("Pornește agentul automat la autentificare")).on_press_maybe(
                (!self.busy && self.state.active.is_none()).then_some(Message::EnableAutostart),
            ),
        );
        card(content)
    }
    fn remote_form<'a>(&'a self, w: &'a Wizard) -> Element<'a, Message> {
        let input = |label: &str, value: &str, field: u8, secret: bool, editable: bool| {
            text_input(&tr(label), value)
                .secure(secret)
                .padding(10)
                .on_input_maybe(
                    (editable && !self.busy)
                        .then_some(move |value| Message::RemoteField(field, value)),
                )
        };
        let editable = w.editing.is_none();
        let mut body =
            column![text("Destinațiile online folosesc backup criptat și incremental.").size(14)]
                .spacing(12);
        match w.remote.provider {
            Provider::Ftp => {
                body = body
                    .push(
                        row![
                            input("Server FTP", &w.remote.host, 0, false, editable),
                            input("Port", &w.remote_port, 1, false, editable)
                        ]
                        .spacing(12),
                    )
                    .push(input("Utilizator FTP", &w.remote.user, 2, false, editable))
                    .push(input(
                        "Parolă FTP",
                        &w.remote_credentials.password,
                        3,
                        true,
                        true,
                    ))
                    .push(
                        checkbox(w.remote.ftp_tls)
                            .label(tr("FTPS explicit (TLS)"))
                            .on_toggle_maybe(
                                (editable && !self.busy).then_some(Message::RemoteTls),
                            ),
                    );
                if !w.remote.ftp_tls {
                    body=body.push(text("FTP fără TLS transmite autentificarea fără criptare. Folosește FTPS dacă serverul permite.").size(13));
                }
            }
            Provider::S3 => {
                body = body
                    .push(input(
                        "Bucket Amazon S3 existent",
                        &w.remote.bucket,
                        4,
                        false,
                        editable,
                    ))
                    .push(input(
                        "Regiune AWS (de exemplu eu-central-1)",
                        &w.remote.region,
                        5,
                        false,
                        editable,
                    ))
                    .push(input(
                        "Access Key ID",
                        &w.remote_credentials.access_key,
                        7,
                        true,
                        true,
                    ))
                    .push(input(
                        "Secret Access Key",
                        &w.remote_credentials.secret_key,
                        8,
                        true,
                        true,
                    ));
            }
            Provider::GoogleDrive | Provider::Dropbox => {
                body = body.push(
                    button(text("Conectează contul în browser"))
                        .on_press_maybe((!self.busy).then_some(Message::ConnectRemote)),
                );
                if w.remote_credentials_changed {
                    body = body.push(text("Cont conectat."));
                }
            }
            Provider::Local => {}
        }
        body=body.push(input("Folder online (de exemplu StrajaBackup)",&w.remote.path,6,false,editable))
            .push(text("Fiecare plan folosește un subfolder propriu. Parola backupului este separată de autentificarea contului.").size(13))
            .push(text("Limită transfer KiB/s (0 = fără limită)").size(14))
            .push(input("KiB/s",&w.remote_bandwidth,9,false,true));
        if !editable {
            body=body.push(text("Lasă autentificarea necompletată pentru a păstra datele salvate. Pentru altă destinație creează un plan nou.").size(13));
        }
        body = body.push(
            button(text("Testează conexiunea"))
                .on_press_maybe(self.available().then_some(Message::TestRemote)),
        );
        if !w.remote_notice.is_empty() {
            body = body.push(text(&w.remote_notice).size(14));
        }
        body.into()
    }
    fn wizard_view<'a>(&'a self, w: &'a Wizard) -> Element<'a, Message> {
        let steps = ["Ce salvezi", "Unde salvezi", "Când", "Confirmare"];
        let mut content = column![
            row![
                text(if w.editing.is_some() {
                    "Editează planul"
                } else {
                    "Plan nou"
                })
                .size(25)
                .width(Fill),
                button(text("Închide"))
                    .style(button::text)
                    .on_press_maybe((!self.busy).then_some(Message::CloseWizard))
            ],
            text(format!(
                "PASUL {} DIN 4  ·  {}",
                w.step + 1,
                steps[w.step as usize]
            ))
            .size(13)
            .color(BLUE),
            horizontal_rule(1)
        ]
        .spacing(20);
        match w.step {
            0 => {
                content = content.push(
                    text_input(&tr("Numele planului, de exemplu Documentele mele"), &w.name)
                        .on_input(Message::Name)
                        .padding(12),
                );
                content = content.push(
                    row![
                        button(text("+ Adaugă foldere")).on_press(Message::BrowseSources(true)),
                        button(text("+ Adaugă fișiere"))
                            .style(button::secondary)
                            .on_press(Message::BrowseSources(false))
                    ]
                    .spacing(12),
                );
                if w.sources.is_empty() {
                    content = content.push(
                        text("Folderele includ automat toate subfolderele și fișierele noi.")
                            .color(MUTED),
                    );
                }
                for (i, p) in w.sources.iter().enumerate() {
                    content = content.push(
                        row![
                            raw_text(p.display().to_string()).size(14).width(Fill),
                            button(text("Elimină"))
                                .style(button::text)
                                .on_press(Message::RemoveSource(i))
                        ]
                        .spacing(10),
                    );
                }
                content = content
                    .push(horizontal_rule(1))
                    .push(text("Ce NU se include în backup").size(19))
                    .push(text("Selectează subfoldere sau fișiere din sursele de mai sus. Un folder exclus este omis cu tot conținutul său, inclusiv fișierele adăugate ulterior. Excluderile se aplică backupurilor viitoare; nu modifică sursele sau conținutul copiilor existente.").size(13).color(MUTED))
                    .push(row![
                        button(text("+ Exclude foldere")).style(button::secondary).on_press_maybe((!w.sources.is_empty()).then_some(Message::BrowseExclusions(true))),
                        button(text("+ Exclude fișiere")).style(button::secondary).on_press_maybe((!w.sources.is_empty()).then_some(Message::BrowseExclusions(false)))
                    ].spacing(12));
                content = content
                    .push(text(if w.exclusion_edit.is_some() { "Editează calea exclusă" } else { "Sau introdu manual calea completă" }).size(14))
                    .push(text_input(&tr("C:/laragon/www/example-suite/public/build"), &w.exclusion_input)
                        .on_input(Message::ManualExclusion).on_submit(Message::SaveExclusion).padding(10))
                    .push(row![
                        button(text(if w.exclusion_edit.is_some() { "Salvează calea" } else { "Adaugă excluderea" }))
                            .on_press_maybe((!w.sources.is_empty() && !w.exclusion_input.trim().is_empty()).then_some(Message::SaveExclusion)),
                        button(text("Anulează")).style(button::text).on_press_maybe((!w.exclusion_input.is_empty() || w.exclusion_edit.is_some()).then_some(Message::CancelExclusion))
                    ].spacing(12))
                    .push(text("Acceptă / și \\. Calea este păstrată exact, fără a urma legăturile/junction-urile. Poți introduce și un folder care nu există momentan.").size(12).color(MUTED));
                if let Some(i) = w.exclusion_edit {
                    content = content.push(
                        button(text("Elimină excluderea"))
                            .style(button::danger)
                            .on_press_maybe((!self.busy).then_some(Message::RemoveExclusion(i))),
                    );
                }
                for (i, path) in w.excluded_paths.iter().enumerate() {
                    content = content.push(
                        row![
                            raw_text(path.display().to_string()).size(14).width(Fill),
                            button(text("Editează"))
                                .style(button::text)
                                .on_press(Message::EditExclusion(i)),
                            button(text("Elimină excluderea"))
                                .style(button::danger)
                                .on_press_maybe(
                                    (!self.busy).then_some(Message::RemoveExclusion(i))
                                )
                        ]
                        .spacing(10),
                    );
                }
                content = content
                    .push(text("Eliminarea unei excluderi include din nou calea în backupurile viitoare. Fișierele nu sunt șterse. Modificările se aplică după Salvează planul.").size(13).color(MUTED))
                    .push(text("Reguli avansate opționale, separate prin ;").size(13))
                    .push(
                        text_input(&tr("De exemplu: *.tmp; node_modules; target"), &w.excludes)
                            .on_input(Message::Excludes)
                            .padding(10),
                    );
            }
            1 => {
                content = content.push(text("Destinația backupului").size(20));
                if w.editing.is_none() {
                    content = content.push(pick_list(
                        Provider::ALL,
                        Some(w.remote.provider),
                        Message::RemoteProvider,
                    ));
                } else {
                    content = content.push(text(w.remote.provider.to_string()));
                }
                if w.remote.provider != Provider::Local {
                    content = content.push(self.remote_form(w));
                    if w.editing.is_none() {
                        content = content
                            .push(text("Parola depozitului criptat").size(16))
                            .push(
                                text_input(&tr("Minimum 8 caractere"), &w.password)
                                    .secure(true)
                                    .on_input(Message::Password)
                                    .padding(12),
                            )
                            .push(
                                text_input(&tr("Repetă parola"), &w.confirm_password)
                                    .secure(true)
                                    .on_input(Message::ConfirmPassword)
                                    .padding(12),
                            );
                    }
                } else {
                    if w.editing.is_none() {
                        content =
                            content
                                .push(text("Cum păstrezi copiile?").size(20))
                                .push(pick_list(
                                    [
                                        BackupMode::Encrypted,
                                        BackupMode::Plain,
                                        BackupMode::Zip,
                                        BackupMode::TarGz,
                                        BackupMode::SevenZip,
                                    ],
                                    Some(w.mode),
                                    Message::Mode,
                                ));
                    } else {
                        content = content.push(text(format!("Mod: {}", w.mode)).size(16));
                    }
                    if w.mode != BackupMode::Encrypted {
                        content = content.push(text(if w.mode == BackupMode::Plain { "Fișiere normale, fără parolă. Le poți deschide și copia direct din Explorer. Fiecare rulare creează o copie completă într-un folder datat." } else { "Arhivă comprimată, fără parolă, pentru fiecare rulare. O poți deschide sau extrage cu un program compatibil, fără Straja. Compresia folosește temporar și spațiu pentru o copie completă necomprimată pe destinație." }).size(15))
                        .push(text("Oricine are acces la discul de backup poate citi aceste fișiere.").size(13).color(MUTED))
                        .push(button(text("Alege destinația…")).on_press(Message::BrowseRepo))
                        .push(text(w.repository.as_ref().map(|p| p.display().to_string()).unwrap_or("Alege un folder gol dedicat planului.".into())).size(14));
                    } else {
                        if w.editing.is_some() {
                            content = content.push(text("Destinația planului").size(20))
                        .push(text(w.repository.as_ref().map(|p| p.display().to_string()).unwrap_or_default()))
                        .push(button(text("Alege altă destinație…")).on_press(Message::BrowseRepo))
                        .push(text("Poți corecta destinația înainte de inițializarea primului backup. După inițializare, creează un plan nou pentru alt depozit. Parola existentă se păstrează, fără să fie afișată.").size(14).color(MUTED));
                        } else {
                            content = content.push(text("Alege un folder gol dedicat acestui plan, pe discul de backup sau în rețea.").size(16))
                    .push(button(text("Alege destinația…")).on_press(Message::BrowseRepo))
                    .push(text(w.repository.as_ref().map(|p| p.display().to_string()).unwrap_or("Nicio destinație selectată".into())).size(14))
                    .push(text("Parola depozitului criptat").size(16))
                    .push(text_input(&tr("Minimum 8 caractere"), &w.password).secure(true).on_input(Message::Password).padding(12))
                    .push(text_input(&tr("Repetă parola"), &w.confirm_password).secure(true).on_input(Message::ConfirmPassword).padding(12))
                    .push(text("Păstrează parola și în afara acestui PC. Aplicația o salvează protejat pentru rulările automate.").size(13).color(MUTED));
                        }
                    }
                    if w.editing.is_some() {
                        content = content.push(text("Pentru schimbarea formatului de backup, creează un plan nou. Copiile existente rămân în formatul inițial.").size(13).color(MUTED));
                    }
                }
            }
            2 => {
                content = content
                    .push(text("Când vrei să ruleze backupul?").size(20))
                    .push(pick_list(
                        [
                            Frequency::Manual,
                            Frequency::Interval,
                            Frequency::Daily,
                            Frequency::Weekly,
                        ],
                        Some(w.frequency),
                        Message::Frequency,
                    ));
                if w.frequency == Frequency::Interval {
                    content = content.push(
                        text_input(&tr("Interval în minute (minimum 15)"), &w.interval)
                            .on_input(Message::Interval)
                            .padding(12),
                    );
                }
                if w.frequency == Frequency::Weekly {
                    content = content.push(pick_list(
                        [
                            Day::Mon,
                            Day::Tue,
                            Day::Wed,
                            Day::Thu,
                            Day::Fri,
                            Day::Sat,
                            Day::Sun,
                        ],
                        Some(w.day),
                        Message::Day,
                    ));
                }
                if matches!(w.frequency, Frequency::Daily | Frequency::Weekly) {
                    content = content
                        .push(text("Ora locală a calculatorului (HH:MM)").size(14))
                        .push(
                            text_input(&tr("19:00"), &w.time)
                                .on_input(Message::Time)
                                .padding(12),
                        );
                }
                if cfg!(windows) {
                    content = content
                    .push(horizontal_rule(1))
                    .push(checkbox(w.use_vss).label(tr("Backup pentru fișiere în folosință (Windows VSS)")).on_toggle(Message::UseVss))
                    .push(text("VSS necesită agent cu drepturi de administrator și surse pe discuri locale NTFS. Nu include modificările nesalvate și nu înlocuiește backupul nativ al bazelor de date.").size(14).color(MUTED));
                }
                if w.use_vss && !self.state.vss_available {
                    content = content.push(self.vss_activation());
                }
                if w.use_vss && self.state.vss_available {
                    content =
                        content.push(text("Agentul are drepturile necesare pentru VSS.").size(14));
                }
                content = content
                    .push(horizontal_rule(1))
                    .push(text("Câte copii păstrezi?").size(20))
                    .push(
                        checkbox(w.keep_all)
                            .label(tr("Păstrează toate copiile"))
                            .on_toggle(Message::KeepAll),
                    );
                if !w.keep_all {
                    content = content.push(text_input(&tr("Număr total de copii (1–10000)"), &w.keep_count).on_input(Message::KeepCount).padding(12))
                        .push(text("Numărul include cea mai nouă copie. Copiile mai vechi se șterg automat numai după un backup nou reușit și verificat. Modificarea limitei se aplică la următorul backup.").size(14).color(MUTED));
                }
                content = content.push(text("Dacă PC-ul este oprit, programarea se recuperează când agentul revine. O destinație indisponibilă se reîncearcă după 15 minute.").size(14).color(MUTED));
            }
            _ => {
                content = content
                    .push(raw_text(&w.name).size(24))
                    .push(text(format!("{} surse selectate", w.sources.len())))
                    .push(text(format!(
                        "Destinație: {}",
                        if w.remote.provider != Provider::Local {
                            w.remote.label()
                        } else {
                            w.repository
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default()
                        }
                    )))
                    .push(text(format!(
                        "Program: {}",
                        w.schedule().map(|s| s.label()).unwrap_or_default()
                    )))
                    .push(
                        text(format!(
                            "{} · {} · Backupul se pornește din pagina planului",
                            w.mode,
                            retention_label(w.retention().unwrap_or(None))
                        ))
                        .size(14)
                        .color(MUTED),
                    );
                content = content.push(text(if w.use_vss {
                    "VSS: activat — fișiere în folosință"
                } else {
                    "VSS: dezactivat"
                }));
                for source in &w.sources {
                    content = content.push(raw_text(source.display().to_string()).size(14));
                }
                if !w.excluded_paths.is_empty() || !w.excludes.trim().is_empty() {
                    content = content.push(text("Excluderi pentru următoarele backupuri").size(16));
                    for path in &w.excluded_paths {
                        content = content
                            .push(raw_text(path.display().to_string()).size(14).color(MUTED));
                    }
                    if !w.excludes.trim().is_empty() {
                        content = content.push(
                            text(format!("Reguli: {}", w.excludes))
                                .size(14)
                                .color(MUTED),
                        );
                    }
                }
                if w.editing.is_none() && w.mode == BackupMode::Encrypted {
                    content = content.push(
                        checkbox(w.acknowledged)
                            .label(tr("Am păstrat parola într-un loc sigur."))
                            .on_toggle(Message::Acknowledge),
                    );
                }
            }
        }
        if !w.error.is_empty() {
            content = content.push(text(&w.error).color(Color::from_rgb(0.78, 0.22, 0.25)));
        }
        if w.editing.is_some() && w.step == 0 {
            content = content.push(
                button(text(if self.busy {
                    "Se salvează…"
                } else {
                    "Salvează planul"
                }))
                .padding([12, 22])
                .on_press_maybe((!self.busy).then_some(Message::Save)),
            );
        }
        content = content.push(horizontal_rule(1)).push(row![
            button(text("Înapoi"))
                .style(button::secondary)
                .on_press_maybe((w.step > 0 && !self.busy).then_some(Message::Back)),
            iced::widget::Space::new().width(Fill),
            button(text(if w.step == 3 {
                if self.busy {
                    "Se salvează…"
                } else {
                    "Salvează planul"
                }
            } else {
                "Continuă"
            }))
            .padding([12, 22])
            .on_press_maybe((!self.busy).then_some(if w.step == 3 {
                Message::Save
            } else {
                Message::Next
            }))
        ]);
        card(content)
    }
}
// Recognize only source-error messages with known delimiters, including saved
// history from earlier versions. Never pick the first drive path: it can be the
// partial backup destination. Preserve the literal junction path, without I/O.
fn backup_error_path(run: &Run) -> Option<PathBuf> {
    if run.operation != "Backup"
        || run.finished.is_none()
        || !matches!(run.outcome.as_str(), "Eroare" | "Incomplet")
    {
        return None;
    }
    let candidate = if let Some((_, tail)) = run
        .detail
        .rsplit_once("Legătură/junction/reparse point neacceptat: ")
    {
        tail.strip_suffix(". Alege folderul real sau exclude legătura.")?
    } else if let Some((_, tail)) = run
        .detail
        .rsplit_once("Fișierul nu poate fi citit (posibil blocat): ")
    {
        tail.split_once(": ").map_or(tail, |(path, _)| path)
    } else {
        return None;
    };
    let path = PathBuf::from(candidate);
    (path.is_absolute() && !candidate.contains(['\0', '\r', '\n'])).then_some(path)
}

fn compact_path(path: String) -> Element<'static, Message> {
    let chars: Vec<char> = path.chars().collect();
    let label = if chars.len() > 48 {
        format!(
            "{}…{}",
            chars[..24].iter().collect::<String>(),
            chars[chars.len() - 23..].iter().collect::<String>()
        )
    } else {
        path.clone()
    };
    tooltip(
        raw_text(label)
            .size(14)
            .width(Fill)
            .wrapping(iced::widget::text::Wrapping::None),
        container(raw_text(path).size(14))
            .max_width(600)
            .padding(8)
            .style(container::rounded_box),
        tooltip::Position::Bottom,
    )
    .delay(Duration::from_millis(400))
    .into()
}

fn card<'a>(body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(body)
        .padding(22)
        .width(Fill)
        .style(surface)
        .into()
}
fn metric(label: &str, value: String) -> Element<'_, Message> {
    container(
        column![
            text(label).size(11).color(MUTED),
            text(value).size(34).color(BLUE)
        ]
        .spacing(10),
    )
    .width(Length::FillPortion(1))
    .padding(22)
    .style(surface)
    .into()
}

fn surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::WHITE.into()),
        border: iced::Border {
            color: Color::from_rgb(0.86, 0.89, 0.94),
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn online_wizard_keeps_numeric_edits_and_requires_separate_credentials() {
        let (mut app, _) = App::new();
        app.wizard = Some(Wizard::default());
        let _ = app.update(Message::RemoteProvider(Provider::Ftp));
        let _ = app.update(Message::RemoteField(1, String::new()));
        assert_eq!(app.wizard.as_ref().unwrap().remote_port, "");
        assert_eq!(app.wizard.as_ref().unwrap().remote.port, 0);
        let _ = app.update(Message::RemoteField(1, "2121".into()));
        let _ = app.update(Message::RemoteField(9, "512".into()));
        let w = app.wizard.as_mut().unwrap();
        w.step = 1;
        w.remote.host = "ftp.example.com".into();
        w.remote.user = "user".into();
        assert!(w.validate_step().is_err());
        w.remote_credentials.password = "provider-password".into();
        assert!(w.validate_step().is_err());
        w.password = "backup-password".into();
        w.confirm_password = w.password.clone();
        assert!(w.validate_step().is_ok());
        assert_eq!(w.remote.bandwidth_kib, 512);
        w.remote_bandwidth = String::new();
        w.remote.bandwidth_kib = u32::MAX;
        assert!(w.validate_step().is_err());
    }
    use super::*;
    #[test]
    fn compact_plan_status_and_read_only_details_survive_refresh() {
        let (mut app, _) = App::new();
        let mut run = failed_backup(r"C:\Example\cache");
        let mut plan = Plan {
            remote: None,
            id: run.plan_id.clone(),
            name: "Example".into(),
            mode: BackupMode::Plain,
            sources: vec![PathBuf::from(r"C:\Example")],
            repository: PathBuf::from(r"E:\Backups"),
            excluded_paths: (0..1000)
                .map(|n| PathBuf::from(format!(r"C:\Example\cache-{n}")))
                .collect(),
            excludes: vec!["*.tmp".into()],
            keep_last: Some(7),
            use_vss: false,
            schedule: Schedule::Manual,
            enabled: true,
            next_due: None,
            last_success: None,
        };
        assert_eq!(app.plan_status(&plan).0, tr("Nerulat"));
        plan.last_success = Some(run.started - chrono::Duration::days(1));
        assert_eq!(app.plan_status(&plan).0, tr("Reușit"));
        app.state.history.push(run.clone());
        assert_eq!(
            app.plan_status(&plan).0,
            tr("Eroare"),
            "a later failure must not be hidden by an older success"
        );
        assert_eq!(app.plan_status(&plan).1, run.finished);
        run.operation = "Verificare".into();
        run.outcome = "Reușit".into();
        app.state.history.insert(0, run.clone());
        assert_eq!(
            app.plan_status(&plan).0,
            tr("Eroare"),
            "verification does not replace the backup outcome"
        );
        app.pending_run = Some(run);
        assert!(app.plan_status(&plan).0.contains(&tr("Se pregătește…")));
        app.state.active = Some(Progress {
            plan_id: plan.id.clone(),
            operation: "Backup".into(),
            ..Progress::default()
        });
        assert_eq!(
            app.plan_status(&plan).0,
            format!("{} · {}", tr("Backup"), tr("În curs"))
        );
        app.pending_run = None;
        app.state.plans.push(plan.clone());
        let state_before = serde_json::to_value(&app.state).unwrap();
        let _ = app.update(Message::TogglePlanDetails(plan.id.clone()));
        assert_eq!(
            app.expanded_plan.as_ref(),
            Some(&plan.id),
            "details remain readable during an operation"
        );
        let _ = app.update(Message::Refresh(Ok(Response::State(app.state.clone()))));
        assert_eq!(app.expanded_plan.as_ref(), Some(&plan.id));
        assert_eq!(serde_json::to_value(&app.state).unwrap(), state_before);
        let _ = app.update(Message::TogglePlanDetails(plan.id.clone()));
        assert!(app.expanded_plan.is_none());
    }

    #[test]
    fn update_install_requires_confirmation_and_waits_for_operations() {
        let (mut app, _) = App::new();
        app.connected = true;
        app.state.agent_version = env!("CARGO_PKG_VERSION").into();
        app.prepared_update = Some(updates::PreparedUpdate {
            path: PathBuf::from("test-only-installer.exe"),
            update: Update {
                version: "1.0.0".into(),
                notes: String::new(),
                page_url: String::new(),
                installer: None,
            },
        });
        let _ = app.update(Message::ConfirmInstallUpdate);
        assert!(!app.install_starting && !app.install_waiting);
        app.wizard = Some(Wizard::default());
        let _ = app.update(Message::AskInstallUpdate);
        assert!(!app.install_confirmation);
        assert!(app.wizard.is_some());
        app.wizard = None;
        app.state.active = Some(Progress::default());
        let _ = app.update(Message::AskInstallUpdate);
        assert!(app.install_confirmation);
        let _ = app.update(Message::ConfirmInstallUpdate);
        assert!(app.install_waiting && !app.install_starting);
        assert!(!app.available());
        let _ = app.update(Message::LaterUpdate);
        assert!(!app.install_waiting);
        let _ = app.update(Message::AskInstallUpdate);
        let _ = app.update(Message::ConfirmInstallUpdate);
        app.state.active = None;
        let _ = app.start_update(); // Dropped Task: no subprocess or IPC in this test.
        assert!(app.install_starting);
        let _ = app.update(Message::New);
        assert!(
            app.wizard.is_none(),
            "handoff must not discard a newly opened form"
        );
        let _ = app.update(Message::UpdaterLaunched(Err("helper failed".into())));
        assert!(!app.install_waiting && !app.install_starting);
        assert_eq!(app.update_error.as_deref(), Some("helper failed"));
        assert!(
            app.prepared_update.is_some(),
            "failed handoff can be retried"
        );
    }
    #[test]
    fn idle_polling_slows_down_but_operations_stay_responsive() {
        let (mut app, _) = App::new();
        assert_eq!(app.poll_interval(), Duration::from_secs(5));
        app.connected = true;
        assert_eq!(app.poll_interval(), Duration::from_secs(10));
        app.state.active = Some(Progress::default());
        assert_eq!(app.poll_interval(), Duration::from_secs(2));
        app.state.active = None;
        app.busy = true;
        assert_eq!(app.poll_interval(), Duration::from_secs(2));
    }
    #[test]
    fn update_notifications_preserve_backup_state_and_form() {
        let (mut app, _) = App::new();
        app.busy = true;
        app.notice = "Backup in progress".into();
        app.wizard = Some(Wizard::default());
        app.wizard.as_mut().unwrap().name = "Unsaved plan".into();
        let _ = app.update(Message::UpdatesChecked(Ok(Some(Update {
            version: "1.0.0".into(),
            installer: None,
            notes: "Changes".into(),
            page_url: "https://github.com/weblikero/straja-backup/releases/tag/v1.0.0".into(),
        }))));
        assert!(app.busy);
        assert_eq!(app.notice, "Backup in progress");
        assert_eq!(app.wizard.as_ref().unwrap().name, "Unsaved plan");
        assert!(!app.update_checking);
        let _ = app.update(Message::CheckUpdates);
        assert!(!app.update_checking, "manual checks have a cooldown");
        let _ = app.update(Message::UpdatesChecked(Err(CheckError::Network)));
        assert!(!app.error, "update errors do not become backup errors");
        assert_eq!(app.notice, "Backup in progress");
    }
    fn failed_backup(path: &str) -> Run {
        Run {
            id: "failed-run".into(),
            plan_id: "laragon".into(),
            plan_name: "Laragon".into(),
            operation: "Backup".into(),
            started: chrono::Utc::now(),
            finished: Some(chrono::Utc::now()),
            outcome: "Eroare".into(),
            detail: format!(
                r"Backup nefinalizat. Copia parțială se află în \\?\E:\bkp_laragon\.in-progress-Backup-20261018-190414-test; versiunile anterioare sunt intacte.: Legătură/junction/reparse point neacceptat: {path}. Alege folderul real sau exclude legătura."
            ),
        }
    }

    #[cfg(windows)]
    #[test]
    fn error_path_uses_source_not_partial_destination_and_preserves_literal_names() {
        for path in [
            r"C:\laragon\www\example-project\public\storage",
            r"\\server\share\folder cu spații și [1]; punct.txt",
            r"\\?\C:\laragon\public\storage",
            "C:/laragon/www/build",
        ] {
            assert_eq!(
                backup_error_path(&failed_backup(path)),
                Some(PathBuf::from(path))
            );
        }
        let mut run = failed_backup(r"C:\laragon\file.txt");
        run.detail = r"Backup nefinalizat în E:\backup: Fișierul nu poate fi citit (posibil blocat): C:\laragon\file.txt: Access is denied. (os error 5)".into();
        assert_eq!(
            backup_error_path(&run),
            Some(PathBuf::from(r"C:\laragon\file.txt"))
        );
        for operation in ["Restaurare", "Verificare"] {
            let mut run = failed_backup(r"C:\laragon\file.txt");
            run.operation = operation.into();
            assert!(backup_error_path(&run).is_none());
        }
        for path in ["relative/path", "C:\\folder\nline", "C:\\folder\0name"] {
            assert!(backup_error_path(&failed_backup(path)).is_none());
        }
        for detail in [
            r"Nu pot scrie în E:\backup",
            "Legătură/junction/reparse point neacceptat: C:\\x (text incomplet)",
        ] {
            run.detail = detail.into();
            assert!(backup_error_path(&run).is_none());
        }
        run = failed_backup(r"C:\laragon\file.txt");
        run.outcome = "Reușit cu avertisment".into();
        assert!(backup_error_path(&run).is_none());
        run.outcome = "Eroare".into();
        run.finished = None;
        assert!(backup_error_path(&run).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn error_exclusion_opens_editable_draft_and_preserves_plan_until_saved() {
        let (mut app, _) = App::new();
        app.connected = true;
        app.state.agent_version = env!("CARGO_PKG_VERSION").into();
        let path = r"C:\laragon\www\example-project\public\storage";
        let run = failed_backup(path);
        app.state.history.push(run.clone());
        let plan = Plan {
            remote: None,
            mode: BackupMode::Plain,
            id: run.plan_id.clone(),
            name: run.plan_name.clone(),
            sources: vec![PathBuf::from(r"C:\laragon")],
            repository: PathBuf::from(r"E:\backup"),
            excluded_paths: vec![PathBuf::from(r"C:\laragon\temp")],
            excludes: vec!["*.tmp".into()],
            keep_last: Some(5),
            use_vss: false,
            schedule: Schedule::Daily {
                hour: 19,
                minute: 0,
            },
            enabled: true,
            next_due: None,
            last_success: None,
        };
        app.state.plans.push(plan.clone());
        let _ = app.update(Message::ExcludeErrorPath(run.id.clone()));
        let w = app.wizard.as_ref().unwrap();
        assert_eq!(w.exclusion_input, path);
        assert_eq!(w.editing.as_ref(), Some(&plan.id));
        assert_eq!(w.excluded_paths, plan.excluded_paths);
        assert!(
            w.validate_step().is_err(),
            "Draft must be accepted or cancelled first"
        );
        let edited = "C:/laragon/www/example-project/public/build";
        let _ = app.update(Message::ManualExclusion(edited.into()));
        let _ = app.update(Message::ExcludeErrorPath(run.id.clone()));
        assert_eq!(
            app.wizard.as_ref().unwrap().exclusion_input,
            edited,
            "Do not overwrite an open draft"
        );
        let _ = app.update(Message::SaveExclusion);
        let w = app.wizard.as_ref().unwrap();
        assert!(w.error.is_empty());
        assert_eq!(w.excluded_paths[1], PathBuf::from(edited));
        assert_eq!(w.excludes, "*.tmp");
        assert_eq!(w.retention().unwrap(), Some(5));
        assert_eq!(w.schedule().unwrap(), plan.schedule);
        assert_eq!(app.state.plans[0].excluded_paths, plan.excluded_paths);
        let _ = app.update(Message::CloseWizard);
        assert_eq!(app.state.plans[0].excluded_paths, plan.excluded_paths);
        // Existing exclusions reopen as edits, regardless of slash/case style.
        app.state.plans[0].excluded_paths.push(PathBuf::from(
            "c:/LARAGON/www/example-project/public/storage",
        ));
        let _ = app.update(Message::ExcludeErrorPath(run.id.clone()));
        assert_eq!(app.wizard.as_ref().unwrap().exclusion_edit, Some(1));
        let _ = app.update(Message::SaveExclusion);
        assert_eq!(app.wizard.as_ref().unwrap().excluded_paths.len(), 2);
        assert!(app.wizard.as_ref().unwrap().error.is_empty());
        let _ = app.update(Message::CloseWizard);
        app.busy = true;
        let _ = app.update(Message::ExcludeErrorPath(run.id.clone()));
        assert!(app.wizard.is_none());
        app.busy = false;
        app.connected = false;
        let _ = app.update(Message::ExcludeErrorPath(run.id.clone()));
        assert!(app.wizard.is_none());
        app.connected = true;
        app.state.plans[0].sources = vec![PathBuf::from(r"D:\changed-source")];
        let _ = app.update(Message::ExcludeErrorPath(run.id.clone()));
        assert!(
            app.wizard.is_none(),
            "Old errors cannot exclude outside current sources"
        );
        app.state.plans.clear();
        let _ = app.update(Message::ExcludeErrorPath(run.id));
        assert!(app.wizard.is_none());
    }

    #[test]
    fn failure_banner_retains_actions_across_poll_and_clears_on_unrelated_message() {
        let (mut app, _) = App::new();
        let run = failed_backup(r"C:\laragon\www\example-project\public\storage");
        app.state.history.push(run.clone());
        let _ = app.update(Message::Reply(Ok(Response::Started(run.clone()))));
        assert_eq!(app.notice_run.as_ref().map(|r| &r.id), Some(&run.id));
        app.update_run_notice();
        assert!(app.notice_run.is_some());
        let _ = app.update(Message::CopyText(tr(&run.detail)));
        assert!(
            app.notice_run.is_some(),
            "Copy must keep the error and its actions visible"
        );
        let _ = app.update(Message::Reply(Ok(Response::Error(
            "Agent indisponibil".into(),
        ))));
        assert!(
            app.notice_run.is_none(),
            "Do not associate the old source with another error"
        );
    }

    #[test]
    fn delete_requires_confirmation_and_cleans_restore_state_only_after_success() {
        let (mut app, _) = App::new();
        app.connected = true;
        app.state.agent_version = env!("CARGO_PKG_VERSION").into();
        let plan = Plan {
            remote: None,
            mode: BackupMode::Encrypted,
            id: "delete-test".into(),
            name: "Test".into(),
            sources: vec![PathBuf::from(r"C:\Data")],
            repository: PathBuf::from(r"E:\Backup"),
            excluded_paths: vec![],
            excludes: vec![],
            keep_last: None,
            use_vss: false,
            schedule: Schedule::Manual,
            enabled: true,
            next_due: None,
            last_success: None,
        };
        app.state.plans.push(plan.clone());
        let _ = app.update(Message::ConfirmDelete);
        assert!(!app.busy, "No deletion without selecting a plan");
        let _ = app.update(Message::AskDelete(plan.id.clone()));
        let _ = app.update(Message::ConfirmDelete);
        assert!(
            !app.busy,
            "Encrypted deletion needs password acknowledgement"
        );
        let _ = app.update(Message::DeleteAcknowledged(true));
        app.state.active = Some(Progress::default());
        let _ = app.update(Message::ConfirmDelete);
        assert!(!app.busy, "Cannot delete during an operation");
        app.state.active = None;
        let _ = app.update(Message::CancelDelete);
        assert!(app.delete_plan.is_none());
        assert!(!app.delete_acknowledged);
        assert_eq!(app.state.plans.len(), 1);
        app.state.plans[0].mode = BackupMode::Plain;
        let _ = app.update(Message::AskDelete(plan.id.clone()));
        let _ = app.update(Message::ConfirmDelete);
        assert!(app.busy, "Plain mode needs no password acknowledgement");
        app.restore_plan = Some(plan.id.clone());
        app.selected_snapshot = Some("snapshot".into());
        app.restore_parent = Some(PathBuf::from(r"E:\Restore"));
        app.wizard = Some(Wizard::from_plan(&plan));
        let _ = app.update(Message::Deleted(
            plan.id.clone(),
            Ok(Response::Error("Busy".into())),
        ));
        assert_eq!(app.state.plans.len(), 1);
        assert!(app.restore_plan.is_some() && app.error);
        let _ = app.update(Message::Deleted(
            plan.id,
            Ok(Response::Ok("Deleted".into())),
        ));
        assert!(app.state.plans.is_empty());
        assert!(
            app.restore_plan.is_none()
                && app.selected_snapshot.is_none()
                && app.restore_parent.is_none()
        );
        assert!(app.wizard.is_none() && app.delete_plan.is_none());
        assert!(!app.busy && !app.error);
    }
    #[test]
    fn plain_wizard_does_not_require_password_or_password_acknowledgement() {
        let tmp = tempfile::tempdir().unwrap();
        let mut w = Wizard::default();
        w.mode = BackupMode::Plain;
        w.repository = Some(tmp.path().to_path_buf());
        w.step = 1;
        assert!(w.validate_step().is_ok());
        w.step = 3;
        assert!(w.validate_step().is_ok());
        w.mode = BackupMode::Encrypted;
        assert!(w.validate_step().is_err());
    }
    #[test]
    fn fast_failure_replaces_acceptance_notice_even_if_poll_finished_first() {
        let (mut app, _) = App::new();
        let run = Run {
            id: "run-1".into(),
            plan_id: "plan-1".into(),
            plan_name: "Test".into(),
            operation: "Backup".into(),
            started: chrono::Utc::now(),
            finished: Some(chrono::Utc::now()),
            outcome: "Eroare".into(),
            detail: "Destinația nu este goală".into(),
        };
        app.state.history.push(run.clone());
        let _ = app.update(Message::Reply(Ok(Response::Started(run))));
        assert!(app.error);
        assert!(app.notice.contains("Destinația nu este goală"));
        assert!(app.pending_run.is_none());
    }
    #[test]
    fn edit_form_keeps_schedule_and_does_not_require_or_expose_password() {
        let p = Plan {
            remote: None,
            mode: BackupMode::Encrypted,
            id: "plan".into(),
            name: "Test".into(),
            sources: vec![PathBuf::from(r"C:\Data")],
            repository: PathBuf::from(r"E:\Backup"),
            excluded_paths: vec![PathBuf::from(r"C:\Data\FolderB")],
            excludes: vec!["*.tmp".into()],
            keep_last: Some(7),
            use_vss: false,
            schedule: Schedule::Weekly {
                weekday: 3,
                hour: 8,
                minute: 45,
            },
            enabled: false,
            next_due: None,
            last_success: None,
        };
        let mut w = Wizard::from_plan(&p);
        assert_eq!(w.schedule().unwrap(), p.schedule);
        assert_eq!(w.retention().unwrap(), Some(7));
        assert!(w.password.is_empty() && w.confirm_password.is_empty());
        assert_eq!(w.sources, p.sources);
        assert_eq!(w.excluded_paths, p.excluded_paths);
        w.step = 3;
        assert!(w.validate_step().is_ok());
    }
    #[test]
    fn archive_wizard_and_retention_validation() {
        let tmp = tempfile::tempdir().unwrap();
        for mode in [BackupMode::Zip, BackupMode::TarGz, BackupMode::SevenZip] {
            let mut w = Wizard::default();
            w.mode = mode;
            w.repository = Some(tmp.path().to_path_buf());
            w.step = 1;
            assert!(w.validate_step().is_ok());
            w.step = 3;
            assert!(w.validate_step().is_ok());
            assert_eq!(w.retention().unwrap(), None);
            w.keep_all = false;
            w.step = 2;
            for invalid in ["", "0", "10001", "abc", "1.5", "-2"] {
                w.keep_count = invalid.into();
                assert!(w.validate_step().is_err());
            }
            w.keep_count = "7".into();
            assert!(w.validate_step().is_ok());
            assert_eq!(w.retention().unwrap(), Some(7));
        }
    }
    #[cfg(windows)]
    #[test]
    fn exclusion_picker_validates_deduplicates_and_can_include_again() {
        let (mut app, _) = App::new();
        let mut w = Wizard::default();
        w.name = "Excluderi".into();
        w.sources = vec![PathBuf::from(r"C:\FolderA")];
        app.wizard = Some(w);
        let path = PathBuf::from(r"C:\FolderA\FolderB");
        let _ = app.update(Message::ExcludedPaths(vec![path.clone(), path.clone()]));
        assert_eq!(
            app.wizard.as_ref().unwrap().excluded_paths,
            vec![path.clone()]
        );
        let _ = app.update(Message::ExcludedPaths(vec![PathBuf::from(r"C:\Other")]));
        assert!(!app.wizard.as_ref().unwrap().error.is_empty());
        assert_eq!(app.wizard.as_ref().unwrap().excluded_paths, vec![path]);
        let _ = app.update(Message::RemoveSource(0));
        assert!(app.wizard.as_ref().unwrap().validate_step().is_err());
        let _ = app.update(Message::RemoveExclusion(0));
        assert!(app.wizard.as_ref().unwrap().excluded_paths.is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn manual_exclusion_add_edit_cancel_and_validation() {
        let (mut app, _) = App::new();
        let mut w = Wizard::default();
        w.name = "Laragon".into();
        w.sources = vec![PathBuf::from(r"C:\laragon")];
        app.wizard = Some(w);
        let literal = "C:/laragon/www/example-suite/public/build";
        let _ = app.update(Message::ManualExclusion(format!("  \"{literal}\"  ")));
        assert!(
            app.wizard.as_ref().unwrap().validate_step().is_err(),
            "Uncommitted input must not be silently lost"
        );
        let _ = app.update(Message::SaveExclusion);
        let w = app.wizard.as_ref().unwrap();
        assert!(w.error.is_empty(), "{}", w.error);
        assert_eq!(w.excluded_paths, vec![PathBuf::from(literal)]);
        assert!(w.validate_step().is_ok());
        let _ = app.update(Message::ManualExclusion(
            r"c:\LARAGON\www\example-suite\public\build".into(),
        ));
        let _ = app.update(Message::SaveExclusion);
        assert_eq!(app.wizard.as_ref().unwrap().excluded_paths.len(), 1);
        assert!(app.wizard.as_ref().unwrap().error.contains("deja"));
        let _ = app.update(Message::CancelExclusion);
        let _ = app.update(Message::EditExclusion(0));
        let _ = app.update(Message::ManualExclusion(r"D:\outside".into()));
        let _ = app.update(Message::SaveExclusion);
        assert!(!app.wizard.as_ref().unwrap().error.is_empty());
        assert_eq!(
            app.wizard.as_ref().unwrap().excluded_paths[0],
            PathBuf::from(literal)
        );
        let _ = app.update(Message::CancelExclusion);
        let _ = app.update(Message::EditExclusion(0));
        let _ = app.update(Message::ManualExclusion(
            r"C:\laragon\other[1];folder".into(),
        ));
        let _ = app.update(Message::SaveExclusion);
        assert!(app.wizard.as_ref().unwrap().error.is_empty());
        assert_eq!(
            app.wizard.as_ref().unwrap().excluded_paths,
            vec![PathBuf::from(r"C:\laragon\other[1];folder")]
        );
        assert!(app.wizard.as_ref().unwrap().exclusion_edit.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn manual_exclusion_edit_tracks_removals_and_preserves_pending_input() {
        let (mut app, _) = App::new();
        let mut w = Wizard::default();
        w.sources = vec![PathBuf::from(r"C:\A")];
        w.excluded_paths = vec![PathBuf::from(r"C:\A\one"), PathBuf::from(r"C:\A\two")];
        app.wizard = Some(w);
        let _ = app.update(Message::EditExclusion(1));
        let _ = app.update(Message::ManualExclusion(r"C:\A\edited".into()));
        let _ = app.update(Message::EditExclusion(0));
        assert_eq!(app.wizard.as_ref().unwrap().exclusion_input, r"C:\A\edited");
        let _ = app.update(Message::RemoveExclusion(0));
        assert_eq!(app.wizard.as_ref().unwrap().exclusion_edit, Some(0));
        let _ = app.update(Message::SaveExclusion);
        assert_eq!(
            app.wizard.as_ref().unwrap().excluded_paths,
            vec![PathBuf::from(r"C:\A\edited")]
        );
        let _ = app.update(Message::EditExclusion(0));
        let _ = app.update(Message::RemoveExclusion(0));
        let w = app.wizard.as_ref().unwrap();
        assert!(
            w.excluded_paths.is_empty()
                && w.exclusion_input.is_empty()
                && w.exclusion_edit.is_none()
        );
    }

    #[test]
    fn removed_exclusion_can_be_saved_directly_from_first_edit_step() {
        let (mut app, _) = App::new();
        let mut w = Wizard::default();
        w.editing = Some("existing-plan".into());
        w.name = "Existing plan".into();
        w.mode = BackupMode::Plain;
        w.sources = vec![PathBuf::from(r"C:\A")];
        w.repository = Some(PathBuf::from(r"E:\Backup"));
        w.excluded_paths = vec![PathBuf::from(r"C:\A\cache")];
        w.excludes = "*.tmp".into();
        app.wizard = Some(w);
        let _ = app.update(Message::EditExclusion(0));
        let _ = app.update(Message::ManualExclusion(String::new()));
        let _ = app.update(Message::RemoveExclusion(0));
        let w = app.wizard.as_ref().unwrap();
        assert_eq!(w.step, 0);
        assert!(w.validate_step().is_ok());
        assert!(w.excluded_paths.is_empty());
        assert_eq!(w.excludes, "*.tmp");
        // Construct the asynchronous save; do not contact the user's agent in a unit test.
        let _ = app.update(Message::Save);
        assert!(app.busy);
        assert!(app.wizard.as_ref().unwrap().error.is_empty());
        let _ = app.update(Message::Saved(Ok(Response::Error(
            "test save failure".into(),
        ))));
        let w = app.wizard.as_ref().unwrap();
        assert!(
            w.excluded_paths.is_empty(),
            "A failed save must keep the edited draft"
        );
        assert_eq!(w.error, "test save failure");
        let _ = app.update(Message::Saved(Ok(Response::Ok("saved".into()))));
        assert!(app.wizard.is_none());
    }
}
