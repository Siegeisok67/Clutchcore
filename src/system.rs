use std::ffi::OsString;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplaySettingsW, DEVMODEW, DISPLAY_DEVICEW,
    DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_MIRRORING_DRIVER, ENUM_CURRENT_SETTINGS,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
};

// GetDriveTypeW return values (winbase.h).
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::SystemInformation::{
    GetLogicalProcessorInformation, GetSystemInfo, GetTickCount64, GlobalMemoryStatusEx,
    MEMORYSTATUSEX, SYSTEM_INFO, SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, SetPriorityClass, HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    PROCESS_SET_INFORMATION,
};
use windows_sys::Win32::UI::Shell::IsUserAnAdmin;

use crate::model::*;
use crate::presets;
use crate::registry;
use crate::tweaks;
use crate::util;

const GPU_CLASS: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
const BIOS_KEY: &str = r"HARDWARE\DESCRIPTION\System\BIOS";
const CPU_KEY: &str = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0";
const WINDOWS_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

fn wide_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide_array(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..end])
        .to_string_lossy()
        .to_string()
}

fn cpu_info() -> (String, u32, u32, u32) {
    let name = registry::read_string(Hive::Hklm, CPU_KEY, "ProcessorNameString")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown CPU".into());
    let mhz = registry::read_u32(Hive::Hklm, CPU_KEY, "~MHz").unwrap_or(0);
    let logical = unsafe {
        let mut info: SYSTEM_INFO = std::mem::zeroed();
        GetSystemInfo(&mut info);
        info.dwNumberOfProcessors
    };
    let physical = physical_core_count().unwrap_or(logical).max(1);
    (name, mhz, physical, logical.max(1))
}

fn physical_core_count() -> Option<u32> {
    unsafe {
        let mut len: u32 = 0;
        GetLogicalProcessorInformation(std::ptr::null_mut(), &mut len);
        if len == 0 {
            return None;
        }
        let entry = size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
        let capacity = (len as usize / entry).max(1);
        let mut buf: Vec<SYSTEM_LOGICAL_PROCESSOR_INFORMATION> = Vec::with_capacity(capacity);
        let mut written = len;
        let ok = GetLogicalProcessorInformation(buf.as_mut_ptr(), &mut written);
        if ok == 0 {
            return None;
        }
        let count = written as usize / entry;
        buf.set_len(count);
        let cores = buf
            .iter()
            .filter(|item| item.Relationship == 0)
            .count() as u32;
        if cores == 0 {
            None
        } else {
            Some(cores)
        }
    }
}

fn memory_mb() -> (u64, u64) {
    unsafe {
        let mut mem: MEMORYSTATUSEX = std::mem::zeroed();
        mem.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut mem) != 0 {
            let total = mem.ullTotalPhys / 1024 / 1024;
            let avail = mem.ullAvailPhys / 1024 / 1024;
            (total.saturating_sub(avail), total)
        } else {
            (0, 0)
        }
    }
}

fn gpus() -> Vec<GpuInfo> {
    let mut out = Vec::new();
    let mut subs = registry::list_subkeys(Hive::Hklm, GPU_CLASS);
    subs.sort();
    for sub in subs {
        if sub.len() != 4 || !sub.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let path = format!("{GPU_CLASS}\\{sub}");
        let Some(name) = registry::read_string(Hive::Hklm, &path, "DriverDesc") else {
            continue;
        };
        if name.trim().is_empty() {
            continue;
        }
        let driver_version = registry::read_string(Hive::Hklm, &path, "DriverVersion").unwrap_or_default();
        let mut vram_mb = 0;
        if let Ok(Some(raw)) = registry::read(Hive::Hklm, &path, "HardwareInformation.qwMemorySize") {
            if let Some(bytes) = raw.to_u64() {
                vram_mb = bytes / 1024 / 1024;
            }
        }
        if vram_mb == 0 {
            if let Ok(Some(raw)) = registry::read(Hive::Hklm, &path, "HardwareInformation.MemorySize") {
                if let Some(bytes) = raw.to_u32() {
                    vram_mb = bytes as u64 / 1024 / 1024;
                }
            }
        }
        let lower = name.to_ascii_lowercase();
        let software = lower.contains("microsoft basic")
            || lower.contains("remote display")
            || lower.contains("virtual display");
        out.push(GpuInfo {
            name,
            driver_version,
            vram_mb,
            software,
        });
    }
    out.sort_by_key(|g| g.software);
    out
}

fn max_refresh(device: *const u16, width: u32, height: u32) -> u32 {
    let mut best = 0u32;
    let mut index = 0u32;
    loop {
        unsafe {
            let mut mode: DEVMODEW = std::mem::zeroed();
            mode.dmSize = size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsW(device, index, &mut mode) == 0 {
                break;
            }
            if mode.dmPelsWidth == width
                && mode.dmPelsHeight == height
                && mode.dmDisplayFrequency >= 50
            {
                best = best.max(mode.dmDisplayFrequency);
            }
        }
        index += 1;
        if index > 3000 {
            break;
        }
    }
    best
}

fn displays() -> Vec<DisplayInfo> {
    let mut out = Vec::new();
    let mut index = 0u32;
    loop {
        unsafe {
            let mut dd: DISPLAY_DEVICEW = std::mem::zeroed();
            dd.cb = size_of::<DISPLAY_DEVICEW>() as u32;
            if EnumDisplayDevicesW(std::ptr::null(), index, &mut dd, 0) == 0 {
                break;
            }
            index += 1;
            if dd.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0 {
                continue;
            }
            if dd.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER != 0 {
                continue;
            }
            let mut mode: DEVMODEW = std::mem::zeroed();
            mode.dmSize = size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsW(
                dd.DeviceName.as_ptr(),
                ENUM_CURRENT_SETTINGS,
                &mut mode,
            ) == 0
            {
                continue;
            }
            let name = from_wide_array(&dd.DeviceString);
            let device = from_wide_array(&dd.DeviceName);
            let width = mode.dmPelsWidth;
            let height = mode.dmPelsHeight;
            let refresh = mode.dmDisplayFrequency;
            out.push(DisplayInfo {
                name: if name.trim().is_empty() {
                    device.clone()
                } else {
                    name
                },
                device,
                width,
                height,
                refresh,
                max_refresh: max_refresh(dd.DeviceName.as_ptr(), width, height).max(refresh),
                primary: out.is_empty(),
            });
        }
    }
    out
}

fn disks() -> Vec<DiskInfo> {
    let mut out = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = format!("{letter}:\\");
        let wroot = wide_string(&root);
        unsafe {
            let drive_type = GetDriveTypeW(wroot.as_ptr());
            if drive_type != DRIVE_FIXED && drive_type != DRIVE_REMOVABLE && drive_type != DRIVE_REMOTE {
                continue;
            }
            let mut free_to_caller: u64 = 0;
            let mut total: u64 = 0;
            let mut free: u64 = 0;
            if GetDiskFreeSpaceExW(
                wroot.as_ptr(),
                &mut free_to_caller,
                &mut total,
                &mut free,
            ) == 0
            {
                continue;
            }
            if total == 0 {
                continue;
            }
            out.push(DiskInfo {
                mount: root,
                kind: match drive_type {
                    DRIVE_REMOTE => "Network",
                    DRIVE_REMOVABLE => "Removable",
                    DRIVE_FIXED => "Fixed",
                    _ => "Drive",
                }
                .into(),
                total_gb: total as f64 / 1024.0 / 1024.0 / 1024.0,
                free_gb: free as f64 / 1024.0 / 1024.0 / 1024.0,
            });
        }
    }
    out
}

fn startup_items() -> Vec<StartupItem> {
    let mut out = Vec::new();
    let locations: [(Hive, &str, &str); 3] = [
        (
            Hive::Hkcu,
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            "Current user",
        ),
        (
            Hive::Hklm,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            "All users",
        ),
        (
            Hive::Hklm,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run",
            "All users (32-bit)",
        ),
    ];
    for (hive, path, scope) in locations {
        for (name, value) in registry::list_values(hive, path) {
            if name.trim().is_empty() {
                continue;
            }
            let command = value
                .to_string_value()
                .unwrap_or_else(|| value.display());
            out.push(StartupItem {
                name,
                command,
                scope: scope.to_string(),
            });
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata)
            .join(r"Microsoft\Windows\Start Menu\Programs\Startup");
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.trim().is_empty() {
                    continue;
                }
                out.push(StartupItem {
                    name,
                    command: entry.path().display().to_string(),
                    scope: "Startup folder".into(),
                });
            }
        }
    }
    out
}

fn looks_virtual(name: &str, pnp: &str) -> bool {
    let lower = format!("{} {}", name, pnp).to_ascii_lowercase();
    [
        "virtual",
        "vmware",
        "hyper-v",
        "virtualbox",
        "loopback",
        "wan miniport",
        "tap-",
        "tap adapter",
        "vpn",
        "wi-fi direct",
        "bluetooth",
        "kernel debug",
        "npcap",
        "wireshark",
        "teredo",
        "isatap",
        "6to4",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn network_adapters() -> Vec<AdapterInfo> {
    let mut out = Vec::new();
    let mut subs = registry::list_subkeys(Hive::Hklm, tweaks::NIC_CLASS);
    subs.sort();
    for sub in subs {
        if sub.len() != 4 || !sub.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let path = format!("{}\\{}", tweaks::NIC_CLASS, sub);
        let Some(name) = registry::read_string(Hive::Hklm, &path, "DriverDesc") else {
            continue;
        };
        if name.trim().is_empty() {
            continue;
        }
        let driver = registry::read_string(Hive::Hklm, &path, "ProviderName").unwrap_or_default();
        let pnp_id = registry::read_string(Hive::Hklm, &path, "PNPDeviceID").unwrap_or_default();
        let power_management = registry::read_u32(Hive::Hklm, &path, "PnPCapabilities")
            .or_else(|| registry::read_u32(Hive::Hklm, &path, "*PnPCapabilities"));
        out.push(AdapterInfo {
            is_virtual: looks_virtual(&name, &pnp_id),
            name,
            driver,
            pnp_id,
            power_management,
            registry_key: path,
        });
    }
    out.sort_by(|a, b| {
        a.is_virtual
            .cmp(&b.is_virtual)
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

pub fn power_plan() -> Option<PowerPlan> {
    let output = Command::new("powercfg").arg("/getactivescheme").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let lower = text.to_ascii_lowercase();
    let idx = lower.find("guid:")?;
    let rest = text[idx + 5..].trim_start();
    let guid: String = rest
        .chars()
        .take_while(|c| c.is_ascii_hexdigit() || *c == '-')
        .collect();
    if guid.len() < 36 {
        return None;
    }
    let name = text
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .unwrap_or("Unknown plan")
        .trim()
        .to_string();
    Some(PowerPlan {
        guid: guid[..36].to_string(),
        name,
    })
}

fn push_dns(out: &mut Vec<String>, text: Option<String>) {
    if let Some(text) = text {
        for part in text.split([',', ' ', ';']) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if part.parse::<std::net::IpAddr>().is_ok() && !out.iter().any(|x| x == part) {
                out.push(part.to_string());
            }
        }
    }
}

fn dns_servers() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    push_dns(
        &mut out,
        registry::read_string(
            Hive::Hklm,
            r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters",
            "NameServer",
        ),
    );
    push_dns(
        &mut out,
        registry::read_string(
            Hive::Hklm,
            r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters",
            "DhcpNameServer",
        ),
    );
    for sub in registry::list_subkeys(Hive::Hklm, tweaks::TCP_INTERFACES) {
        let path = format!("{}\\{}", tweaks::TCP_INTERFACES, sub);
        push_dns(&mut out, registry::read_string(Hive::Hklm, &path, "NameServer"));
        push_dns(
            &mut out,
            registry::read_string(Hive::Hklm, &path, "DhcpNameServer"),
        );
        if out.len() >= 6 {
            break;
        }
    }
    out.truncate(6);
    out
}

fn board_info() -> (String, String, String) {
    let board_maker =
        registry::read_string(Hive::Hklm, BIOS_KEY, "BaseBoardManufacturer").unwrap_or_default();
    let board_product =
        registry::read_string(Hive::Hklm, BIOS_KEY, "BaseBoardProduct").unwrap_or_default();
    let bios_vendor = registry::read_string(Hive::Hklm, BIOS_KEY, "BIOSVendor").unwrap_or_default();
    let bios_version = registry::read_string(Hive::Hklm, BIOS_KEY, "BIOSVersion").unwrap_or_default();
    let bios_date =
        registry::read_string(Hive::Hklm, BIOS_KEY, "BIOSReleaseDate").unwrap_or_default();
    let sys_maker =
        registry::read_string(Hive::Hklm, BIOS_KEY, "SystemManufacturer").unwrap_or_default();
    let sys_product =
        registry::read_string(Hive::Hklm, BIOS_KEY, "SystemProductName").unwrap_or_default();

    let board = [board_maker.trim(), board_product.trim()]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let bios = [bios_vendor.trim(), bios_version.trim(), bios_date.trim()]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");
    let machine = [sys_maker.trim(), sys_product.trim()]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    (board, bios, machine)
}

fn os_info() -> (String, String, String) {
    let mut product =
        registry::read_string(Hive::Hklm, WINDOWS_KEY, "ProductName").unwrap_or_else(|| "Windows".into());
    let display_version =
        registry::read_string(Hive::Hklm, WINDOWS_KEY, "DisplayVersion").unwrap_or_default();
    let build = registry::read_string(Hive::Hklm, WINDOWS_KEY, "CurrentBuildNumber").unwrap_or_default();
    let ubr = registry::read_u32(Hive::Hklm, WINDOWS_KEY, "UBR").unwrap_or(0);
    if let Ok(build_num) = build.parse::<u32>() {
        if build_num >= 22000 && product.contains("Windows 10") {
            product = product.replace("Windows 10", "Windows 11");
        }
    }
    let version = if display_version.is_empty() {
        format!("build {build}.{ubr}")
    } else {
        format!("{display_version} · build {build}.{ubr}")
    };
    (product, version, build)
}

fn uptime_secs() -> u64 {
    unsafe { GetTickCount64() / 1000 }
}

fn installed_apps() -> Vec<InstalledApp> {
    let roots: [(&str, &str, &str); 3] = [
        (
            "HKLM",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            "All users",
        ),
        (
            "HKLM",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            "All users (32-bit)",
        ),
        (
            "HKCU",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            "Current user",
        ),
    ];
    let mut out = Vec::new();
    for (hive_label, base, scope) in roots {
        let hive = if hive_label == "HKCU" { Hive::Hkcu } else { Hive::Hklm };
        for sub in registry::list_subkeys(hive, base) {
            if out.len() > 700 {
                break;
            }
            let path = format!("{base}\\{sub}");
            let Some(name) = registry::read_string(hive, &path, "DisplayName") else {
                continue;
            };
            if name.trim().is_empty() {
                continue;
            }
            let location = registry::read_string(hive, &path, "InstallLocation").unwrap_or_default();
            out.push(InstalledApp {
                name,
                location,
                scope: scope.to_string(),
            });
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct InstalledApp {
    pub name: String,
    pub location: String,
    pub scope: String,
}

fn steam_libraries() -> Vec<PathBuf> {
    let mut libs = Vec::new();
    let root = registry::read_string(Hive::Hkcu, r"Software\Valve\Steam", "SteamPath")
        .or_else(|| registry::read_string(Hive::Hklm, r"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"));
    if let Some(root) = root {
        let root = PathBuf::from(util::expand_env(&root).replace('/', "\\"));
        libs.push(root.clone());
        let vdf = root.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(vdf) {
            for line in text.lines() {
                let tokens: Vec<&str> = line.trim().split('"').collect();
                if tokens.len() < 4 {
                    continue;
                }
                let key = tokens[1].trim();
                if key == "path" || key.parse::<u32>().is_ok() {
                    let value = tokens[3].replace("\\\\", "\\");
                    if !value.trim().is_empty() {
                        libs.push(PathBuf::from(value));
                    }
                }
            }
        }
    }
    libs.sort();
    libs.dedup();
    libs
}

fn detect_games_from(apps: &[InstalledApp], libs: &[PathBuf]) -> Vec<DetectedGame> {
    let mut out = Vec::new();
    for preset in presets::all() {
        let mut found: Option<String> = None;
        for raw in preset.install_paths {
            let expanded = util::expand_env(raw);
            if Path::new(&expanded).exists() {
                found = Some(expanded);
                break;
            }
        }
        if found.is_none() {
            'apps: for app in apps {
                let lower = app.name.to_ascii_lowercase();
                if preset
                    .match_names
                    .iter()
                    .any(|m| lower.contains(&m.to_ascii_lowercase()))
                    && !app.location.trim().is_empty()
                    && Path::new(&app.location).exists()
                {
                    found = Some(app.location.clone());
                    break 'apps;
                }
            }
        }
        if found.is_none() {
            'steam: for folder in preset.steam_folders {
                for lib in libs {
                    let candidate = lib.join("steamapps").join("common").join(folder);
                    if candidate.exists() {
                        found = Some(candidate.display().to_string());
                        break 'steam;
                    }
                }
            }
        }
        if let Some(path) = found {
            out.push(DetectedGame {
                preset_id: preset.id.to_string(),
                game: preset.game.to_string(),
                path,
            });
        }
    }
    out
}

pub fn detect_games() -> Vec<DetectedGame> {
    let apps = installed_apps();
    let libs = steam_libraries();
    detect_games_from(&apps, &libs)
}

pub fn scan() -> SystemReport {
    let mut report = SystemReport::default();
    report.admin = is_admin();
    report.hostname = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "This PC".into());

    let (cpu, mhz, physical, logical) = cpu_info();
    report.cpu_name = cpu;
    report.cpu_mhz = mhz;
    report.physical_cores = physical;
    report.logical_cores = logical;

    let (used, total) = memory_mb();
    report.ram_used_mb = used;
    report.ram_total_mb = total;

    report.gpus = gpus();
    if report.gpus.is_empty() {
        report.errors.push("Could not read GPU information from the registry.".into());
    }

    let (board, bios, machine) = board_info();
    report.board = board;
    report.bios = bios;
    report.machine = machine;

    let (os_name, os_version, os_build) = os_info();
    report.os_name = os_name;
    report.os_version = os_version;
    report.os_build = os_build;
    report.uptime_secs = uptime_secs();

    report.displays = displays();
    if report.displays.is_empty() {
        report
            .errors
            .push("Could not enumerate displays; refresh-rate suggestions are unavailable.".into());
    }

    report.disks = disks();
    report.startup_items = startup_items();
    report.adapters = network_adapters();
    report.dns_servers = dns_servers();
    report.power_plan = power_plan();

    let apps = installed_apps();
    report.installed_app_count = apps.len();
    let libs = steam_libraries();
    report.detected_games = detect_games_from(&apps, &libs);

    report
}

pub fn running_games() -> Vec<RunningGame> {
    let mut out = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
            return out;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            let exe = from_wide_array(&entry.szExeFile);
            if !exe.is_empty() {
                for preset in presets::all() {
                    if preset
                        .exe_names
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(&exe))
                    {
                        out.push(RunningGame {
                            pid: entry.th32ProcessID,
                            exe: exe.clone(),
                            preset_id: preset.id.to_string(),
                            game: preset.game.to_string(),
                        });
                        break;
                    }
                }
            }
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
    }
    out.sort_by(|a, b| a.game.cmp(&b.game));
    out.dedup_by(|a, b| a.pid == b.pid);
    out
}

pub fn set_process_priority(pid: u32, high: bool) -> Result<(), String> {
    unsafe {
        let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
        if handle.is_null() {
            return Err(format!(
                "Could not open process {pid}: {}. Try running Clutchcore as administrator.",
                std::io::Error::last_os_error()
            ));
        }
        let class = if high {
            HIGH_PRIORITY_CLASS
        } else {
            NORMAL_PRIORITY_CLASS
        };
        let ok = SetPriorityClass(handle, class);
        let err = std::io::Error::last_os_error();
        CloseHandle(handle);
        if ok == 0 {
            Err(format!("Windows rejected the priority change: {err}"))
        } else {
            Ok(())
        }
    }
}

#[repr(C)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    fn value(&self) -> u64 {
        ((self.high as u64) << 32) | self.low as u64
    }
}

extern "system" {
    fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
}

fn system_times() -> (u64, u64) {
    unsafe {
        let mut idle: FileTime = std::mem::zeroed();
        let mut kernel: FileTime = std::mem::zeroed();
        let mut user: FileTime = std::mem::zeroed();
        if GetSystemTimes(&mut idle, &mut kernel, &mut user) == 0 {
            return (0, 0);
        }
        let idle = idle.value();
        // Kernel time includes idle time.
        let total = kernel.value() + user.value();
        (idle, total)
    }
}

pub struct LiveMonitor {
    prev: Option<(u64, u64)>,
}

impl LiveMonitor {
    pub fn new() -> Self {
        let mut monitor = Self { prev: None };
        let _ = monitor.sample();
        monitor
    }

    pub fn sample(&mut self) -> LiveStats {
        let (idle, total) = system_times();
        let cpu = match self.prev {
            Some((prev_idle, prev_total)) if total > prev_total && idle >= prev_idle => {
                let dt = (total - prev_total) as f64;
                let di = (idle - prev_idle) as f64;
                ((1.0 - di / dt) * 100.0).clamp(0.0, 100.0) as f32
            }
            _ => 0.0,
        };
        self.prev = Some((idle, total));
        let (used, total_mb) = memory_mb();
        LiveStats {
            cpu_pct: cpu,
            ram_used_mb: used,
            ram_total_mb: total_mb,
            uptime_secs: uptime_secs(),
            updated_unix: util::unix_now(),
        }
    }
}

pub fn report_text(report: &SystemReport) -> String {
    let mut out = String::new();
    out.push_str("CLUTCHCORE — SYSTEM REPORT\n");
    out.push_str(&format!("Generated: {}\n", util::fmt_datetime(util::unix_now())));
    out.push_str(&format!("Computer: {}\n\n", report.hostname));

    out.push_str("SYSTEM\n");
    out.push_str(&format!("CPU: {} @ {} MHz\n", report.cpu_name, report.cpu_mhz));
    out.push_str(&format!(
        "Cores: {} physical / {} logical\n",
        report.physical_cores, report.logical_cores
    ));
    out.push_str(&format!(
        "Memory: {} MB installed, {} MB in use\n",
        report.ram_total_mb, report.ram_used_mb
    ));
    out.push_str(&format!("Motherboard: {}\n", if report.board.is_empty() { "unknown" } else { &report.board }));
    out.push_str(&format!("BIOS: {}\n", if report.bios.is_empty() { "unknown" } else { &report.bios }));
    out.push_str(&format!("Model: {}\n", if report.machine.is_empty() { "unknown" } else { &report.machine }));
    out.push_str(&format!("OS: {} ({})\n", report.os_name, report.os_version));
    out.push_str(&format!("Uptime: {}\n", util::human_uptime(report.uptime_secs)));
    out.push_str(&format!(
        "Power plan: {}\n",
        report
            .power_plan
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("unknown")
    ));
    out.push_str(&format!(
        "Administrator: {}\n",
        if report.admin { "yes" } else { "no" }
    ));
    out.push('\n');

    out.push_str("GRAPHICS\n");
    if report.gpus.is_empty() {
        out.push_str("No GPU detected.\n");
    }
    for gpu in &report.gpus {
        out.push_str(&format!(
            "- {}{} (driver {}{})\n",
            gpu.name,
            if gpu.software { " [software adapter]" } else { "" },
            if gpu.driver_version.is_empty() { "unknown" } else { &gpu.driver_version },
            if gpu.vram_mb > 0 {
                format!(", {} MB VRAM", gpu.vram_mb)
            } else {
                String::new()
            }
        ));
    }
    out.push('\n');

    out.push_str("DISPLAYS\n");
    for d in &report.displays {
        out.push_str(&format!(
            "- {} · {}×{} @ {} Hz (max {} Hz){}\n",
            d.name,
            d.width,
            d.height,
            d.refresh,
            d.max_refresh,
            if d.primary { " [primary]" } else { "" }
        ));
    }
    out.push('\n');

    out.push_str("STORAGE\n");
    for disk in &report.disks {
        out.push_str(&format!(
            "- {} {} · {:.1} GB free of {:.1} GB\n",
            disk.mount, disk.kind, disk.free_gb, disk.total_gb
        ));
    }
    out.push('\n');

    out.push_str("NETWORK ADAPTERS\n");
    for adapter in &report.adapters {
        out.push_str(&format!(
            "- {}{} · {}{}\n",
            adapter.name,
            if adapter.is_virtual { " [virtual]" } else { "" },
            adapter.driver,
            match adapter.power_management {
                Some(v) => format!(" · power flags {v}"),
                None => String::new(),
            }
        ));
    }
    if !report.dns_servers.is_empty() {
        out.push_str(&format!("DNS: {}\n", report.dns_servers.join(", ")));
    }
    out.push('\n');

    out.push_str("DETECTED GAMES\n");
    if report.detected_games.is_empty() {
        out.push_str("None of the supported presets were detected.\n");
    }
    for game in &report.detected_games {
        out.push_str(&format!("- {} · {}\n", game.game, game.path));
    }
    out.push('\n');

    out.push_str(&format!(
        "STARTUP ITEMS ({})\n",
        report.startup_items.len()
    ));
    for item in &report.startup_items {
        out.push_str(&format!(
            "- {} [{}] · {}\n",
            item.name,
            item.scope,
            util::truncate(&item.command, 110)
        ));
    }
    out.push('\n');

    if !report.errors.is_empty() {
        out.push_str("SCAN NOTES\n");
        for error in &report.errors {
            out.push_str(&format!("- {error}\n"));
        }
    }
    out
}

pub fn export_report(report: &SystemReport) -> Result<PathBuf, String> {
    util::ensure_dirs().map_err(|e| format!("could not create data folder: {e}"))?;
    let path = util::data_dir().join(format!(
        "system-report-{}.txt",
        util::file_stamp(util::unix_now())
    ));
    util::write_text(&path, &report_text(report))
        .map_err(|e| format!("could not write report: {e}"))?;
    Ok(path)
}
