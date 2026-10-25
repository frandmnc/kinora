use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, OpenMutexW, SetEvent, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// El que está abierto (uno solo por usuario).
const INSTANCE: &str = "Local\\KinoraTray";
/// El pedido de cerrarse, de `Kinora.exe --quit`.
const QUIT: &str = "Local\\KinoraTrayQuit";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const EVENT_MODIFY_STATE: u32 = 0x0002;
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
/// El código con el que el servidor se apaga para que esta app instale una actualización.
const UPDATE_EXIT_CODE: i32 = 75;

pub fn main() {
    if std::env::args().any(|arg| arg == "--quit") {
        ask_running_to_quit();
        return;
    }

    // Uno solo: si ya hay uno abierto, abrir Kinora otra vez lleva al Admin.
    let instance = unsafe { CreateMutexW(std::ptr::null(), 1, wide(INSTANCE).as_ptr()) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        open_url(&format!("http://localhost:{}/admin/", configured_port()));
        return;
    }
    let quit_event = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide(QUIT).as_ptr()) };

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));

    let supervisor = Arc::new(Mutex::new(Supervisor::new()));
    // La primera vez (sin base de datos), al responder se abre el Admin: ahí se crea la cuenta.
    let first_run = !data_dir().join("kinora.db").exists();
    spawn_monitor(
        supervisor.clone(),
        proxy.clone(),
        quit_event as usize,
        first_run,
    );

    let items = Items::new();
    let menu = Menu::new();
    menu.append_items(&[
        &items.status,
        &PredefinedMenuItem::separator(),
        &items.player,
        &items.admin,
        &items.tv,
        &PredefinedMenuItem::separator(),
        &items.restart,
        &items.logs,
        &items.autostart,
        &PredefinedMenuItem::separator(),
        &items.quit,
    ])
    .expect("could not build the menu");

    let icon_on = icon(false);
    let icon_off = icon(true);
    let mut tray: Option<TrayIcon> = None;
    let mut quitting = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                tray = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(menu.clone()))
                        .with_menu_on_left_click(true)
                        .with_tooltip("Kinora: arrancando")
                        .with_icon(icon_off.clone())
                        .build()
                        .expect("could not create the tray icon"),
                );
            }
            Event::UserEvent(UserEvent::Status(state)) => {
                items.show(&state);
                if let Some(tray) = &tray {
                    let _ = tray.set_icon(Some(if state.is_up() {
                        icon_on.clone()
                    } else {
                        icon_off.clone()
                    }));
                    let _ = tray.set_tooltip(Some(format!("Kinora: {}", state.short())));
                }
            }
            Event::UserEvent(UserEvent::Menu(event)) => {
                let id = event.id();
                let port = configured_port();
                if id == items.player.id() {
                    open_url(&format!("http://localhost:{port}/"));
                } else if id == items.admin.id() {
                    open_url(&format!("http://localhost:{port}/admin/"));
                } else if id == items.tv.id() {
                    if let Some(url) = tv_url(port) {
                        let _ = clipboard_win::set_clipboard_string(&url);
                    }
                } else if id == items.restart.id() {
                    let supervisor = supervisor.clone();
                    thread::spawn(move || supervisor.lock().unwrap().restart());
                } else if id == items.logs.id() {
                    open_url(&log_path().to_string_lossy());
                } else if id == items.autostart.id() {
                    let wanted = items.autostart.is_checked();
                    if set_autostart(wanted).is_err() {
                        items.autostart.set_checked(!wanted);
                    }
                } else if id == items.quit.id() && !quitting {
                    quitting = true;
                    let _ = proxy.send_event(UserEvent::Quit);
                }
            }
            Event::UserEvent(UserEvent::Quit) => {
                quitting = true;
                items.status.set_text("Apagando Kinora…");
                let supervisor = supervisor.clone();
                let proxy = proxy.clone();
                thread::spawn(move || {
                    supervisor.lock().unwrap().shut_down();
                    let _ = proxy.send_event(UserEvent::Exit);
                });
            }
            Event::UserEvent(UserEvent::Exit) => {
                tray.take();
                unsafe { CloseHandle(instance) };
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

enum UserEvent {
    Menu(MenuEvent),
    Status(State),
    Quit,
    Exit,
}

// MARK: El menú

struct Items {
    status: MenuItem,
    player: MenuItem,
    admin: MenuItem,
    tv: MenuItem,
    restart: MenuItem,
    logs: MenuItem,
    autostart: CheckMenuItem,
    quit: MenuItem,
}

impl Items {
    fn new() -> Items {
        let tv = match tv_url(configured_port()) {
            Some(url) => format!("Copiar la dirección para la TV: {url}"),
            None => "Copiar la dirección para la TV".to_string(),
        };
        Items {
            status: MenuItem::new("Kinora está arrancando…", false, None),
            player: MenuItem::new("Abrir el Player", false, None),
            admin: MenuItem::new("Abrir el Admin", false, None),
            tv: MenuItem::new(tv, true, None),
            restart: MenuItem::new("Reiniciar Kinora", true, None),
            logs: MenuItem::new("Ver los registros", true, None),
            autostart: CheckMenuItem::new("Iniciar con Windows", true, autostart_enabled(), None),
            quit: MenuItem::new("Salir de Kinora", true, None),
        }
    }

    fn show(&self, state: &State) {
        self.status.set_text(state.long());
        self.player.set_enabled(state.is_up());
        self.admin.set_enabled(state.is_up());
        self.restart
            .set_enabled(!matches!(state, State::External(_) | State::Updating(_)));
        self.restart.set_text(match state {
            State::Stopped | State::Failed(_) => "Prender Kinora",
            _ => "Reiniciar Kinora",
        });
    }
}

// MARK: El servidor

#[derive(Clone, PartialEq)]
enum State {
    Starting,
    Running(String),
    /// Responde, pero no lo arrancó esta app (por ejemplo, uno abierto desde una terminal).
    External(String),
    Stopped,
    Failed(String),
    /// Bajó una actualización desde el Admin: corre el instalador y esta app se cierra.
    Updating(String),
}

impl State {
    fn is_up(&self) -> bool {
        matches!(self, State::Running(_) | State::External(_))
    }

    fn long(&self) -> String {
        match self {
            State::Starting => "Kinora está arrancando…".to_string(),
            State::Running(version) => format!("Kinora {version} está prendido"),
            State::External(_) => "Kinora ya estaba prendido en otro proceso".to_string(),
            State::Stopped => "Kinora está apagado".to_string(),
            State::Failed(reason) => format!("Kinora no arrancó: {reason}"),
            State::Updating(version) => format!("Instalando Kinora {version}…"),
        }
    }

    fn short(&self) -> &'static str {
        match self {
            State::Starting => "arrancando",
            State::Running(_) | State::External(_) => "prendido",
            State::Stopped => "apagado",
            State::Failed(_) => "no arrancó",
            State::Updating(_) => "actualizando",
        }
    }
}

struct Supervisor {
    child: Option<Child>,
    started_at: Instant,
    /// Cuántas veces seguidas se cerró enseguida: a la tercera se deja de insistir.
    quick_exits: u32,
    restart_at: Option<Instant>,
    /// El instalador que bajó el servidor, para correrlo apenas se apagó.
    update: Option<PathBuf>,
    state: State,
    quitting: bool,
    /// Si esta app se cierra de golpe, Windows cierra también el servidor (y su ffmpeg).
    job: HANDLE,
}

// El HANDLE del job es un puntero de Windows; solo se usa con el candado tomado.
unsafe impl Send for Supervisor {}

impl Supervisor {
    fn new() -> Supervisor {
        Supervisor {
            child: None,
            started_at: Instant::now(),
            quick_exits: 0,
            restart_at: None,
            update: None,
            state: State::Stopped,
            quitting: false,
            job: kill_on_close_job(),
        }
    }

    fn start(&mut self) {
        if self.child.is_some() || self.quitting {
            return;
        }
        self.restart_at = None;
        let server = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("kinora-server.exe")))
            .unwrap_or_else(|| PathBuf::from("kinora-server.exe"));
        let (Some(out), Some(err)) = (open_log(), open_log()) else {
            self.state = State::Failed("no se pudo abrir el registro".to_string());
            return;
        };
        let spawned = Command::new(&server)
            // Se apaga cuando esta app le cierra la entrada: es la forma ordenada (corta el
            // ffmpeg de la cola y cierra la base). Windows no tiene SIGTERM.
            .env("KINORA_STOP_ON_STDIN_EOF", "1")
            .env("PATH", path_with_winget())
            // Con esto, el Admin ofrece actualizar: el servidor sabe quién lo instala y dónde
            // dejarlo.
            .env("KINORA_SUPERVISOR", "windows-tray")
            .env("KINORA_UPDATE_DIR", updates_dir())
            .current_dir(data_dir().parent().unwrap_or(Path::new("C:\\")))
            .stdin(Stdio::piped())
            .stdout(out)
            .stderr(err)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        match spawned {
            Ok(child) => {
                unsafe { AssignProcessToJobObject(self.job, child.as_raw_handle() as HANDLE) };
                self.child = Some(child);
                self.started_at = Instant::now();
                self.state = State::Starting;
            }
            Err(error) => self.state = State::Failed(format!("{} ({error})", server.display())),
        }
    }

    /// Le cierra la entrada y espera hasta 10 segundos a que se apague; si no, lo corta.
    fn stop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        drop(child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
    }

    fn restart(&mut self) {
        self.stop();
        self.quick_exits = 0;
        self.state = State::Starting;
        self.start();
    }

    fn shut_down(&mut self) {
        self.quitting = true;
        self.stop();
    }

    /// Cada medio segundo: si el servidor se cerró solo, y si responde.
    fn tick(&mut self, health: Option<String>) {
        if self.quitting {
            return;
        }
        let exited = match &mut self.child {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => Some(status.code().unwrap_or(-1)),
                _ => None,
            },
            None => None,
        };
        if let Some(code) = exited {
            self.child = None;
            if code == UPDATE_EXIT_CODE {
                match pending_update() {
                    Some((version, installer)) => {
                        self.state = State::Updating(version);
                        self.quitting = true;
                        self.update = Some(installer);
                    }
                    None => {
                        self.state =
                            State::Failed("la actualización no dejó pending.json".to_string())
                    }
                }
                return;
            }
            if let Some(version) = &health {
                // El puerto lo tiene otro Kinora: este no pudo arrancar.
                self.state = State::External(version.clone());
                return;
            }
            let quick = self.started_at.elapsed() < Duration::from_secs(15);
            self.quick_exits = if quick { self.quick_exits + 1 } else { 0 };
            if self.quick_exits >= 3 {
                self.state = State::Failed(format!(
                    "se cerró enseguida {} veces (código {code})",
                    self.quick_exits
                ));
                return;
            }
            self.state = State::Starting;
            self.restart_at = Some(Instant::now() + Duration::from_secs(3));
            return;
        }
        if let Some(at) = self.restart_at
            && Instant::now() >= at
        {
            self.start();
            return;
        }
        match (health, self.child.is_some()) {
            (Some(version), true) => self.state = State::Running(version),
            (Some(version), false) => self.state = State::External(version),
            (None, true) => {
                if matches!(self.state, State::Running(_)) {
                    self.state = State::Starting;
                }
            }
            (None, false) => {
                // El de afuera se apagó: lo reemplaza el de esta app.
                if matches!(self.state, State::External(_))
                    || self.restart_at.is_none() && matches!(self.state, State::Stopped)
                {
                    self.start();
                }
            }
        }
    }
}

fn spawn_monitor(
    supervisor: Arc<Mutex<Supervisor>>,
    proxy: EventLoopProxy<UserEvent>,
    quit_event: usize,
    mut open_admin: bool,
) {
    thread::spawn(move || {
        let port = configured_port();
        let mut last: Option<State> = None;
        loop {
            if unsafe { WaitForSingleObject(quit_event as HANDLE, 0) } == 0 {
                let _ = proxy.send_event(UserEvent::Quit);
                return;
            }
            let health = check_health(port);
            let state = {
                let mut supervisor = supervisor.lock().unwrap();
                if supervisor.quitting {
                    return;
                }
                supervisor.tick(health);
                // Llegó una actualización: el instalador, en silencio, la instala y vuelve a
                // abrir Kinora (/RESTART). Esta app se cierra para que pueda reemplazarla.
                if let Some(installer) = supervisor.update.take() {
                    let _ = proxy.send_event(UserEvent::Status(supervisor.state.clone()));
                    if let Err(error) = Command::new(&installer).args(["/S", "/RESTART"]).spawn() {
                        supervisor.quitting = false;
                        supervisor.state =
                            State::Failed(format!("no se pudo correr el instalador ({error})"));
                    } else {
                        let _ = proxy.send_event(UserEvent::Exit);
                        return;
                    }
                }
                supervisor.state.clone()
            };
            if open_admin && state.is_up() {
                open_admin = false;
                open_url(&format!("http://localhost:{port}/admin/"));
            }
            if last.as_ref() != Some(&state) {
                last = Some(state.clone());
                let _ = proxy.send_event(UserEvent::Status(state));
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
}

/// GET /api/health: la versión si responde, `None` si no.
fn check_health(port: u16) -> Option<String> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(400)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream
        .write_all(b"GET /api/health HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return None;
    }
    let version = response
        .split("\"version\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("?");
    Some(version.to_string())
}

// MARK: `Kinora.exe --quit`

/// Avisa al que está abierto que se cierre y espera (hasta 30 segundos) a que termine.
fn ask_running_to_quit() {
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, wide(QUIT).as_ptr()) };
    if event.is_null() {
        return;
    }
    unsafe { SetEvent(event) };
    let instance = unsafe { OpenMutexW(SYNCHRONIZE, 0, wide(INSTANCE).as_ptr()) };
    if !instance.is_null() {
        // Cuando el otro termina, el mutex queda abandonado y la espera vuelve.
        unsafe { WaitForSingleObject(instance, 30_000) };
        unsafe { CloseHandle(instance) };
    }
    unsafe { CloseHandle(event) };
}

// MARK: Ayudas

fn kill_on_close_job() -> HANDLE {
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        job
    }
}

/// La carpeta de datos por defecto, la misma que usa el servidor: ~/.kinora.
fn data_dir() -> PathBuf {
    std::env::home_dir()
        .unwrap_or_else(|| PathBuf::from("C:\\"))
        .join(".kinora")
}

/// El puerto de ~/.kinora/config.toml (`port = 8090`), o 8090.
fn configured_port() -> u16 {
    let Ok(text) = fs::read_to_string(data_dir().join("config.toml")) else {
        return 8090;
    };
    text.lines()
        .filter_map(|line| line.split('#').next()?.split_once('='))
        .find(|(key, _)| key.trim() == "port")
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(8090)
}

fn tv_url(port: u16) -> Option<String> {
    local_ip_address::local_ip()
        .ok()
        .map(|ip| format!("http://{ip}:{port}"))
}

/// %LOCALAPPDATA%\Kinora\updates: donde el servidor deja lo que baja.
fn updates_dir() -> PathBuf {
    log_path()
        .parent()
        .and_then(Path::parent)
        .map(|kinora| kinora.join("updates"))
        .unwrap_or_else(|| data_dir().join("updates"))
}

/// Lo que dejó el servidor en pending.json: la versión y el instalador.
fn pending_update() -> Option<(String, PathBuf)> {
    let text = fs::read_to_string(updates_dir().join("pending.json")).ok()?;
    let pending: serde_json::Value = serde_json::from_str(&text).ok()?;
    let version = pending.get("version")?.as_str()?.to_string();
    let file = PathBuf::from(pending.get("file")?.as_str()?);
    let _ = fs::remove_file(updates_dir().join("pending.json"));
    file.is_file().then_some((version, file))
}

/// %LOCALAPPDATA%\Kinora\logs\kinora.log
fn log_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir());
    base.join("Kinora").join("logs").join("kinora.log")
}

/// El registro, para agregar al final. Si pasa de 10 MB, el anterior queda como kinora.log.1.
fn open_log() -> Option<File> {
    let path = log_path();
    fs::create_dir_all(path.parent()?).ok()?;
    if fs::metadata(&path).is_ok_and(|m| m.len() > 10_000_000) {
        let _ = fs::rename(&path, path.with_extension("log.1"));
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// El PATH con las carpetas donde winget deja ffmpeg: si se acaba de instalar, el PATH de esta
/// sesión todavía no las tiene.
fn path_with_winget() -> String {
    let mut parts = vec![std::env::var("PATH").unwrap_or_default()];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        parts.push(
            PathBuf::from(local)
                .join("Microsoft\\WinGet\\Links")
                .to_string_lossy()
                .into_owned(),
        );
    }
    parts.push("C:\\Program Files\\WinGet\\Links".to_string());
    parts.join(";")
}

fn autostart_enabled() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|key| key.get_value::<String, _>("Kinora"))
        .is_ok()
}

fn set_autostart(enabled: bool) -> std::io::Result<()> {
    let (key, _) =
        winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
    if enabled {
        let exe = std::env::current_exe()?;
        key.set_value("Kinora", &format!("\"{}\"", exe.display()))
    } else {
        match key.delete_value("Kinora") {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

fn open_url(target: &str) {
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide(target).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// El ícono de la bandeja; apagado (más tenue y sin color) si Kinora no responde.
fn icon(dim: bool) -> Icon {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../../assets/kinora-32.png"
    )));
    let mut reader = decoder.read_info().expect("tray icon");
    let mut rgba = vec![0; reader.output_buffer_size().expect("tray icon size")];
    let info = reader.next_frame(&mut rgba).expect("tray icon");
    rgba.truncate(info.buffer_size());
    if dim {
        for pixel in rgba.chunks_exact_mut(4) {
            let gray = ((pixel[0] as u32 + pixel[1] as u32 + pixel[2] as u32) / 3) as u8;
            pixel[0] = gray;
            pixel[1] = gray;
            pixel[2] = gray;
            pixel[3] = (pixel[3] as u32 * 55 / 100) as u8;
        }
    }
    Icon::from_rgba(rgba, info.width, info.height).expect("tray icon")
}
