//! Interactive-session tray companion. It monitors the worker; it never owns backups.
use crate::{
    model::State,
    protocol::{self, Request, Response},
};
use anyhow::{Context, Result, bail};
use std::{
    cell::RefCell,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM,
    },
    System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW},
    UI::{Shell::*, WindowsAndMessaging::*},
};

const CALLBACK: u32 = WM_APP + 17;
const KEYSELECT: u32 = NIN_SELECT | 1;
const OPEN: usize = 1;
const START: usize = 2;
const STOP: usize = 3;
const EXIT: usize = 4;

#[derive(Clone, Default)]
struct Monitor {
    state: Option<State>,
    checked: bool,
    command_pending: bool,
    command_error: Option<String>,
    close: bool,
}

impl Monitor {
    fn presentation(&self) -> (usize, String) {
        if let Some(error) = &self.command_error {
            return (2, format!("Straja · {error}"));
        }
        let Some(state) = &self.state else {
            return (
                3,
                if self.checked {
                    "Straja · Agent oprit / deconectat"
                } else {
                    "Straja · Se conectează la agent…"
                }
                .into(),
            );
        };
        if let Some(active) = &state.active {
            return (
                1,
                format!(
                    "Straja · {} în curs · {:.0}%",
                    active.operation,
                    active.percent * 100.
                ),
            );
        }
        if state.agent_version != env!("CARGO_PKG_VERSION") {
            return (
                2,
                "Straja · Agent vechi activ; repornește agentul pentru actualizare".into(),
            );
        }
        if let Some(run) = state.history.iter().find(|r| r.finished.is_some())
            && run.outcome != "Reușit"
        {
            return (
                2,
                format!(
                    "Straja · {}: {} · Deschide aplicația pentru detalii",
                    run.plan_name, run.outcome
                ),
            );
        }
        (
            0,
            format!(
                "Straja · {} activ · {} planuri",
                if state.service_mode {
                    "Serviciu"
                } else {
                    "Agent"
                },
                state.plans.len()
            ),
        )
    }
    fn can_stop(&self) -> bool {
        !self.command_pending
            && self.state.as_ref().is_some_and(|s| {
                !s.service_mode
                    && s.active.is_none()
                    && s.agent_version == env!("CARGO_PKG_VERSION")
            })
    }
}

pub fn launch() -> Result<()> {
    spawn_sibling("straja-agent.exe", &["--tray"])
}

pub fn show_existing_window() -> bool {
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    struct Search {
        expected: String,
        found: HWND,
    }
    unsafe extern "system" fn visit(hwnd: HWND, parameter: LPARAM) -> windows_sys::core::BOOL {
        unsafe {
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            let search = &mut *(parameter as *mut Search);
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return 1;
            }
            let mut buffer = [0u16; 32768];
            let mut size = buffer.len() as u32;
            let ok = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                buffer.as_mut_ptr(),
                &mut size,
            );
            CloseHandle(process);
            if ok != 0
                && String::from_utf16_lossy(&buffer[..size as usize]).to_lowercase()
                    == search.expected
            {
                search.found = hwnd;
                return 0;
            }
            1
        }
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut search = Search {
        expected: exe
            .with_file_name("straja.exe")
            .to_string_lossy()
            .to_lowercase(),
        found: std::ptr::null_mut(),
    };
    unsafe {
        EnumWindows(Some(visit), (&mut search as *mut Search) as LPARAM);
        if !search.found.is_null() {
            if IsIconic(search.found) != 0 {
                ShowWindow(search.found, SW_RESTORE);
            }
            SetForegroundWindow(search.found);
            return true;
        }
    }
    false
}

fn spawn_sibling(file: &str, args: &[&str]) -> Result<()> {
    use std::os::windows::process::CommandExt;
    Command::new(std::env::current_exe()?.with_file_name(file))
        .args(args)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Nu pot porni {file}"))?;
    Ok(())
}

fn wide(s: &str) -> Vec<u16> {
    crate::i18n::tr(s).encode_utf16().chain(Some(0)).collect()
}
fn copy_wide<const N: usize>(target: &mut [u16; N], text: &str) {
    target.fill(0);
    let mut offset = 0;
    for ch in text.chars() {
        let mut units = [0; 2];
        let encoded = ch.encode_utf16(&mut units);
        if offset + encoded.len() >= N {
            break;
        }
        target[offset..offset + encoded.len()].copy_from_slice(encoded);
        offset += encoded.len();
    }
}

struct Tray {
    menu_open: bool,
    data: NOTIFYICONDATAW,
    icons: [HICON; 4],
    added: bool,
    recreated: u32,
    monitor: Arc<Mutex<Monitor>>,
    smoke_deadline: Option<std::time::Instant>,
    ever_added: bool,
}
thread_local! { static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) }; }

#[cfg(test)]
#[derive(Default)]
struct PopupTest {
    init_with_free_state: bool,
    timer_in_menu: bool,
    nested_title_length: usize,
}
#[cfg(test)]
thread_local! { static POPUP_TEST: RefCell<Option<PopupTest>> = const { RefCell::new(None) }; }

impl Tray {
    fn refresh(&mut self) {
        let snapshot = self
            .monitor
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let (index, label) = snapshot.presentation();
        self.data.hIcon = self.icons[index];
        crate::i18n::reload();
        copy_wide(&mut self.data.szTip, &crate::i18n::tr(&label));
        // Explorer can start after us or restart. Retry registration until available.
        unsafe {
            if !self.added {
                self.added = Shell_NotifyIconW(NIM_ADD, &self.data) != 0;
                if self.added {
                    self.ever_added = true;
                    self.data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                    Shell_NotifyIconW(NIM_SETVERSION, &self.data);
                }
            } else if Shell_NotifyIconW(NIM_MODIFY, &self.data) == 0 {
                self.added = false;
            }
        }
    }
    fn menu(hwnd: HWND, snapshot: Monitor) -> usize {
        let label = snapshot.presentation().1.replace('&', "&&");
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return 0;
            }
            AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, wide(&label).as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(
                menu,
                MF_STRING,
                OPEN,
                wide("Deschide Straja Backup").as_ptr(),
            );
            AppendMenuW(
                menu,
                MF_STRING
                    | if snapshot.checked && snapshot.state.is_none() && !snapshot.command_pending {
                        0
                    } else {
                        MF_GRAYED
                    },
                START,
                wide("Pornește agentul").as_ptr(),
            );
            AppendMenuW(
                menu,
                MF_STRING | if snapshot.can_stop() { 0 } else { MF_GRAYED },
                STOP,
                wide("Oprește agentul și pictograma").as_ptr(),
            );
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(
                menu,
                MF_STRING,
                EXIT,
                wide("Închide doar pictograma (agentul continuă)").as_ptr(),
            );
            let mut point = POINT::default();
            GetCursorPos(&mut point);
            #[cfg(test)]
            if POPUP_TEST.with(|test| test.borrow().is_some()) {
                let mut rect = windows_sys::Win32::Foundation::RECT::default();
                GetWindowRect(hwnd, &mut rect);
                point.x = rect.left + 40;
                point.y = rect.top + 60;
            }
            SetForegroundWindow(hwnd);
            let action = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                0,
                hwnd,
                std::ptr::null(),
            ) as usize;
            DestroyMenu(menu);
            PostMessageW(hwnd, WM_NULL, 0, 0);
            action
        }
    }
    fn action(&mut self, action: usize) {
        let result = match action {
            OPEN => {
                if show_existing_window() {
                    Ok(())
                } else {
                    spawn_sibling("straja.exe", &[])
                }
            }
            START => spawn_sibling("straja-agent.exe", &[]),
            STOP => {
                let mut monitor = self.monitor.lock().unwrap_or_else(|e| e.into_inner());
                if !monitor.can_stop() {
                    return;
                }
                monitor.command_pending = true;
                monitor.command_error = None;
                drop(monitor);
                let shared = self.monitor.clone();
                std::thread::spawn(move || {
                    let result = tokio::runtime::Runtime::new()
                        .map_err(anyhow::Error::from)
                        .and_then(|rt| rt.block_on(protocol::call(Request::Shutdown)));
                    let mut monitor = shared.lock().unwrap_or_else(|e| e.into_inner());
                    monitor.command_pending = false;
                    match result {
                        Ok(Response::Ok(_)) => monitor.close = true,
                        Ok(Response::Error(e)) => monitor.command_error = Some(e),
                        Err(e) => monitor.command_error = Some(format!("{e:#}")),
                        _ => monitor.command_error = Some("Răspuns neașteptat de la agent".into()),
                    }
                });
                Ok(())
            }
            EXIT => {
                unsafe {
                    PostMessageW(self.data.hWnd, WM_CLOSE, 0, 0);
                }
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.monitor
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .command_error = Some(format!("{e:#}"));
        } else if action == START {
            self.monitor
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .command_error = None;
        }
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_DESTROY {
        unsafe {
            PostQuitMessage(0);
        }
        return 0;
    }
    // TrackPopupMenu runs a nested message loop. Never hold the RefCell borrow across it:
    // Windows sends menu drawing/theme/accessibility messages back to this same procedure.
    if message == CALLBACK && (l as u32 & 0xffff) == WM_CONTEXTMENU {
        let snapshot = TRAY.with(|slot| {
            let mut slot = slot.try_borrow_mut().ok()?;
            let tray = slot.as_mut()?;
            if tray.menu_open {
                return None;
            }
            tray.menu_open = true;
            Some(
                tray.monitor
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            )
        });
        if let Some(snapshot) = snapshot {
            let action = Tray::menu(hwnd, snapshot);
            TRAY.with(|slot| {
                if let Some(tray) = slot.borrow_mut().as_mut() {
                    tray.menu_open = false;
                    if tray.smoke_deadline.is_none() {
                        tray.action(action);
                    }
                }
            });
        }
        return 0;
    }
    #[cfg(test)]
    if message == WM_INITMENUPOPUP {
        let free = TRAY.with(|slot| slot.try_borrow_mut().is_ok());
        POPUP_TEST.with(|test| {
            if let Some(test) = test.borrow_mut().as_mut() {
                test.init_with_free_state = free;
            }
        });
    }
    let handled = TRAY.with(|slot| {
        // Preserve the native default handler even if another synchronous callback is reentrant.
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return None;
        };
        let tray = slot.as_mut()?;
        if message == tray.recreated {
            tray.added = false;
            tray.refresh();
            return Some(0);
        }
        match message {
            WM_TIMER => {
                #[cfg(test)]
                if tray.menu_open {
                    POPUP_TEST.with(|test| {
                        if let Some(test) = test.borrow_mut().as_mut() {
                            test.timer_in_menu = true;
                        }
                    });
                }
                let close = tray.monitor.lock().unwrap_or_else(|e| e.into_inner()).close
                    || tray
                        .smoke_deadline
                        .is_some_and(|d| std::time::Instant::now() >= d);
                if close {
                    unsafe {
                        if tray.menu_open {
                            EndMenu();
                        }
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                } else {
                    tray.refresh();
                }
                Some(0)
            }
            CALLBACK => {
                match (l as u32) & 0xffff {
                    NIN_SELECT | KEYSELECT if !tray.menu_open && tray.smoke_deadline.is_none() => {
                        tray.action(OPEN)
                    }
                    _ => {}
                }
                Some(0)
            }
            WM_CLOSE => {
                unsafe {
                    if tray.menu_open {
                        EndMenu();
                    }
                    PostQuitMessage(0);
                }
                Some(0)
            }
            _ => None,
        }
    });
    handled.unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, message, w, l) })
}

pub fn run() -> Result<()> {
    run_inner(None)
}

fn run_inner(smoke: Option<Duration>) -> Result<()> {
    let mutex_name = wide(&format!(
        "Local\\StrajaTray-{}{}",
        crate::security::user_sid()?,
        if smoke.is_some() { "-smoke" } else { "" }
    ));
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr()) };
    if mutex.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    struct MutexGuard(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for MutexGuard {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let _guard = MutexGuard(mutex);
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(());
    }

    let monitor = Arc::new(Mutex::new(Monitor::default()));
    if smoke.is_none() {
        let shared = monitor.clone();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Runtime::new() else {
                return;
            };
            loop {
                let state = match runtime.block_on(protocol::call(Request::State)) {
                    Ok(Response::State(s)) => Some(s),
                    _ => None,
                };
                {
                    let mut m = shared.lock().unwrap_or_else(|e| e.into_inner());
                    m.state = state;
                    m.checked = true;
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    }
    let class_name = wide("StrajaBackupTrayWindow");
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            bail!(
                "Nu pot înregistra fereastra pictogramei: {}",
                std::io::Error::last_os_error()
            );
        }
        // Hidden top-level window receives TaskbarCreated; a message-only window would not.
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide("Straja Backup — pictogramă").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            bail!("Nu pot crea fereastra pictogramei");
        }
        let images: [&[u8]; 4] = [
            include_bytes!("../installer/assets/tray-ready.png"),
            include_bytes!("../installer/assets/tray-busy.png"),
            include_bytes!("../installer/assets/tray-error.png"),
            include_bytes!("../installer/assets/tray-offline.png"),
        ];
        let icons = images.map(|bytes| {
            CreateIconFromResourceEx(
                bytes.as_ptr(),
                bytes.len() as u32,
                1,
                0x30000,
                32,
                32,
                LR_DEFAULTCOLOR,
            )
        });
        if icons.iter().any(|h| h.is_null()) {
            for h in icons {
                if !h.is_null() {
                    DestroyIcon(h);
                }
            }
            DestroyWindow(hwnd);
            UnregisterClassW(class_name.as_ptr(), instance);
            bail!("Pictogramele Straja nu au putut fi încărcate");
        }
        TRAY.with(|slot| {
            *slot.borrow_mut() = Some(Tray {
                menu_open: false,
                data: NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: 1,
                    uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
                    uCallbackMessage: CALLBACK,
                    ..Default::default()
                },
                icons,
                added: false,
                recreated: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                monitor,
                smoke_deadline: smoke.map(|d| std::time::Instant::now() + d),
                ever_added: false,
            });
            if let Some(tray) = slot.borrow_mut().as_mut() {
                tray.refresh();
            }
        });
        if SetTimer(hwnd, 1, 1000, None) == 0 {
            PostQuitMessage(1);
        }
        #[cfg(test)]
        if POPUP_TEST.with(|test| test.borrow().is_some()) {
            let length = TRAY.with(|slot| {
                let _borrow = slot.borrow_mut();
                SendMessageW(hwnd, WM_GETTEXTLENGTH, 0, 0) as usize
            });
            POPUP_TEST
                .with(|test| test.borrow_mut().as_mut().unwrap().nested_title_length = length);
            SetWindowTextW(hwnd, wide("Straja — test meniu (fără comenzi)").as_ptr());
            SetWindowLongPtrW(hwnd, GWL_STYLE, WS_OVERLAPPEDWINDOW as isize);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                200,
                160,
                680,
                330,
                SWP_NOZORDER | SWP_FRAMECHANGED,
            );
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            PostMessageW(hwnd, CALLBACK, 0, WM_CONTEXTMENU as LPARAM);
        }
        let mut message = MSG::default();
        let loop_result = loop {
            let result = GetMessageW(&mut message, std::ptr::null_mut(), 0, 0);
            if result <= 0 {
                break result;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        };
        KillTimer(hwnd, 1);
        let registered = TRAY.with(|slot| {
            if let Some(tray) = slot.borrow_mut().take() {
                Shell_NotifyIconW(NIM_DELETE, &tray.data);
                for icon in tray.icons {
                    DestroyIcon(icon);
                }
                tray.ever_added
            } else {
                false
            }
        });
        DestroyWindow(hwnd);
        UnregisterClassW(class_name.as_ptr(), instance);
        if loop_result < 0 {
            bail!("Bucla pictogramei a eșuat");
        }
        if smoke.is_some() && !registered {
            bail!("Windows nu a acceptat pictograma");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_reflects_agent_activity_and_never_allows_stopping_busy_or_service_worker() {
        let mut m = Monitor {
            checked: true,
            ..Default::default()
        };
        assert_eq!(m.presentation().0, 3);
        assert!(!m.can_stop());
        m.state = Some(State {
            agent_version: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        });
        assert_eq!(m.presentation().0, 0);
        assert!(m.can_stop());
        m.state.as_mut().unwrap().active = Some(crate::model::Progress {
            operation: "Backup".into(),
            percent: 0.42,
            ..Default::default()
        });
        assert_eq!(m.presentation().0, 1);
        assert!(m.presentation().1.contains("42%"));
        assert!(!m.can_stop());
        m.state.as_mut().unwrap().active = None;
        m.state.as_mut().unwrap().service_mode = true;
        assert!(!m.can_stop());
        m.state.as_mut().unwrap().agent_version.clear();
        assert_eq!(m.presentation().0, 2);
        assert!(!m.can_stop());
    }
    #[test]
    fn tooltip_is_terminated_and_does_not_split_surrogates() {
        let mut buffer = [1u16; 5];
        copy_wide(&mut buffer, "ab😀c");
        assert_eq!(String::from_utf16(&buffer[..4]).unwrap(), "ab😀");
        assert_eq!(buffer[4], 0);
    }
    #[test]
    #[ignore = "Creates a temporary real Windows notification icon for three seconds"]
    fn native_tray_icon_registers_and_cleans_up() {
        run_inner(Some(Duration::from_secs(3))).unwrap();
    }
    #[test]
    #[ignore = "Shows an isolated native popup menu; no backup or agent commands are executed"]
    fn native_popup_dispatches_reentrant_messages_and_keeps_timer_alive() {
        POPUP_TEST.with(|test| *test.borrow_mut() = Some(PopupTest::default()));
        let seconds = std::env::var("STRAJA_MENU_TEST_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(3)
            .clamp(3, 60);
        run_inner(Some(Duration::from_secs(seconds))).unwrap();
        let observed = POPUP_TEST.with(|test| test.borrow_mut().take().unwrap());
        assert!(
            observed.nested_title_length > 0,
            "Reentrant unhandled messages must reach DefWindowProcW"
        );
        assert!(
            observed.init_with_free_state,
            "Popup message loop must not retain a mutable Tray borrow"
        );
        assert!(
            observed.timer_in_menu,
            "The owner must remain responsive while the popup is open"
        );
    }
}
