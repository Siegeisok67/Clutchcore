#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../../src/theme.rs"]
mod theme;

use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Rect, RichText, Rounding, Sense, Stroke, Vec2,
};
use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    IShellLinkW, SHBrowseForFolderW, SHGetPathFromIDListW, ShellExecuteW, BIF_NEWDIALOGSTYLE,
    BIF_RETURNONLYFSDIRS, BROWSEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const APP: &str = "Clutchcore";
const VERSION: &str = "1.0.4";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Clutchcore";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const LOGO_SIZE: usize = 256;

const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));
const LOGO: &[u8] = include_bytes!("../../assets/logo256.rgba");

const CLSID_SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = Options::parse(&args);

    if options.silent {
        let code = if options.uninstall {
            silent_uninstall(&options)
        } else {
            silent_install(&options)
        };
        std::process::exit(code);
    }

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let icon = egui::IconData {
        rgba: LOGO.to_vec(),
        width: LOGO_SIZE as u32,
        height: LOGO_SIZE as u32,
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 566.0])
            .with_min_inner_size([820.0, 566.0])
            .with_resizable(false)
            .with_icon(std::sync::Arc::new(icon)),
        ..Default::default()
    };

    let mode = if options.uninstall { Mode::Uninstall } else { Mode::Install };
    let title = if options.uninstall { "Clutchcore Uninstall" } else { "Clutchcore Setup" };

    if let Err(error) = eframe::run_native(
        title,
        native,
        Box::new(move |cc| Ok(Box::new(SetupApp::new(cc, mode, options)))),
    ) {
        eprintln!("setup could not start: {error}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------- options

#[derive(Clone)]
struct Options {
    silent: bool,
    uninstall: bool,
    keep_data: bool,
    dir: Option<PathBuf>,
    desktop_shortcut: bool,
    register: bool,
    launch: bool,
}

impl Options {
    fn parse(args: &[String]) -> Self {
        let mut options = Self {
            silent: false,
            uninstall: false,
            keep_data: false,
            dir: None,
            desktop_shortcut: true,
            register: true,
            launch: false,
        };
        let mut index = 0;
        while index < args.len() {
            let arg = args[index].to_ascii_lowercase();
            match arg.as_str() {
                "--silent" | "/s" => options.silent = true,
                "--uninstall" => options.uninstall = true,
                "--keep-data" => options.keep_data = true,
                "--no-desktop" => options.desktop_shortcut = false,
                "--no-register" => options.register = false,
                "--launch" => options.launch = true,
                "--dir" => {
                    if let Some(value) = args.get(index + 1) {
                        options.dir = Some(PathBuf::from(value));
                        index += 1;
                    }
                }
                other => {
                    if let Some(value) = other.strip_prefix("--dir=") {
                        options.dir = Some(PathBuf::from(value));
                    }
                }
            }
            index += 1;
        }
        if !options.dir.is_some() {
            if let Ok(value) = std::env::var("CLUTCHCORE_SETUP_DIR") {
                if !value.trim().is_empty() {
                    options.dir = Some(PathBuf::from(value));
                }
            }
        }
        options
    }
}

// ---------------------------------------------------------------- paths

fn local_app_data() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public"))
}

fn roaming_app_data() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| local_app_data())
}

fn user_profile() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public"))
}

fn default_dir() -> PathBuf {
    local_app_data().join("Programs").join(APP)
}

fn data_dir() -> PathBuf {
    roaming_app_data().join(APP)
}

fn desktop_dir() -> PathBuf {
    let profile = user_profile();
    let onedrive = profile.join("OneDrive").join("Desktop");
    if onedrive.is_dir() {
        return onedrive;
    }
    profile.join("Desktop")
}

fn desktop_shortcut() -> PathBuf {
    desktop_dir().join(format!("{APP}.lnk"))
}

fn start_menu_shortcut() -> PathBuf {
    roaming_app_data()
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join(format!("{APP}.lnk"))
}

fn installed_dir() -> Option<PathBuf> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(UNINSTALL_KEY)
        .ok()?;
    let value: String = key.get_value("InstallLocation").ok()?;
    if value.trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

// ---------------------------------------------------------------- install work

struct Plan {
    dir: PathBuf,
    desktop: bool,
    register: bool,
}

fn payload_bytes() -> Result<Vec<u8>, String> {
    if PAYLOAD.len() < 8 {
        return Err("The installer payload is missing or damaged.".into());
    }
    let expected = u64::from_le_bytes(PAYLOAD[..8].try_into().unwrap()) as usize;
    let data = miniz_oxide::inflate::decompress_to_vec(&PAYLOAD[8..])
        .map_err(|_| "The installer payload could not be unpacked. The file may be damaged.".to_string())?;
    if data.len() != expected {
        return Err("The installer payload is incomplete. Please download the setup file again.".into());
    }
    Ok(data)
}

fn stop_running_app() {
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "Clutchcore.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(Duration::from_millis(300));
}

fn perform_install(plan: &Plan, notify: &mut dyn FnMut(f32, &str)) -> Result<(), String> {
    notify(0.04, "Closing any running copy of Clutchcore");
    stop_running_app();

    notify(0.10, &format!("Creating {}", plan.dir.display()));
    fs::create_dir_all(&plan.dir)
        .map_err(|error| format!("Could not create {}: {error}", plan.dir.display()))?;

    notify(0.18, "Unpacking application files");
    let bytes = payload_bytes()?;
    let exe = plan.dir.join("Clutchcore.exe");
    fs::write(&exe, &bytes).map_err(|error| {
        format!(
            "Could not write {}: {error}\n\nClose Clutchcore and try again, or pick a different folder.",
            exe.display()
        )
    })?;

    notify(0.70, "Preparing the uninstaller");
    let uninstaller = plan.dir.join("uninstall.exe");
    let current = std::env::current_exe().map_err(|error| format!("Could not locate setup: {error}"))?;
    if current != uninstaller {
        fs::copy(&current, &uninstaller)
            .map_err(|error| format!("Could not write the uninstaller: {error}"))?;
    }

    notify(0.80, "Creating shortcuts");
    if plan.desktop {
        create_shortcut(&desktop_shortcut(), &exe, &plan.dir)?;
    }
    create_shortcut(&start_menu_shortcut(), &exe, &plan.dir)?;

    if plan.register {
        notify(0.90, "Registering with Windows");
        register_uninstall(plan, &exe, bytes.len())?;
    }

    notify(1.0, "Installed");
    Ok(())
}

fn perform_uninstall(dir: &Path, keep_data: bool, notify: &mut dyn FnMut(f32, &str)) -> Result<(), String> {
    notify(0.10, "Closing Clutchcore");
    stop_running_app();

    notify(0.30, "Removing shortcuts");
    let _ = fs::remove_file(start_menu_shortcut());
    let _ = fs::remove_file(desktop_shortcut());

    notify(0.50, "Removing the entry in Windows Settings");
    let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(UNINSTALL_KEY);

    if !keep_data {
        notify(0.68, "Deleting profiles and backups");
        let _ = fs::remove_dir_all(data_dir());
    }

    notify(0.88, "Removing program files");
    let running = std::env::current_exe().ok();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if running.as_ref().map(|exe| exe == &path).unwrap_or(false) {
                continue;
            }
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }
    }

    // Our own executable can keep the folder locked. Move it out of the way so
    // the install folder can be deleted right now instead of on next logon.
    let leftover = detach_self(dir);
    if fs::remove_dir(dir).is_err() && leftover.is_none() {
        schedule_dir_removal(dir);
    }
    if let Some(path) = leftover {
        schedule_file_removal(&path);
    }

    notify(1.0, "Removed");
    Ok(())
}

fn detach_self(dir: &Path) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    let same = match (fs::canonicalize(parent), fs::canonicalize(dir)) {
        (Ok(current), Ok(target)) => current == target,
        _ => parent == dir,
    };
    if !same {
        return None;
    }
    let temp = std::env::temp_dir().join(format!("clutchcore-uninstall-{}.exe", std::process::id()));
    fs::rename(&exe, &temp).ok()?;
    Some(temp)
}

fn register_uninstall(plan: &Plan, exe: &Path, size: usize) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(UNINSTALL_KEY)
        .map_err(|error| format!("Could not register the app with Windows: {error}"))?;
    let uninstaller = plan.dir.join("uninstall.exe");
    let location = plan.dir.display().to_string();

    let write = |name: &str, value: &str| -> Result<(), String> {
        key.set_value(name, &value)
            .map_err(|error| format!("Could not write the uninstall entry: {error}"))
    };

    write("DisplayName", APP)?;
    write("DisplayVersion", VERSION)?;
    write("Publisher", APP)?;
    write("InstallLocation", &location)?;
    write("DisplayIcon", &exe.display().to_string())?;
    write("UninstallString", &format!("\"{}\" --uninstall", uninstaller.display()))?;
    write("QuietUninstallString", &format!("\"{}\" --uninstall --silent", uninstaller.display()))?;
    key.set_value("EstimatedSize", &((size / 1024) as u32))
        .map_err(|error| error.to_string())?;
    key.set_value("NoModify", &1u32).map_err(|error| error.to_string())?;
    key.set_value("NoRepair", &1u32).map_err(|error| error.to_string())?;
    Ok(())
}

fn normalize(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('/', "\\"))
}

fn schedule_dir_removal(dir: &Path) {
    let command = format!(
        "ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"",
        normalize(dir).display()
    );
    run_hidden(&command);
}

fn schedule_file_removal(file: &Path) {
    let command = format!(
        "ping -n 3 127.0.0.1 >nul & del /f /q \"{}\"",
        normalize(file).display()
    );
    run_hidden(&command);
}

fn run_hidden(command: &str) {
    let _ = Command::new("cmd")
        .args(["/C", command])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn silent_install(options: &Options) -> i32 {
    let plan = Plan {
        dir: options
            .dir
            .as_ref()
            .map(|path| normalize(path))
            .unwrap_or_else(default_dir),
        desktop: options.desktop_shortcut,
        register: options.register,
    };
    let mut quiet = |progress: f32, status: &str| {
        println!("[{:>3.0}%] {status}", progress * 100.0);
    };
    match perform_install(&plan, &mut quiet) {
        Ok(()) => {
            println!("{APP} {VERSION} installed to {}", plan.dir.display());
            if options.launch {
                open_path(&plan.dir.join("Clutchcore.exe"));
            }
            0
        }
        Err(error) => {
            eprintln!("install failed: {error}");
            1
        }
    }
}

fn silent_uninstall(options: &Options) -> i32 {
    let dir = match options.dir.as_ref() {
        Some(path) => normalize(path),
        None => installed_dir()
            .or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().map(Path::to_path_buf))
            })
            .unwrap_or_else(default_dir),
    };
    let mut quiet = |progress: f32, status: &str| {
        println!("[{:>3.0}%] {status}", progress * 100.0);
    };
    match perform_uninstall(&dir, options.keep_data, &mut quiet) {
        Ok(()) => {
            println!("{APP} removed");
            0
        }
        Err(error) => {
            eprintln!("uninstall failed: {error}");
            1
        }
    }
}

// ---------------------------------------------------------------- windows integration

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn wide_str(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn create_shortcut(link: &Path, target: &Path, workdir: &Path) -> Result<(), String> {
    if let Some(parent) = link.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let target_w = wide(target.as_os_str());
    let workdir_w = wide(workdir.as_os_str());
    let link_w = wide(link.as_os_str());
    let description_w = wide_str("Clutchcore - competitive system tuner");

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let shell_link: IShellLinkW =
            CoCreateInstance(&CLSID_SHELL_LINK, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("Could not create a shortcut: {error}"))?;
        shell_link
            .SetPath(PCWSTR(target_w.as_ptr()))
            .map_err(|error| format!("Could not create a shortcut: {error}"))?;
        let _ = shell_link.SetWorkingDirectory(PCWSTR(workdir_w.as_ptr()));
        let _ = shell_link.SetDescription(PCWSTR(description_w.as_ptr()));
        let _ = shell_link.SetIconLocation(PCWSTR(target_w.as_ptr()), 0);
        let persist: IPersistFile = shell_link
            .cast()
            .map_err(|error| format!("Could not create a shortcut: {error}"))?;
        persist
            .Save(PCWSTR(link_w.as_ptr()), BOOL(1))
            .map_err(|error| format!("Could not save {}: {error}", link.display()))?;
    }
    Ok(())
}

fn open_path(path: &Path) {
    let path_w = wide(path.as_os_str());
    let verb_w = wide_str("open");
    unsafe {
        ShellExecuteW(
            HWND(std::ptr::null_mut()),
            PCWSTR(verb_w.as_ptr()),
            PCWSTR(path_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn pick_folder() -> Option<PathBuf> {
    let title = wide_str("Select the Clutchcore install folder");
    let mut display = vec![0u16; 260];
    let browse = BROWSEINFOW {
        hwndOwner: HWND(std::ptr::null_mut()),
        pidlRoot: std::ptr::null_mut(),
        pszDisplayName: windows::core::PWSTR(display.as_mut_ptr()),
        lpszTitle: PCWSTR(title.as_ptr()),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: None,
        lParam: LPARAM(0),
        iImage: 0,
    };

    unsafe {
        let pidl = SHBrowseForFolderW(&browse);
        if pidl.is_null() {
            return None;
        }
        let mut buffer = [0u16; 260];
        let ok = SHGetPathFromIDListW(pidl, &mut buffer);
        CoTaskMemFree(Some(pidl as *const _));
        if !ok.as_bool() {
            return None;
        }
        let length = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
    }
}

// ---------------------------------------------------------------- app

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Install,
    Uninstall,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Welcome,
    Working,
    Done,
    Failed,
    ConfirmUninstall,
    Removed,
}

#[derive(Clone, Copy)]
enum Action {
    Install,
    Uninstall,
    Browse,
    Close,
    Launch,
    OpenFolder,
}

enum Msg {
    Progress(f32, String),
    Finished(Result<(), String>),
}

struct SetupApp {
    mode: Mode,
    screen: Screen,
    dir: String,
    desktop: bool,
    keep_data: bool,
    existing: Option<PathBuf>,
    progress: f32,
    status: String,
    error: Option<String>,
    cleanup: Option<PathBuf>,
    logo: Option<egui::TextureHandle>,
    screen_t: f32,
    screen_started: Option<f64>,
    shown_screen: Screen,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl SetupApp {
    fn new(cc: &eframe::CreationContext<'_>, mode: Mode, options: Options) -> Self {
        theme::install(&cc.egui_ctx);
        let (tx, rx) = mpsc::channel();
        let existing = installed_dir();
        let fallback = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .unwrap_or_else(default_dir);
        let dir = match mode {
            Mode::Install => options
                .dir
                .clone()
                .or_else(|| existing.clone())
                .unwrap_or_else(default_dir),
            Mode::Uninstall => options
                .dir
                .clone()
                .or_else(|| existing.clone())
                .unwrap_or(fallback),
        };

        let screen = match mode {
            Mode::Install => Screen::Welcome,
            Mode::Uninstall => Screen::ConfirmUninstall,
        };

        Self {
            mode,
            screen,
            dir: dir.display().to_string(),
            desktop: options.desktop_shortcut,
            keep_data: if options.uninstall { true } else { options.keep_data },
            existing,
            progress: 0.0,
            status: String::new(),
            error: None,
            cleanup: None,
            logo: None,
            screen_t: 0.0,
            screen_started: None,
            shown_screen: screen,
            tx,
            rx,
        }
    }

    fn logo(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        if let Some(handle) = &self.logo {
            return handle.clone();
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([LOGO_SIZE, LOGO_SIZE], LOGO);
        let handle = ctx.load_texture("clutchcore-logo", image, egui::TextureOptions::LINEAR);
        self.logo = Some(handle.clone());
        handle
    }

    fn poll(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Msg::Progress(value, status) => {
                    self.progress = value;
                    self.status = status;
                }
                Msg::Finished(result) => match result {
                    Ok(()) => {
                        self.progress = 1.0;
                        match self.mode {
                            Mode::Install => {
                                self.existing = Some(PathBuf::from(&self.dir));
                                self.screen = Screen::Done;
                            }
                            Mode::Uninstall => {
                                self.cleanup = Some(PathBuf::from(&self.dir));
                                self.screen = Screen::Removed;
                            }
                        }
                    }
                    Err(error) => {
                        self.error = Some(error);
                        self.screen = Screen::Failed;
                    }
                },
            }
        }
    }

    fn start_install(&mut self) {
        self.screen = Screen::Working;
        self.progress = 0.0;
        self.status = "Starting".into();
        self.error = None;
        let plan = Plan {
            dir: normalize(Path::new(self.dir.trim())),
            desktop: self.desktop,
            register: true,
        };
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let mut notify = |progress: f32, status: &str| {
                let _ = tx.send(Msg::Progress(progress, status.to_string()));
            };
            let result = perform_install(&plan, &mut notify);
            let _ = tx.send(Msg::Finished(result));
        });
    }

    fn start_uninstall(&mut self) {
        self.screen = Screen::Working;
        self.progress = 0.0;
        self.status = "Starting".into();
        self.error = None;
        let dir = normalize(Path::new(self.dir.trim()));
        let keep_data = self.keep_data;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let mut notify = |progress: f32, status: &str| {
                let _ = tx.send(Msg::Progress(progress, status.to_string()));
            };
            let result = perform_uninstall(&dir, keep_data, &mut notify);
            let _ = tx.send(Msg::Finished(result));
        });
    }

    fn handle(&mut self, action: Option<Action>, ctx: &egui::Context) {
        match action {
            Some(Action::Install) => self.start_install(),
            Some(Action::Uninstall) => self.start_uninstall(),
            Some(Action::Browse) => {
                if let Some(folder) = pick_folder() {
                    self.dir = folder.join(APP).display().to_string();
                }
            }
            Some(Action::Close) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Some(Action::Launch) => open_path(&PathBuf::from(&self.dir).join("Clutchcore.exe")),
            Some(Action::OpenFolder) => open_path(Path::new(&self.dir)),
            None => {}
        }
    }

    fn header(&mut self, ctx: &egui::Context) {
        let texture = self.logo(ctx);
        egui::TopBottomPanel::top("header")
            .exact_height(112.0)
            .frame(
                egui::Frame::none()
                    .fill(theme::SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(26.0, 20.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (logo_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(62.0), Sense::hover());
                    ui.painter().circle_filled(
                        logo_rect.center(),
                        34.0,
                        theme::with_alpha(theme::ACCENT, 14),
                    );
                    ui.painter().image(
                        texture.id(),
                        logo_rect,
                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(
                            RichText::new(APP.to_uppercase())
                                .size(24.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                        ui.label(
                            RichText::new("COMPETITIVE SYSTEM TUNER")
                                .size(8.5)
                                .strong()
                                .color(theme::ACCENT),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::Frame::none()
                            .fill(theme::CHIP)
                            .rounding(Rounding::same(999.0))
                            .inner_margin(egui::Margin::symmetric(11.0, 5.0))
                            .show(ui, |ui| {
                                let label = match self.mode {
                                    Mode::Install => format!("SETUP {VERSION}"),
                                    Mode::Uninstall => "UNINSTALL".to_string(),
                                };
                                ui.label(RichText::new(label).size(9.0).strong().color(theme::MUTED));
                            });
                    });
                });
                accent_rule(ui);
            });
    }

    fn footer(&mut self, ctx: &egui::Context, action: &mut Option<Action>) {
        let screen = self.screen;
        egui::TopBottomPanel::bottom("footer")
            .exact_height(66.0)
            .frame(
                egui::Frame::none()
                    .fill(theme::SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(26.0, 17.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{VERSION}  ·  Windows 10 / 11  ·  64-bit"))
                            .size(9.5)
                            .color(theme::FAINT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| match screen {
                        Screen::Welcome => {
                            if accent(ui, if self.existing.is_some() { "Reinstall" } else { "Install" }).clicked() {
                                *action = Some(Action::Install);
                            }
                            if ghost(ui, "Cancel").clicked() {
                                *action = Some(Action::Close);
                            }
                        }
                        Screen::Working => {
                            ui.add_enabled(false, egui::Button::new(RichText::new("Working…").size(12.0)));
                        }
                        Screen::Done => {
                            if accent(ui, "Launch Clutchcore").clicked() {
                                *action = Some(Action::Launch);
                            }
                            if ghost(ui, "Open folder").clicked() {
                                *action = Some(Action::OpenFolder);
                            }
                            if ghost(ui, "Close").clicked() {
                                *action = Some(Action::Close);
                            }
                        }
                        Screen::Failed => {
                            if accent(ui, "Try again").clicked() {
                                *action = Some(if self.mode == Mode::Uninstall {
                                    Action::Uninstall
                                } else {
                                    Action::Install
                                });
                            }
                            if ghost(ui, "Close").clicked() {
                                *action = Some(Action::Close);
                            }
                        }
                        Screen::ConfirmUninstall => {
                            if danger(ui, "Remove Clutchcore").clicked() {
                                *action = Some(Action::Uninstall);
                            }
                            if ghost(ui, "Cancel").clicked() {
                                *action = Some(Action::Close);
                            }
                        }
                        Screen::Removed => {
                            if accent(ui, "Close").clicked() {
                                *action = Some(Action::Close);
                            }
                        }
                    });
                });
            });
    }

    fn body(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        match self.screen {
            Screen::Welcome => self.welcome(ui, action),
            Screen::Working => self.working(ui),
            Screen::Done => self.done(ui),
            Screen::Failed => self.failed(ui),
            Screen::ConfirmUninstall => self.confirm_uninstall(ui),
            Screen::Removed => self.removed(ui),
        }
    }

    fn welcome(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        title(ui, "Install Clutchcore", "Latency, frame pacing and input tuning for competitive play. Every original setting is captured before anything is written.");
        ui.add_space(14.0);

        card(ui, theme::CARD, |ui| {
            ui.label(RichText::new("INSTALL LOCATION").size(8.5).strong().color(theme::FAINT));
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 96.0).max(200.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.dir)
                        .desired_width(width)
                        .font(FontId::proportional(12.0))
                        .margin(Vec2::new(8.0, 6.0)),
                );
                if ghost(ui, "Browse").clicked() {
                    *action = Some(Action::Browse);
                }
            });
            ui.add_space(7.0);
            ui.label(
                RichText::new("Installs for this Windows account. No administrator rights required.")
                    .size(10.0)
                    .color(theme::MUTED),
            );
        });

        ui.add_space(10.0);

        card(ui, theme::CARD, |ui| {
            ui.label(RichText::new("OPTIONS").size(8.5).strong().color(theme::FAINT));
            ui.add_space(8.0);
            check_row(ui, &mut self.desktop, "Create a desktop shortcut");
            ui.add_space(2.0);
            ui.label(
                RichText::new("A Start menu shortcut is always created.")
                    .size(10.0)
                    .color(theme::FAINT),
            );
        });

        if let Some(existing) = self.existing.clone() {
            ui.add_space(10.0);
            let mut note = format!(
                "Clutchcore {VERSION} is already installed in {}. Setup will replace the program files.",
                existing.display()
            );
            if !data_dir().is_dir() {
                note = format!("Setup will install Clutchcore {VERSION}.");
            }
            card(ui, theme::ACCENT_TINT, |ui| {
                ui.horizontal(|ui| {
                    ui::dot(ui, theme::ACCENT, 12.0);
                    ui.label(
                        RichText::new(note)
                            .size(10.5)
                            .color(theme::TEXT),
                    );
                });
            });
        }
    }

    fn working(&mut self, ui: &mut egui::Ui) {
        let animated = ui
            .ctx()
            .animate_value_with_time(egui::Id::new("setup-progress"), self.progress, 0.25);
        ui.add_space(6.0);
        let heading = if self.mode == Mode::Uninstall {
            "Removing Clutchcore"
        } else {
            "Installing Clutchcore"
        };
        ui.label(RichText::new(heading).size(21.0).strong().color(theme::TEXT));
        ui.add_space(4.0);
        ui.label(
            RichText::new(if self.status.is_empty() { "Starting…" } else { self.status.as_str() })
                .size(11.5)
                .color(theme::MUTED),
        );
        ui.add_space(18.0);
        progress_bar(ui, animated);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{:.0}%", animated * 100.0))
                    .size(10.0)
                    .strong()
                    .color(theme::ACCENT),
            );
            ui.label(
                RichText::new("This only takes a moment. Nothing is sent anywhere.")
                    .size(10.0)
                    .color(theme::FAINT),
            );
        });
    }

    fn done(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        mark(ui, theme::GREEN, "✓", self.screen_t);
        ui.add_space(12.0);
        ui.label(
            RichText::new("Clutchcore is installed")
                .size(21.0)
                .strong()
                .color(theme::TEXT),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("Start with a preset — Safe Street keeps everything reversible, Full Clutch pushes harder. Every change is backed up first.")
                .size(11.5)
                .color(theme::MUTED),
        );
        ui.add_space(14.0);
        card(ui, theme::CARD, |ui| {
            ui.label(RichText::new("INSTALLED TO").size(8.5).strong().color(theme::FAINT));
            ui.add_space(6.0);
            ui.label(
                RichText::new(self.dir.clone())
                    .size(11.5)
                    .font(FontId::monospace(11.0))
                    .color(theme::TEXT),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Some tweaks need administrator rights or a reboot — Clutchcore tells you before it applies them.")
                    .size(10.0)
                    .color(theme::FAINT),
            );
        });
    }

    fn failed(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        mark(ui, theme::RED, "!", self.screen_t);
        ui.add_space(12.0);
        ui.label(
            RichText::new("Setup could not finish")
                .size(21.0)
                .strong()
                .color(theme::TEXT),
        );
        ui.add_space(6.0);
        let message = self.error.clone().unwrap_or_else(|| "Unknown error.".into());
        card(ui, theme::RED_TINT, |ui| {
            ui.label(
                RichText::new(message)
                    .size(11.0)
                    .color(theme::TEXT),
            );
        });
    }

    fn confirm_uninstall(&mut self, ui: &mut egui::Ui) {
        title(
            ui,
            "Remove Clutchcore",
            "Setup removes the app, its shortcuts and its Windows Settings entry. Your game settings are not touched.",
        );
        ui.add_space(14.0);
        card(ui, theme::YELLOW_TINT, |ui| {
            ui.horizontal(|ui| {
                ui::dot(ui, theme::YELLOW, 12.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Applied tweaks stay applied")
                            .size(11.5)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(
                        RichText::new("Open Clutchcore and use Restore All before removing it if you want your original registry settings back.")
                            .size(10.5)
                            .color(theme::MUTED),
                    );
                });
            });
        });
        ui.add_space(10.0);
        card(ui, theme::CARD, |ui| {
            ui.label(RichText::new("YOUR DATA").size(8.5).strong().color(theme::FAINT));
            ui.add_space(8.0);
            check_row(
                ui,
                &mut self.keep_data,
                "Keep my profiles, backups and original settings",
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new(format!("Stored in {}", data_dir().display()))
                    .size(10.0)
                    .color(theme::FAINT),
            );
        });
    }

    fn removed(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        mark(ui, theme::GREEN, "✓", self.screen_t);
        ui.add_space(12.0);
        ui.label(
            RichText::new("Clutchcore was removed")
                .size(21.0)
                .strong()
                .color(theme::TEXT),
        );
        ui.add_space(4.0);
        let message = if self.keep_data {
            format!(
                "Your profiles, backups and original settings are still in {}.",
                data_dir().display()
            )
        } else {
            "Your profiles, backups and original settings were deleted.".to_string()
        };
        ui.label(RichText::new(message).size(11.5).color(theme::MUTED));
    }
}

impl eframe::App for SetupApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        let mut action = None;

        let now = ctx.input(|input| input.time);
        if self.screen != self.shown_screen {
            self.shown_screen = self.screen;
            self.screen_started = Some(now);
        }
        let start = *self.screen_started.get_or_insert(now);
        self.screen_t = theme::ease_out(((now - start) as f32 / 0.28).clamp(0.0, 1.0));
        let screen_t = self.screen_t;

        self.header(ctx);
        self.footer(ctx, &mut action);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(26.0, 18.0)),
            )
            .show(ctx, |ui| {
                ui.add_space((1.0 - screen_t) * 6.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.body(ui, &mut action));
                if screen_t < 1.0 {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        theme::with_alpha(theme::BG, ((1.0 - screen_t) * 235.0) as u8),
                    );
                }
            });

        if matches!(self.screen, Screen::Working) {
            ctx.request_repaint_after(Duration::from_millis(40));
        }
        if screen_t < 1.0 {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        self.handle(action, ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(dir) = self.cleanup.take() {
            let _ = fs::remove_dir(&dir);
        }
    }
}

// ---------------------------------------------------------------- widgets

mod ui {
    use super::*;
    use crate::theme;

    pub fn dot(ui: &mut egui::Ui, color: Color32, size: f32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        ui.painter().circle_filled(rect.center(), size * 0.3, color);
    }
}

fn accent_rule(ui: &egui::Ui) {
    let rect = ui.max_rect();
    let y = rect.bottom() - 1.0;
    let line = Rect::from_min_max(
        egui::pos2(rect.left(), y),
        egui::pos2(rect.right(), y + 1.0),
    );
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(line.left_top(), theme::with_alpha(theme::ACCENT, 120));
    mesh.colored_vertex(line.left_bottom(), theme::with_alpha(theme::ACCENT, 120));
    mesh.colored_vertex(line.right_top(), Color32::TRANSPARENT);
    mesh.colored_vertex(line.right_bottom(), Color32::TRANSPARENT);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(egui::Shape::mesh(mesh));
}

fn title(ui: &mut egui::Ui, heading: &str, subtitle: &str) {
    ui.label(RichText::new(heading).size(22.0).strong().color(theme::TEXT));
    ui.add_space(3.0);
    ui.label(RichText::new(subtitle).size(11.5).color(theme::MUTED));
}

fn card<R>(ui: &mut egui::Ui, fill: Color32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::same(15.0))
        .show(ui, add)
        .inner
}

fn accent(ui: &mut egui::Ui, text: &str) -> egui::Response {
    button(ui, text, theme::ACCENT, theme::ACCENT_TEXT, Stroke::NONE)
}

fn ghost(ui: &mut egui::Ui, text: &str) -> egui::Response {
    button(ui, text, theme::CHIP, theme::TEXT, Stroke::new(1.0, theme::BORDER))
}

fn danger(ui: &mut egui::Ui, text: &str) -> egui::Response {
    button(ui, text, theme::RED_TINT, theme::RED, Stroke::new(1.0, theme::RED))
}

fn button(ui: &mut egui::Ui, text: &str, fill: Color32, fg: Color32, stroke: Stroke) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(12.0).strong().color(fg))
            .fill(fill)
            .stroke(stroke)
            .rounding(Rounding::same(7.0))
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

fn check_row(ui: &mut egui::Ui, checked: &mut bool, label: &str) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::click());
    let box_rect = Rect::from_min_size(rect.left_top() + Vec2::new(1.0, 4.0), Vec2::splat(16.0));
    let fill = if *checked { theme::ACCENT } else { theme::INPUT_BG };
    let stroke = if *checked { theme::ACCENT } else { theme::BORDER };
    ui.painter()
        .rect(box_rect, Rounding::same(4.0), fill, Stroke::new(1.0, stroke));
    if *checked {
        let center = box_rect.center();
        ui.painter().add(egui::Shape::line(
            vec![
                center + Vec2::new(-4.0, 0.0),
                center + Vec2::new(-1.2, 3.0),
                center + Vec2::new(4.4, -3.6),
            ],
            Stroke::new(1.8, theme::ACCENT_TEXT),
        ));
    }
    ui.painter().text(
        egui::pos2(box_rect.right() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        theme::TEXT,
    );
    if response.clicked() {
        *checked = !*checked;
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn progress_bar(ui: &mut egui::Ui, value: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 7.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::same(3.5), theme::RING_BG);
    let width = (rect.width() * value.clamp(0.0, 1.0)).max(4.0);
    let fill = Rect::from_min_size(rect.min, Vec2::new(width, rect.height()));
    ui.painter()
        .rect_filled(fill, Rounding::same(3.5), theme::ACCENT);

    // A slow highlight sweeping through the filled section makes progress feel
    // alive even while a single file step is running.
    if fill.width() > 10.0 {
        let time = ui.ctx().input(|input| input.time) as f32;
        let band_width = 54.0;
        let travel = ((time * 0.55) % 1.0) * (fill.width() + band_width);
        let band = Rect::from_min_size(
            egui::pos2(fill.left() - band_width + travel, fill.top()),
            Vec2::new(band_width, fill.height()),
        );
        ui.painter()
            .with_clip_rect(fill)
            .rect_filled(band, Rounding::same(3.5), theme::with_alpha(Color32::WHITE, 60));
    }
}

fn mark(ui: &mut egui::Ui, color: Color32, glyph: &str, t: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
    let t = theme::ease_out(t);
    let radius = 23.0 * t;
    ui.painter().circle_filled(
        rect.center(),
        radius,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (32.0 * t) as u8),
    );
    ui.painter()
        .circle_stroke(rect.center(), radius - 0.5, Stroke::new(1.0, theme::with_alpha(color, (255.0 * t) as u8)));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(21.0 * t.max(0.2)),
        theme::with_alpha(color, (255.0 * t) as u8),
    );
}
