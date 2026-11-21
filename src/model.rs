use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Input,
    Latency,
    Network,
    Visuals,
    System,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Input,
        Category::Latency,
        Category::Network,
        Category::Visuals,
        Category::System,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Input => "Input",
            Category::Latency => "Latency",
            Category::Network => "Network",
            Category::Visuals => "Visuals",
            Category::System => "System",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Category::Input => "Pointer and display response",
            Category::Latency => "Frame delivery and system scheduling",
            Category::Network => "Ping, jitter and Windows TCP stack",
            Category::Visuals => "Desktop effects that cost frames",
            Category::System => "Windows tuning and background load",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Impact {
    InputDelay,
    FramePacing,
    Fps,
    Ping,
}

impl Impact {
    pub fn label(self) -> &'static str {
        match self {
            Impact::InputDelay => "Input delay",
            Impact::FramePacing => "Frame pacing",
            Impact::Fps => "FPS",
            Impact::Ping => "Ping",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::Low => "Low risk",
            Risk::Medium => "Medium risk",
            Risk::High => "High risk",
        }
    }

    pub fn describes(self) -> &'static str {
        match self {
            Risk::Low => "Cosmetic or fully reversible with no known side effects.",
            Risk::Medium => "Reversible, but may change power use or a feature you use.",
            Risk::High => "Changes system behaviour noticeably. Read the details first.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hive {
    Hkcu,
    Hklm,
}

impl Hive {
    pub fn label(self) -> &'static str {
        match self {
            Hive::Hkcu => "HKCU",
            Hive::Hklm => "HKLM",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DesiredValue {
    Dword(u32),
    Str(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct RegTarget {
    pub hive: Hive,
    pub path: &'static str,
    pub name: &'static str,
    pub value: DesiredValue,
}

#[derive(Clone, Copy, Debug)]
pub enum TweakKind {
    Regs(&'static [RegTarget]),
    TcpInterfaces,
    NicPower,
    PowerPlan,
}

#[derive(Clone, Copy, Debug)]
pub struct TweakSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub impacts: &'static [Impact],
    pub risk: Risk,
    pub requires_admin: bool,
    pub requires_reboot: bool,
    pub summary: &'static str,
    pub detail: &'static str,
    pub revert_hint: &'static str,
    pub kind: TweakKind,
}

impl TweakSpec {
    pub fn is_applied(&self, statuses: &std::collections::BTreeMap<String, TweakStatus>) -> bool {
        statuses
            .get(self.id)
            .map(|s| s.state == TweakState::Applied)
            .unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TweakState {
    Applied,
    NotApplied,
    Partial,
    Unknown,
}

impl TweakState {
    pub fn label(self) -> &'static str {
        match self {
            TweakState::Applied => "Applied",
            TweakState::NotApplied => "Not applied",
            TweakState::Partial => "Partial",
            TweakState::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct TweakStatus {
    pub state: TweakState,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct SystemReport {
    pub cpu_name: String,
    pub cpu_mhz: u32,
    pub physical_cores: u32,
    pub logical_cores: u32,
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    pub gpus: Vec<GpuInfo>,
    pub board: String,
    pub bios: String,
    pub machine: String,
    pub os_name: String,
    pub os_version: String,
    pub os_build: String,
    pub hostname: String,
    pub uptime_secs: u64,
    pub displays: Vec<DisplayInfo>,
    pub disks: Vec<DiskInfo>,
    pub startup_items: Vec<StartupItem>,
    pub adapters: Vec<AdapterInfo>,
    pub detected_games: Vec<DetectedGame>,
    pub installed_app_count: usize,
    pub dns_servers: Vec<String>,
    pub power_plan: Option<PowerPlan>,
    pub admin: bool,
    pub errors: Vec<String>,
}

impl Default for SystemReport {
    fn default() -> Self {
        Self {
            cpu_name: "Scanning…".into(),
            cpu_mhz: 0,
            physical_cores: 0,
            logical_cores: 0,
            ram_total_mb: 0,
            ram_used_mb: 0,
            gpus: Vec::new(),
            board: String::new(),
            bios: String::new(),
            machine: String::new(),
            os_name: String::new(),
            os_version: String::new(),
            os_build: String::new(),
            hostname: String::new(),
            uptime_secs: 0,
            displays: Vec::new(),
            disks: Vec::new(),
            startup_items: Vec::new(),
            adapters: Vec::new(),
            detected_games: Vec::new(),
            installed_app_count: 0,
            dns_servers: Vec::new(),
            power_plan: None,
            admin: false,
            errors: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct GpuInfo {
    pub name: String,
    pub driver_version: String,
    pub vram_mb: u64,
    pub software: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayInfo {
    pub name: String,
    pub device: String,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub max_refresh: u32,
    pub primary: bool,
}

impl DisplayInfo {
    pub fn resolution(&self) -> String {
        format!("{} × {}", self.width, self.height)
    }

    pub fn is_high_refresh(&self) -> bool {
        self.refresh >= 120
    }

    pub fn refresh_gap(&self) -> bool {
        self.max_refresh > self.refresh + 1 && self.max_refresh >= 100
    }
}

#[derive(Clone, Debug, Default)]
pub struct DiskInfo {
    pub mount: String,
    pub kind: String,
    pub total_gb: f64,
    pub free_gb: f64,
}

#[derive(Clone, Debug, Default)]
pub struct StartupItem {
    pub name: String,
    pub command: String,
    pub scope: String,
}

#[derive(Clone, Debug, Default)]
pub struct AdapterInfo {
    pub name: String,
    pub driver: String,
    pub pnp_id: String,
    pub is_virtual: bool,
    pub power_management: Option<u32>,
    pub registry_key: String,
}

#[derive(Clone, Debug, Default)]
pub struct DetectedGame {
    pub preset_id: String,
    pub game: String,
    pub path: String,
}

#[derive(Clone, Debug, Default)]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
}

impl PowerPlan {
    pub fn is_high_performance(&self) -> bool {
        let g = self.guid.to_ascii_lowercase();
        g.contains("8c5e7fda") || g.contains("e9a42b02") || self.name.to_ascii_lowercase().contains("high performance") || self.name.to_ascii_lowercase().contains("ultimate")
    }
}

#[derive(Clone, Debug, Default)]
pub struct LiveStats {
    pub cpu_pct: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub uptime_secs: u64,
    pub updated_unix: u64,
}

#[derive(Clone, Debug)]
pub struct RunningGame {
    pub pid: u32,
    pub exe: String,
    pub preset_id: String,
    pub game: String,
}
