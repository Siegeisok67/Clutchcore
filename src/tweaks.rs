use std::collections::BTreeMap;
use std::process::Command;

use crate::model::*;
use crate::registry;
use crate::system;

const MOUSE: &str = r"Control Panel\Mouse";
const GAME_CONFIG_STORE: &str = r"System\GameConfigStore";
const GAME_DVR: &str = r"Software\Microsoft\Windows\CurrentVersion\GameDVR";
const GAME_BAR: &str = r"Software\Microsoft\GameBar";
const GAME_BAR_VERSION: &str = r"Software\Microsoft\Windows\CurrentVersion\GameBar";
const GRAPHICS_DRIVERS: &str = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers";
const PRIORITY_CONTROL: &str = r"SYSTEM\CurrentControlSet\Control\PriorityControl";
const MMCSS_GAMES: &str =
    r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games";
const MMCSS_ROOT: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
const POWER_THROTTLING: &str = r"SYSTEM\CurrentControlSet\Control\Power\PowerThrottling";
const VISUAL_EFFECTS: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects";
const EXPLORER_ADVANCED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const DESKTOP: &str = r"Control Panel\Desktop";
const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const BACKGROUND_APPS: &str =
    r"Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications";
const SEARCH: &str = r"Software\Microsoft\Windows\CurrentVersion\Search";

pub const TCP_INTERFACES: &str =
    r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";
pub const NIC_CLASS: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";

const MOUSE_ACCEL_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: MOUSE,
        name: "MouseSpeed",
        value: DesiredValue::Str("0"),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: MOUSE,
        name: "MouseThreshold1",
        value: DesiredValue::Str("0"),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: MOUSE,
        name: "MouseThreshold2",
        value: DesiredValue::Str("0"),
    },
];

const GAME_DVR_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_CONFIG_STORE,
        name: "GameDVR_Enabled",
        value: DesiredValue::Dword(0),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_DVR,
        name: "AppCaptureEnabled",
        value: DesiredValue::Dword(0),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_DVR,
        name: "HistoricalCaptureEnabled",
        value: DesiredValue::Dword(0),
    },
];

const GAME_MODE_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_BAR,
        name: "AllowAutoGameMode",
        value: DesiredValue::Dword(1),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_BAR,
        name: "AutoGameModeEnabled",
        value: DesiredValue::Dword(1),
    },
];

const FULLSCREEN_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_CONFIG_STORE,
        name: "GameDVR_FSEBehaviorMode",
        value: DesiredValue::Dword(2),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_CONFIG_STORE,
        name: "GameDVR_HonorUserFSEBehaviorMode",
        value: DesiredValue::Dword(1),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_CONFIG_STORE,
        name: "GameDVR_DXGIHonorFSEWindowsCompatible",
        value: DesiredValue::Dword(1),
    },
];

const XBOX_BAR_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_BAR,
        name: "UseNexusForGameBarEnabled",
        value: DesiredValue::Dword(0),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_BAR,
        name: "ShowStartupPanel",
        value: DesiredValue::Dword(0),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: GAME_BAR_VERSION,
        name: "GamePanelStartupTipIndex",
        value: DesiredValue::Dword(3),
    },
];

const HAGS_REGS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: GRAPHICS_DRIVERS,
    name: "HwSchMode",
    value: DesiredValue::Dword(2),
}];

const MMCSS_GAMES_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: MMCSS_GAMES,
        name: "GPU Priority",
        value: DesiredValue::Dword(8),
    },
    RegTarget {
        hive: Hive::Hklm,
        path: MMCSS_GAMES,
        name: "Priority",
        value: DesiredValue::Dword(6),
    },
    RegTarget {
        hive: Hive::Hklm,
        path: MMCSS_GAMES,
        name: "Scheduling Category",
        value: DesiredValue::Str("High"),
    },
    RegTarget {
        hive: Hive::Hklm,
        path: MMCSS_GAMES,
        name: "SFIO Priority",
        value: DesiredValue::Str("High"),
    },
];

const PRIORITY_SEP_REGS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: PRIORITY_CONTROL,
    name: "Win32PrioritySeparation",
    value: DesiredValue::Dword(38),
}];

const POWER_THROTTLING_REGS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hklm,
    path: POWER_THROTTLING,
    name: "PowerThrottlingOff",
    value: DesiredValue::Dword(1),
}];

const VISUAL_EFFECT_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: VISUAL_EFFECTS,
        name: "VisualFXSetting",
        value: DesiredValue::Dword(2),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: DESKTOP,
        name: "MenuShowDelay",
        value: DesiredValue::Str("0"),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: EXPLORER_ADVANCED,
        name: "TaskbarAnimations",
        value: DesiredValue::Dword(0),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: DESKTOP,
        name: "DragFullWindows",
        value: DesiredValue::Str("0"),
    },
];

const TRANSPARENCY_REGS: &[RegTarget] = &[RegTarget {
    hive: Hive::Hkcu,
    path: PERSONALIZE,
    name: "EnableTransparency",
    value: DesiredValue::Dword(0),
}];

const BACKGROUND_APPS_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hkcu,
        path: BACKGROUND_APPS,
        name: "GlobalUserDisabled",
        value: DesiredValue::Dword(1),
    },
    RegTarget {
        hive: Hive::Hkcu,
        path: SEARCH,
        name: "BackgroundAppGlobalToggle",
        value: DesiredValue::Dword(0),
    },
];

const NETWORK_THROTTLING_REGS: &[RegTarget] = &[
    RegTarget {
        hive: Hive::Hklm,
        path: MMCSS_ROOT,
        name: "NetworkThrottlingIndex",
        value: DesiredValue::Dword(0xFFFF_FFFF),
    },
    RegTarget {
        hive: Hive::Hklm,
        path: MMCSS_ROOT,
        name: "SystemResponsiveness",
        value: DesiredValue::Dword(10),
    },
];

pub static SPECS: &[TweakSpec] = &[
    TweakSpec {
        id: "mouse_accel_off",
        name: "Disable mouse acceleration",
        category: Category::Input,
        impacts: &[Impact::InputDelay],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Removes Windows' pointer acceleration curve for 1:1 mouse movement.",
        detail: "Windows applies 'Enhance pointer precision' on top of raw mouse input. With it on, how far the cursor travels depends on how fast you move. Turning it off makes flicks predictable and matches what most competitive players use. Games that read raw input ignore this, but it still affects menus, map screens and older titles.",
        revert_hint: "Your previous pointer-speed values are restored from the backup vault.",
        kind: TweakKind::Regs(MOUSE_ACCEL_REGS),
    },
    TweakSpec {
        id: "game_dvr_off",
        name: "Disable background game recording (Game DVR)",
        category: Category::Latency,
        impacts: &[Impact::InputDelay, Impact::Fps, Impact::FramePacing],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Stops Xbox Game DVR from keeping a recording buffer while you play.",
        detail: "Background capture keeps a rolling buffer and hooks the graphics pipeline, which costs frames and adds encoding work. This disables the capture buffer and the Game DVR switches for your user. Game clips you already saved are untouched.",
        revert_hint: "Restores your previous Game DVR capture settings.",
        kind: TweakKind::Regs(GAME_DVR_REGS),
    },
    TweakSpec {
        id: "fullscreen_optimizations_off",
        name: "Prefer true exclusive fullscreen",
        category: Category::Latency,
        impacts: &[Impact::InputDelay, Impact::FramePacing],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Tells Windows to stop layering fullscreen optimizations over fullscreen games.",
        detail: "Fullscreen optimizations run a 'fullscreen' game through a borderless-window compositor path. That can add an extra compositor pass of latency. This sets the per-user compatibility flags that prefer exclusive fullscreen. If alt-tabbing becomes slow in a specific game, revert this or edit that game's compatibility options.",
        revert_hint: "Restores the previous fullscreen-optimization flags.",
        kind: TweakKind::Regs(FULLSCREEN_REGS),
    },
    TweakSpec {
        id: "game_mode_on",
        name: "Enable Windows Game Mode",
        category: Category::System,
        impacts: &[Impact::Fps, Impact::FramePacing],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Prioritises the foreground game so background work yields CPU/GPU time.",
        detail: "Game Mode asks Windows to keep background tasks from stealing resources while a game is focused. It is low risk and helps most 4–16 thread systems. A few capture/streaming setups behave better with it off — test your own setup before a ranked session.",
        revert_hint: "Restores your previous Game Mode setting.",
        kind: TweakKind::Regs(GAME_MODE_REGS),
    },
    TweakSpec {
        id: "xbox_bar_off",
        name: "Disable the Xbox Game Bar overlay",
        category: Category::Latency,
        impacts: &[Impact::Fps],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Turns off the Game Bar overlay, its startup panel and tips.",
        detail: "The Game Bar overlay hooks the game's swap chain and can appear over fullscreen games. This disables its controller-shortcut entry points and startup hints for your user. The Xbox app and Game Pass themselves still work.",
        revert_hint: "Restores the Game Bar overlay settings.",
        kind: TweakKind::Regs(XBOX_BAR_REGS),
    },
    TweakSpec {
        id: "hags_on",
        name: "Enable Hardware-accelerated GPU Scheduling",
        category: Category::System,
        impacts: &[Impact::FramePacing, Impact::Fps],
        risk: Risk::Medium,
        requires_admin: true,
        requires_reboot: true,
        summary: "Lets the GPU schedule its own work to reduce frame-delivery latency.",
        detail: "HAGS hands work scheduling to the GPU instead of Windows' graphics scheduler. On modern GPUs and drivers it can reduce frame-delivery latency and improve pacing. It needs a reboot, and a small number of older drivers stutter with it — test before a ranked session.",
        revert_hint: "Restores the previous HAGS setting; reboot to apply.",
        kind: TweakKind::Regs(HAGS_REGS),
    },
    TweakSpec {
        id: "power_plan_high",
        name: "High performance power plan",
        category: Category::System,
        impacts: &[Impact::Fps, Impact::InputDelay],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Keeps CPU clocks up so frame-critical moments are not spent waking cores.",
        detail: "Balanced and power-saver plans park cores and lower clocks to save power, which can add wake-up latency. High performance keeps the CPU ready. On laptops this raises heat and battery drain, so keep the charger connected.",
        revert_hint: "Switches back to the exact power plan that was active before Clutchcore.",
        kind: TweakKind::PowerPlan,
    },
    TweakSpec {
        id: "mmcss_games",
        name: "Raise the Games multimedia task priority",
        category: Category::System,
        impacts: &[Impact::InputDelay, Impact::Fps],
        risk: Risk::Medium,
        requires_admin: true,
        requires_reboot: false,
        summary: "Gives the Windows 'Games' MMCSS task high scheduling and GPU priority.",
        detail: "The Multimedia Class Scheduler Service reserves CPU time for games and media. Raising the Games task's priority and GPU priority is a long-standing tuning step for smoother frame delivery. It applies after the next service refresh or a reboot.",
        revert_hint: "Restores the original MMCSS Games task values.",
        kind: TweakKind::Regs(MMCSS_GAMES_REGS),
    },
    TweakSpec {
        id: "win32_priority_separation",
        name: "Boost foreground scheduling (PrioritySeparation 38)",
        category: Category::System,
        impacts: &[Impact::InputDelay],
        risk: Risk::Medium,
        requires_admin: true,
        requires_reboot: false,
        summary: "Favours the foreground process with shorter, fixed CPU time slices.",
        detail: "Win32PrioritySeparation controls how aggressively Windows favours the foreground app. 38 is the common competitive value: short fixed quanta with a foreground boost. Background apps can feel slightly less responsive while a game runs.",
        revert_hint: "Restores your original Win32PrioritySeparation value.",
        kind: TweakKind::Regs(PRIORITY_SEP_REGS),
    },
    TweakSpec {
        id: "power_throttling_off",
        name: "Disable Windows Power Throttling",
        category: Category::System,
        impacts: &[Impact::Fps],
        risk: Risk::Medium,
        requires_admin: true,
        requires_reboot: false,
        summary: "Stops Windows from capping process performance under power policy.",
        detail: "Power Throttling can cap process performance based on power policy even on desktops. Disabling it removes that cap and may increase power draw and heat. It matters most on laptops and handhelds.",
        revert_hint: "Restores the previous Power Throttling setting.",
        kind: TweakKind::Regs(POWER_THROTTLING_REGS),
    },
    TweakSpec {
        id: "visual_effects_performance",
        name: "Tune desktop visuals for performance",
        category: Category::Visuals,
        impacts: &[Impact::Fps],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Removes menu fades, taskbar animations and other shell eye candy.",
        detail: "Windows spends GPU and CPU time on menu fades, window animations and full-window drag previews. This switches the Visual Effects preference to 'Adjust for best performance', removes the menu delay and disables taskbar animations. Purely cosmetic.",
        revert_hint: "Restores your original visual-effects preferences.",
        kind: TweakKind::Regs(VISUAL_EFFECT_REGS),
    },
    TweakSpec {
        id: "transparency_off",
        name: "Disable Windows transparency effects",
        category: Category::Visuals,
        impacts: &[Impact::Fps],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Stops the desktop compositor from blurring the background continuously.",
        detail: "Transparency makes the compositor blur the desktop behind windows. Turning it off removes that work — a small but free frame-time win on integrated GPUs and while a game runs borderless.",
        revert_hint: "Restores your previous transparency setting.",
        kind: TweakKind::Regs(TRANSPARENCY_REGS),
    },
    TweakSpec {
        id: "background_apps_off",
        name: "Stop background Store app activity",
        category: Category::System,
        impacts: &[Impact::Fps],
        risk: Risk::Low,
        requires_admin: false,
        requires_reboot: false,
        summary: "Blocks packaged apps from running background tasks while you play.",
        detail: "Windows lets Store apps wake in the background for updates, live tiles and notifications. This disables background execution for your user and turns off the search background toggle, freeing CPU, disk and network. Some apps only update when you open them.",
        revert_hint: "Restores your previous background-app settings.",
        kind: TweakKind::Regs(BACKGROUND_APPS_REGS),
    },
    TweakSpec {
        id: "tcp_latency",
        name: "Disable Nagle and delayed ACKs",
        category: Category::Network,
        impacts: &[Impact::Ping],
        risk: Risk::Medium,
        requires_admin: true,
        requires_reboot: false,
        summary: "Stops small packets being batched or held back on active network adapters.",
        detail: "Nagle batches small TCP packets and delayed ACK waits before acknowledging; both can add 10–200 ms to small packets. This writes TcpAckFrequency=1 and TCPNoDelay=1 on every active IPv4 interface. Most shooters use UDP, but this still helps launchers, voice chat, anti-cheat handshakes and TCP-based games.",
        revert_hint: "Restores the exact per-interface TCP values, including removing ones that did not exist.",
        kind: TweakKind::TcpInterfaces,
    },
    TweakSpec {
        id: "network_throttling_off",
        name: "Remove the multimedia network throttle",
        category: Category::Network,
        impacts: &[Impact::Ping],
        risk: Risk::Medium,
        requires_admin: true,
        requires_reboot: false,
        summary: "Clears Windows' 10 ms packet throttle and lowers reserved multimedia bandwidth.",
        detail: "By default Windows throttles non-multimedia network packets to 10 ms intervals and reserves part of the stack for multimedia. Clearing NetworkThrottlingIndex and lowering SystemResponsiveness gives game traffic a cleaner path. Some audio software may notice the changed reserve.",
        revert_hint: "Restores the original throttling and responsiveness values.",
        kind: TweakKind::Regs(NETWORK_THROTTLING_REGS),
    },
    TweakSpec {
        id: "nic_power_off",
        name: "Disable network adapter power saving",
        category: Category::Network,
        impacts: &[Impact::Ping],
        risk: Risk::Medium,
        requires_admin: true,
        requires_reboot: false,
        summary: "Stops Windows from putting physical Ethernet/Wi-Fi adapters to sleep.",
        detail: "Adapter power management can drop the NIC into a low-power state, causing latency spikes and occasional reconnect hitches. This clears the power-saving flag on physical adapters. Wake-on-LAN and some vendor power features may stop working; revert restores the exact original values. A reboot or adapter disable/enable may be needed for full effect.",
        revert_hint: "Restores each adapter's original power-management value.",
        kind: TweakKind::NicPower,
    },
];

pub fn specs() -> &'static [TweakSpec] {
    SPECS
}

pub fn find(id: &str) -> Option<&'static TweakSpec> {
    SPECS.iter().find(|s| s.id == id)
}

pub fn tweak_names(ids: &[String]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| find(id).map(|s| s.name.to_string()))
        .collect()
}

#[derive(Clone, Debug)]
pub struct ResolvedTarget {
    pub hive: Hive,
    pub path: String,
    pub name: String,
    pub desired: DesiredValue,
}

pub fn active_tcp_interface_paths() -> Vec<String> {
    let mut out = Vec::new();
    for sub in registry::list_subkeys(Hive::Hklm, TCP_INTERFACES) {
        let path = format!("{TCP_INTERFACES}\\{sub}");
        let dhcp_ip = registry::read_string(Hive::Hklm, &path, "DhcpIPAddress").unwrap_or_default();
        let static_ip = registry::read_string(Hive::Hklm, &path, "IPAddress").unwrap_or_default();
        let gateway = registry::read_string(Hive::Hklm, &path, "DefaultGateway").unwrap_or_default();
        let has_dhcp = !dhcp_ip.trim().is_empty() && dhcp_ip.trim() != "0.0.0.0";
        let has_static = !static_ip.trim().is_empty();
        let has_gateway = !gateway.trim().is_empty();
        if has_dhcp || has_static || has_gateway {
            out.push(path);
        }
    }
    out
}

pub fn resolve_targets(spec: &TweakSpec) -> Result<Vec<ResolvedTarget>, String> {
    match spec.kind {
        TweakKind::Regs(regs) => Ok(regs
            .iter()
            .map(|r| ResolvedTarget {
                hive: r.hive,
                path: r.path.to_string(),
                name: r.name.to_string(),
                desired: r.value,
            })
            .collect()),
        TweakKind::TcpInterfaces => {
            let paths = active_tcp_interface_paths();
            if paths.is_empty() {
                return Err("No active IPv4 network interface was found.".into());
            }
            let mut out = Vec::new();
            for path in paths {
                out.push(ResolvedTarget {
                    hive: Hive::Hklm,
                    path: path.clone(),
                    name: "TcpAckFrequency".into(),
                    desired: DesiredValue::Dword(1),
                });
                out.push(ResolvedTarget {
                    hive: Hive::Hklm,
                    path,
                    name: "TCPNoDelay".into(),
                    desired: DesiredValue::Dword(1),
                });
            }
            Ok(out)
        }
        TweakKind::NicPower => {
            let adapters = system::network_adapters();
            let mut out = Vec::new();
            for adapter in adapters {
                if adapter.is_virtual {
                    continue;
                }
                let path = adapter.registry_key.clone();
                let name = if registry::value_exists(Hive::Hklm, &path, "*PnPCapabilities") {
                    "*PnPCapabilities"
                } else {
                    "PnPCapabilities"
                };
                out.push(ResolvedTarget {
                    hive: Hive::Hklm,
                    path,
                    name: name.to_string(),
                    desired: DesiredValue::Dword(24),
                });
            }
            if out.is_empty() {
                return Err("No physical network adapters were found.".into());
            }
            Ok(out)
        }
        TweakKind::PowerPlan => Ok(Vec::new()),
    }
}

pub struct Detection {
    pub statuses: BTreeMap<String, TweakStatus>,
    pub plan: Option<PowerPlan>,
}

pub fn detect_all() -> Detection {
    let plan = system::power_plan();
    let mut statuses = BTreeMap::new();
    for spec in SPECS {
        statuses.insert(spec.id.to_string(), detect(spec, plan.as_ref()));
    }
    Detection { statuses, plan }
}

pub fn detect_one(id: &str) -> TweakStatus {
    let plan = system::power_plan();
    find(id)
        .map(|s| detect(s, plan.as_ref()))
        .unwrap_or(TweakStatus {
            state: TweakState::Unknown,
            detail: "Unknown tweak".into(),
        })
}

pub fn detect(spec: &TweakSpec, plan: Option<&PowerPlan>) -> TweakStatus {
    if matches!(spec.kind, TweakKind::PowerPlan) {
        return match plan {
            Some(p) if p.is_high_performance() => TweakStatus {
                state: TweakState::Applied,
                detail: format!("Active plan: {}", p.name),
            },
            Some(p) => TweakStatus {
                state: TweakState::NotApplied,
                detail: format!("Active plan: {}", p.name),
            },
            None => TweakStatus {
                state: TweakState::Unknown,
                detail: "Could not read the active power plan.".into(),
            },
        };
    }

    let targets = match resolve_targets(spec) {
        Ok(t) => t,
        Err(e) => {
            return TweakStatus {
                state: TweakState::Unknown,
                detail: e,
            }
        }
    };
    if targets.is_empty() {
        return TweakStatus {
            state: TweakState::Unknown,
            detail: "No targets resolved.".into(),
        };
    }

    let mut matched = 0usize;
    let mut errors = 0usize;
    for target in &targets {
        match registry::read(target.hive, &target.path, &target.name) {
            Ok(current) => {
                if registry::matches_desired(&current, target.desired) {
                    matched += 1;
                }
            }
            Err(_) => errors += 1,
        }
    }

    let total = targets.len();
    if errors == total {
        return TweakStatus {
            state: TweakState::Unknown,
            detail: "Windows blocked reading these settings.".into(),
        };
    }

    let state = if matched == total {
        TweakState::Applied
    } else if matched == 0 {
        TweakState::NotApplied
    } else {
        TweakState::Partial
    };
    let detail = if state == TweakState::Applied {
        format!("{total} of {total} settings match")
    } else {
        format!("{matched} of {total} settings match")
    };
    TweakStatus { state, detail }
}

pub fn describe(spec: &TweakSpec) -> Vec<String> {
    match spec.kind {
        TweakKind::Regs(regs) => regs
            .iter()
            .map(|r| {
                format!(
                    "{}\\{} · {} = {}",
                    r.hive.label(),
                    r.path,
                    r.name,
                    r.value.display()
                )
            })
            .collect(),
        TweakKind::TcpInterfaces => vec![
            "Every active IPv4 interface".into(),
            "TcpAckFrequency = 1 (no delayed ACK)".into(),
            "TCPNoDelay = 1 (Nagle disabled)".into(),
        ],
        TweakKind::NicPower => vec![
            "Every physical Ethernet / Wi-Fi adapter".into(),
            "PnPCapabilities = 24 (power saving disabled)".into(),
        ],
        TweakKind::PowerPlan => vec![
            "Active Windows power plan → High performance".into(),
            "The previous plan GUID is backed up before switching".into(),
        ],
    }
}

pub fn impact_text(spec: &TweakSpec) -> String {
    spec.impacts
        .iter()
        .map(|i| i.label())
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn run_powercfg(args: &[&str]) -> Result<String, String> {
    let output = Command::new("powercfg")
        .args(args)
        .output()
        .map_err(|e| format!("could not run powercfg: {e}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if output.status.success() {
        Ok(text)
    } else {
        Err(text.trim().to_string())
    }
}

pub fn set_power_plan_high() -> Result<(), String> {
    let mut last_err = String::new();
    for arg in ["SCHEME_MIN", "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"] {
        match run_powercfg(&["/setactive", arg]) {
            Ok(_) => {
                if system::power_plan()
                    .map(|p| p.is_high_performance())
                    .unwrap_or(false)
                {
                    return Ok(());
                }
            }
            Err(e) => last_err = e,
        }
    }

    if let Ok(text) = run_powercfg(&["-duplicatescheme", "e9a42b02-d5df-448d-aa00-03f14749eb61"]) {
        if let Some(guid) = extract_guid(&text) {
            if run_powercfg(&["/setactive", &guid]).is_ok() {
                return Ok(());
            }
        }
    }

    Err(if last_err.is_empty() {
        "Windows rejected the power-plan change. Try running Clutchcore as administrator.".into()
    } else {
        format!("Windows rejected the power-plan change: {last_err}")
    })
}

pub fn restore_power_plan(guid: &str) -> Result<(), String> {
    run_powercfg(&["/setactive", guid])
        .map(|_| ())
        .map_err(|e| format!("could not restore power plan {guid}: {e}"))
}

fn extract_guid(text: &str) -> Option<String> {
    let idx = text.to_ascii_lowercase().find("guid:")?;
    let rest = &text[idx + 5..];
    let candidate: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_hexdigit() || *c == '-')
        .collect();
    if candidate.len() >= 36 {
        Some(candidate[..36].to_string())
    } else {
        None
    }
}

pub fn is_admin_required_in(ids: &[String]) -> bool {
    ids.iter()
        .filter_map(|id| find(id))
        .any(|s| s.requires_admin)
}

pub fn requires_reboot_in(ids: &[String]) -> bool {
    ids.iter()
        .filter_map(|id| find(id))
        .any(|s| s.requires_reboot)
}

pub fn risky_in(ids: &[String]) -> Vec<&'static TweakSpec> {
    ids.iter()
        .filter_map(|id| find(id))
        .filter(|s| s.risk != Risk::Low)
        .collect()
}
