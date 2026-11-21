use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Rect, RichText, Rounding, Sense, Stroke, Vec2,
};

use crate::analysis::{Suggestion, SuggestionAction};
use crate::backup::{self, Originals, Snapshot};
use crate::engine::{self, ApplyReport};
use crate::logging::{self, Level, LogEntry};
use crate::model::*;
use crate::network::{self, ProbeSummary};
use crate::profiles::{self, Profile, ProfileStore};
use crate::system;
use crate::theme;
use crate::tweaks;
use crate::ui;
use crate::util;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    System,
    Network,
    Tweaks,
    Profiles,
    Vault,
    Activity,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Tab::Dashboard,
        Tab::System,
        Tab::Network,
        Tab::Tweaks,
        Tab::Profiles,
        Tab::Vault,
        Tab::Activity,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::System => "System",
            Tab::Network => "Network",
            Tab::Tweaks => "Tweaks",
            Tab::Profiles => "Profiles",
            Tab::Vault => "Backup vault",
            Tab::Activity => "Activity",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::System => "System scan",
            Tab::Network => "Network lab",
            Tab::Tweaks => "Tweak library",
            Tab::Profiles => "Profiles & game presets",
            Tab::Vault => "Backup vault",
            Tab::Activity => "Activity log",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Tab::Dashboard => "Your competitive readiness at a glance",
            Tab::System => "Hardware, displays, storage and startup load",
            Tab::Network => "Route latency, jitter and the Windows TCP stack",
            Tab::Tweaks => "Every change Clutchcore can make — and its backup",
            Tab::Profiles => "Popular-game presets and your own profiles",
            Tab::Vault => "Original values, captured before anything changed",
            Tab::Activity => "What happened, when, and whether it worked",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Tab::Dashboard => 0,
            Tab::System => 1,
            Tab::Network => 2,
            Tab::Tweaks => 3,
            Tab::Profiles => 4,
            Tab::Vault => 5,
            Tab::Activity => 6,
        }
    }

    pub fn from_key(key: &str) -> Tab {
        match key {
            "system" => Tab::System,
            "network" => Tab::Network,
            "tweaks" => Tab::Tweaks,
            "profiles" => Tab::Profiles,
            "vault" => Tab::Vault,
            "activity" => Tab::Activity,
            _ => Tab::Dashboard,
        }
    }
}

pub enum BgEvent {
    ScanDone(Box<SystemReport>, Box<tweaks::Detection>),
    Live(Box<LiveStats>, Vec<RunningGame>),
    Probe(Box<network::ProbeResult>),
    ProbeDone,
}

#[derive(Clone, Debug)]
pub struct ProfileDraft {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub selected: BTreeSet<String>,
    pub error: Option<String>,
}

impl ProfileDraft {
    pub fn new() -> Self {
        let mut selected = BTreeSet::new();
        for id in [
            "mouse_accel_off",
            "game_dvr_off",
            "fullscreen_optimizations_off",
            "game_mode_on",
            "power_plan_high",
            "visual_effects_performance",
            "transparency_off",
        ] {
            selected.insert(id.to_string());
        }
        Self {
            id: None,
            name: String::new(),
            description: String::new(),
            selected,
            error: None,
        }
    }

    pub fn from_profile(profile: &Profile) -> Self {
        Self {
            id: Some(profile.id.clone()),
            name: profile.name.clone(),
            description: profile.description.clone(),
            selected: profile.tweak_ids.iter().cloned().collect(),
            error: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ConfirmKind {
    ApplyProfile {
        profile_id: String,
        label: String,
        ids: Vec<String>,
    },
    ApplyTweak(String),
    RestoreAll,
    RestoreSnapshot(String),
    DeleteProfile(String),
    Elevate,
}

#[derive(Clone, Debug)]
pub struct ConfirmDialog {
    pub title: String,
    pub body: String,
    pub confirm_label: String,
    pub danger: bool,
    pub kind: ConfirmKind,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub level: Level,
    pub shown_at: Instant,
    pub until: Instant,
}

pub struct App {
    pub tab: Tab,
    pub report: SystemReport,
    pub scan_running: bool,
    pub statuses: BTreeMap<String, TweakStatus>,
    pub active_plan: Option<PowerPlan>,
    pub admin: bool,
    pub live: LiveStats,
    pub running_games: Vec<RunningGame>,
    pub boosted: BTreeSet<u32>,
    pub probe_results: Vec<network::ProbeResult>,
    pub probe_running: bool,
    pub probe_completed: usize,
    pub probe_expected: usize,
    pub probe_provider: Option<String>,
    pub custom_host: String,
    pub custom_result: Option<network::ProbeResult>,
    pub custom_running: bool,
    pub profiles_store: ProfileStore,
    pub selected_preset: Option<String>,
    pub editing: Option<ProfileDraft>,
    pub expanded_tweaks: BTreeSet<String>,
    pub tweak_filter: Option<Category>,
    pub tweak_search: String,
    pub snapshots: Vec<Snapshot>,
    pub originals: Originals,
    pub expanded_snapshot: Option<String>,
    pub confirm: Option<ConfirmDialog>,
    pub toast: Option<Toast>,
    pub last_report: Option<ApplyReport>,
    pub log: Vec<LogEntry>,
    pub tx: Sender<BgEvent>,
    pub rx: Receiver<BgEvent>,
    pub data_dir: PathBuf,
    pub last_probe_summary: Option<ProbeSummary>,
    pub view_t: f32,
    view_started: Option<f64>,
    shown_tab: Tab,
    confirm_started: Option<f64>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        logging::init();
        let _ = util::ensure_dirs();
        let (tx, rx) = mpsc::channel();
        let detection = tweaks::detect_all();
        let admin = system::is_admin();
        let probe_results = network::load_results();
        let last_probe_summary = network::summarize(&probe_results);
        let data_dir = util::data_dir();
        let mut app = Self {
            tab: Tab::Dashboard,
            report: SystemReport::default(),
            scan_running: true,
            statuses: detection.statuses,
            active_plan: detection.plan,
            admin,
            live: LiveStats::default(),
            running_games: Vec::new(),
            boosted: BTreeSet::new(),
            probe_results,
            probe_running: false,
            probe_completed: 0,
            probe_expected: 0,
            probe_provider: None,
            custom_host: String::new(),
            custom_result: None,
            custom_running: false,
            profiles_store: profiles::load_store(),
            selected_preset: None,
            editing: None,
            expanded_tweaks: BTreeSet::new(),
            tweak_filter: None,
            tweak_search: String::new(),
            snapshots: backup::list_snapshots(),
            originals: backup::load_originals(),
            expanded_snapshot: None,
            confirm: None,
            toast: None,
            last_report: None,
            log: logging::entries(),
            tx,
            rx,
            data_dir,
            last_probe_summary,
            view_t: 0.0,
            view_started: None,
            shown_tab: Tab::Dashboard,
            confirm_started: None,
        };
        app.start_scan();
        app.start_live_monitor();
        logging::info("Clutchcore started");
        app.log = logging::entries();
        app
    }

    pub(crate) fn start_scan(&mut self) {
        if self.scan_running && self.report.logical_cores > 0 {
            return;
        }
        self.scan_running = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let report = system::scan();
            let detection = tweaks::detect_all();
            let _ = tx.send(BgEvent::ScanDone(Box::new(report), Box::new(detection)));
        });
    }

    fn start_live_monitor(&mut self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let mut monitor = system::LiveMonitor::new();
            loop {
                std::thread::sleep(Duration::from_millis(1200));
                let stats = monitor.sample();
                let games = system::running_games();
                if tx.send(BgEvent::Live(Box::new(stats), games)).is_err() {
                    break;
                }
            }
        });
    }

    fn poll_events(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(BgEvent::ScanDone(report, detection)) => {
                    self.report = *report;
                    self.statuses = detection.statuses;
                    self.active_plan = detection.plan;
                    self.admin = self.report.admin;
                    self.scan_running = false;
                    self.snapshots = backup::list_snapshots();
                    self.originals = backup::load_originals();
                    self.log = logging::entries();
                    logging::info("System scan complete");
                }
                Ok(BgEvent::Live(stats, games)) => {
                    self.live = *stats;
                    self.running_games = games;
                    let pids: BTreeSet<u32> = self.running_games.iter().map(|g| g.pid).collect();
                    self.boosted.retain(|pid| pids.contains(pid));
                }
                Ok(BgEvent::Probe(result)) => {
                    self.absorb_probe(*result);
                }
                Ok(BgEvent::ProbeDone) => {
                    self.probe_running = false;
                    network::save_results(&self.probe_results);
                    self.log = logging::entries();
                    if let Some(summary) = network::summarize(&self.probe_results) {
                        self.toast(
                            Level::Success,
                            format!(
                                "Route test finished · best {} at {:.0} ms",
                                summary.best_label, summary.best_ms
                            ),
                        );
                    }
                }
                Err(mpsc::TryRecvError::Empty) | Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }

    fn absorb_probe(&mut self, result: network::ProbeResult) {
        if result.provider == "Custom" {
            self.custom_running = false;
            if let Some(error) = &result.error {
                self.toast(Level::Error, format!("Probe failed: {error}"));
            } else {
                self.toast(
                    Level::Success,
                    format!("{} answered in {:.0} ms", result.host, result.avg_ms),
                );
            }
            self.custom_result = Some(result);
            self.log = logging::entries();
            return;
        }

        self.probe_completed = self.probe_completed.saturating_add(1);
        self.probe_results
            .retain(|r| !(r.host == result.host && r.port == result.port));
        self.probe_results.push(result);
        self.probe_results.sort_by(|a, b| {
            a.avg_ms
                .partial_cmp(&b.avg_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    fn refresh_statuses(&mut self) {
        let detection = tweaks::detect_all();
        self.statuses = detection.statuses;
        self.active_plan = detection.plan;
    }

    pub(crate) fn finish_report(&mut self, report: ApplyReport) {
        let failed = report.failed();
        let needs_reboot = report.needs_reboot;
        let summary = report.summary();
        self.last_report = Some(report);
        self.refresh_statuses();
        self.snapshots = backup::list_snapshots();
        self.originals = backup::load_originals();
        self.log = logging::entries();
        if failed > 0 {
            let mut message = summary;
            if !self.admin {
                message.push_str(" Try Run as administrator.");
            }
            self.toast(Level::Error, message);
        } else {
            let mut message = summary;
            if needs_reboot {
                message.push_str(" · reboot required");
            }
            self.toast(Level::Success, message);
        }
    }

    pub(crate) fn apply_ids(&mut self, ids: Vec<String>, label: &str, profile: Option<&str>) {
        if ids.is_empty() {
            return;
        }
        let report = engine::apply(&ids, label, profile);
        self.finish_report(report);
    }

    pub(crate) fn toggle_tweak(&mut self, id: &str) {
        let Some(spec) = tweaks::find(id) else {
            return;
        };
        let applied = self
            .statuses
            .get(id)
            .map(|s| s.state == TweakState::Applied)
            .unwrap_or(false);
        if applied {
            let report = engine::revert(
                &[id.to_string()],
                &format!("Revert {}", spec.name),
            );
            self.finish_report(report);
        } else if spec.risk != Risk::Low {
            self.confirm = Some(ConfirmDialog {
                title: format!("Apply {}?", spec.name),
                body: format!(
                    "{}\n\nRisk: {}. {}\n\nA snapshot of the current values is written before anything changes.",
                    spec.detail,
                    spec.risk.label(),
                    spec.risk.describes()
                ),
                confirm_label: "Apply change".into(),
                danger: false,
                kind: ConfirmKind::ApplyTweak(spec.id.to_string()),
            });
        } else {
            self.apply_ids(
                vec![id.to_string()],
                &format!("Apply {}", spec.name),
                None,
            );
        }
    }

    pub(crate) fn ask_apply_profile(&mut self, profile: &Profile) {
        let ids = profiles::profile_tweak_ids(profile);
        if ids.is_empty() {
            self.toast(Level::Warn, "That profile has no valid tweaks.");
            return;
        }
        let risky = tweaks::risky_in(&ids);
        if risky.is_empty() {
            self.apply_profile_now(profile);
            return;
        }
        let names = risky
            .iter()
            .map(|s| format!("• {}", s.name))
            .collect::<Vec<_>>()
            .join("\n");
        self.confirm = Some(ConfirmDialog {
            title: format!("Apply {}?", profile.name),
            body: format!(
                "This profile includes {} change(s) that affect scheduling, the GPU driver or the network.\n\n{}\n\nEvery original value is written to the vault first, so this can be undone.",
                risky.len(),
                names
            ),
            confirm_label: "Apply profile".into(),
            danger: false,
            kind: ConfirmKind::ApplyProfile {
                profile_id: profile.id.clone(),
                label: format!("{} profile", profile.name),
                ids,
            },
        });
    }

    pub(crate) fn apply_profile_now(&mut self, profile: &Profile) {
        let ids = profiles::profile_tweak_ids(profile);
        self.apply_ids(
            ids,
            &format!("{} profile", profile.name),
            Some(&profile.id),
        );
        let _ = profiles::set_active(&mut self.profiles_store, Some(&profile.id));
    }

    pub(crate) fn ask_restore_all(&mut self) {
        let count = self.originals.entries.len();
        if count == 0 {
            self.toast(
                Level::Warn,
                "The vault is empty — nothing has been changed yet.",
            );
            return;
        }
        self.confirm = Some(ConfirmDialog {
            title: "Restore every original setting?".into(),
            body: format!(
                "Clutchcore will write the first captured values for {count} tweak(s) back to the registry and switch to your original power plan. A pre-restore snapshot is saved, so this action is reversible too."
            ),
            confirm_label: "Restore originals".into(),
            danger: true,
            kind: ConfirmKind::RestoreAll,
        });
    }

    pub(crate) fn ask_restore_snapshot(&mut self, snapshot: &Snapshot) {
        self.confirm = Some(ConfirmDialog {
            title: "Restore this snapshot?".into(),
            body: format!(
                "This writes the values from '{}' ({}) back to the registry. A snapshot of the current state is saved first.",
                snapshot.label,
                util::fmt_datetime(snapshot.created_unix)
            ),
            confirm_label: "Restore snapshot".into(),
            danger: true,
            kind: ConfirmKind::RestoreSnapshot(snapshot.id.clone()),
        });
    }

    pub(crate) fn ask_delete_profile(&mut self, profile: &Profile) {
        self.confirm = Some(ConfirmDialog {
            title: format!("Delete '{}'?", profile.name),
            body: "This removes the profile definition only. System settings and backups are untouched.".into(),
            confirm_label: "Delete profile".into(),
            danger: true,
            kind: ConfirmKind::DeleteProfile(profile.id.clone()),
        });
    }

    pub(crate) fn ask_elevate(&mut self) {
        self.confirm = Some(ConfirmDialog {
            title: "Restart as administrator?".into(),
            body: "Clutchcore will close and relaunch with administrator rights. Windows will show a UAC prompt. Your profiles, backups and settings stay exactly where they are.".into(),
            confirm_label: "Restart elevated".into(),
            danger: false,
            kind: ConfirmKind::Elevate,
        });
    }

    fn run_confirm(&mut self, kind: ConfirmKind) {
        match kind {
            ConfirmKind::ApplyProfile {
                profile_id,
                label,
                ids,
            } => {
                self.apply_ids(ids, &label, Some(&profile_id));
                let _ = profiles::set_active(&mut self.profiles_store, Some(&profile_id));
            }
            ConfirmKind::ApplyTweak(id) => {
                if let Some(spec) = tweaks::find(&id) {
                    self.apply_ids(vec![id], &format!("Apply {}", spec.name), None);
                }
            }
            ConfirmKind::RestoreAll => {
                let report = engine::restore_all_originals();
                self.finish_report(report);
                let _ = profiles::set_active(&mut self.profiles_store, None);
            }
            ConfirmKind::RestoreSnapshot(id) => {
                if let Some(snapshot) = self.snapshots.iter().find(|s| s.id == id).cloned() {
                    let report = engine::restore_snapshot(&snapshot);
                    self.finish_report(report);
                }
            }
            ConfirmKind::DeleteProfile(id) => {
                match profiles::delete_custom(&mut self.profiles_store, &id) {
                    Ok(()) => {
                        self.log = logging::entries();
                        if self.selected_preset.as_deref() == Some(id.as_str()) {
                            self.selected_preset = None;
                        }
                        self.toast(Level::Success, "Profile deleted.");
                    }
                    Err(error) => self.toast(Level::Error, error),
                }
            }
            ConfirmKind::Elevate => self.relaunch_elevated(),
        }
    }

    fn relaunch_elevated(&mut self) {
        let Ok(exe) = std::env::current_exe() else {
            self.toast(Level::Error, "Could not locate the Clutchcore executable.");
            return;
        };
        let quoted = exe.display().to_string().replace('\'', "''");
        let script = format!("Start-Process -FilePath '{quoted}' -Verb RunAs");
        let spawned = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                script.as_str(),
            ])
            .spawn();
        match spawned {
            Ok(_) => std::process::exit(0),
            Err(error) => self.toast(
                Level::Error,
                format!("Could not launch the elevated copy: {error}"),
            ),
        }
    }

    pub(crate) fn start_probe_all(&mut self, provider: Option<String>) {
        if self.probe_running || self.custom_running {
            return;
        }
        let mut endpoints: Vec<&'static network::ProbeEndpoint> = network::endpoints()
            .iter()
            .filter(|endpoint| {
                provider
                    .as_deref()
                    .map(|p| endpoint.provider == p)
                    .unwrap_or(true)
            })
            .collect();
        if endpoints.is_empty() {
            self.toast(Level::Warn, "No routes match that filter.");
            return;
        }
        self.probe_running = true;
        self.probe_completed = 0;
        self.probe_expected = endpoints.len();
        self.probe_provider = provider;
        self.probe_results.retain(|r| r.provider == "Custom");
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            std::thread::scope(|scope| {
                for endpoint in endpoints.drain(..) {
                    let tx = tx.clone();
                    scope.spawn(move || {
                        let result = network::probe(
                            endpoint.label,
                            endpoint.provider,
                            endpoint.region,
                            endpoint.host,
                            endpoint.port,
                            8,
                            1100,
                        );
                        let _ = tx.send(BgEvent::Probe(Box::new(result)));
                    });
                }
            });
            let _ = tx.send(BgEvent::ProbeDone);
        });
    }

    pub(crate) fn start_custom_probe(&mut self) {
        let input = self.custom_host.trim().to_string();
        if input.is_empty() {
            self.toast(Level::Warn, "Type a hostname or IP first.");
            return;
        }
        if self.custom_running || self.probe_running {
            return;
        }
        self.custom_running = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = network::custom_probe(&input, 8);
            let _ = tx.send(BgEvent::Probe(Box::new(result)));
        });
    }

    pub(crate) fn toggle_boost(&mut self, pid: u32, game: &str) {
        let is_boosted = self.boosted.contains(&pid);
        match system::set_process_priority(pid, !is_boosted) {
            Ok(()) => {
                if is_boosted {
                    self.boosted.remove(&pid);
                    self.toast(Level::Success, format!("{game} priority restored to Normal."));
                } else {
                    self.boosted.insert(pid);
                    self.toast(Level::Success, format!("{game} priority set to High."));
                }
            }
            Err(error) => self.toast(Level::Error, error),
        }
    }

    pub(crate) fn toast(&mut self, level: Level, message: impl Into<String>) {
        let now = Instant::now();
        self.toast = Some(Toast {
            text: message.into(),
            level,
            shown_at: now,
            until: now + Duration::from_secs(5),
        });
    }

    pub(crate) fn handle_suggestion(&mut self, suggestion: Suggestion) {
        match suggestion.action {
            SuggestionAction::ApplyTweak(id) => self.toggle_tweak(id),
            SuggestionAction::OpenUri(uri) => util::open_uri(uri),
            SuggestionAction::Navigate(key) => self.tab = Tab::from_key(key),
            SuggestionAction::Elevate => self.ask_elevate(),
            SuggestionAction::Info => {}
        }
    }

    pub(crate) fn export_originals(&mut self) {
        match backup::export_originals_txt(&self.originals) {
            Ok(path) => self.toast(
                Level::Success,
                format!("Readable copy written to {}", path.display()),
            ),
            Err(error) => self.toast(Level::Error, error),
        }
    }

    pub(crate) fn export_snapshot(&mut self, id: &str) {
        if let Some(snapshot) = self.snapshots.iter().find(|s| s.id == id).cloned() {
            match backup::export_snapshot_txt(&snapshot) {
                Ok(path) => self.toast(
                    Level::Success,
                    format!("Snapshot exported to {}", path.display()),
                ),
                Err(error) => self.toast(Level::Error, error),
            }
        }
    }

    pub(crate) fn export_report(&mut self) {
        if self.scan_running {
            self.toast(Level::Warn, "The scan is still running.");
            return;
        }
        match system::export_report(&self.report) {
            Ok(path) => self.toast(
                Level::Success,
                format!("System report written to {}", path.display()),
            ),
            Err(error) => self.toast(Level::Error, error),
        }
    }

    fn nav_list(&self, ui: &mut egui::Ui) -> Option<Tab> {
        let width = ui.available_width();
        let row = 35.0;
        let top = ui.cursor().top();
        let spacing = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut clicked = None;
        let mut rows = Vec::with_capacity(Tab::ALL.len());

        for tab in Tab::ALL {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, row), Sense::click());
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                clicked = Some(tab);
            }
            rows.push((tab, rect, response));
        }

        let marker_index = ui.ctx().animate_value_with_time(
            egui::Id::new("clutchcore-nav-marker"),
            self.tab.index() as f32,
            0.22,
        );
        let marker = Rect::from_min_size(
            egui::pos2(rows[0].1.left(), top + marker_index * row),
            Vec2::new(width, row),
        );
        let painter = ui.painter();
        painter.rect_filled(
            marker.shrink2(Vec2::new(0.0, 2.0)),
            Rounding::same(8.0),
            theme::ACCENT_TINT,
        );
        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(marker.left(), marker.top() + 8.0),
                Vec2::new(3.0, row - 16.0),
            ),
            Rounding::same(1.5),
            theme::ACCENT,
        );

        for (tab, rect, response) in &rows {
            if *tab != self.tab && response.hovered() {
                painter.rect_filled(
                    rect.shrink2(Vec2::new(0.0, 2.0)),
                    Rounding::same(8.0),
                    theme::CHIP,
                );
            }
        }

        for (tab, rect, response) in &rows {
            let selected = *tab == self.tab;
            let hover = ui
                .ctx()
                .animate_bool(response.id.with("nav-hover"), response.hovered());
            let color = if selected {
                theme::ACCENT
            } else {
                theme::lerp_color(theme::MUTED, theme::TEXT, hover)
            };
            nav_icon(
                painter,
                *tab,
                egui::pos2(rect.left() + 17.0, rect.center().y),
                color,
            );
            painter.text(
                egui::pos2(rect.left() + 37.0, rect.center().y),
                Align2::LEFT_CENTER,
                tab.label(),
                FontId::proportional(12.5),
                color,
            );
        }

        ui.spacing_mut().item_spacing.y = spacing;
        clicked
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        let mut rescan = false;
        let mut elevate = false;
        let ping = self.last_probe_summary.as_ref().map(|s| s.best_ms);
        let cpu = self.live.cpu_pct;
        let admin = self.admin;
        let tab = self.tab;
        egui::TopBottomPanel::top("topbar")
            .exact_height(66.0)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(24.0, 12.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(tab.title())
                                .size(19.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                        ui.label(
                            RichText::new(tab.subtitle())
                                .size(10.5)
                                .color(theme::MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !admin && ui::ghost_button(ui, "Run as administrator").clicked() {
                            elevate = true;
                        }
                        if ui::ghost_button(ui, "Re-scan").clicked() {
                            rescan = true;
                        }
                        ui::badge(
                            ui,
                            format!("CPU {:.0}%", cpu),
                            if cpu > 85.0 { theme::RED } else { theme::MUTED },
                        );
                        match ping {
                            Some(ms) => ui::badge(
                                ui,
                                format!("PING {:.0} ms", ms),
                                ui::ping_color(ms),
                            ),
                            None => ui::badge(ui, "PING —", theme::FAINT),
                        }
                        ui::admin_badge(ui, admin);
                    });
                });
                accent_rule(ui);
            });
        if rescan {
            self.start_scan();
        }
        if elevate {
            self.ask_elevate();
        }
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        let mut new_tab = self.tab;
        let mut open_folder = false;
        let mut restore = false;
        let mut export = false;
        let active = profiles::active_name(&self.profiles_store);
        let capture_count = self.originals.entries.len();
        let admin = self.admin;
        let data_dir = self.data_dir.clone();
        egui::SidePanel::left("nav")
            .exact_width(220.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(theme::SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(14.0, 16.0)),
            )
            .show(ctx, |ui| {
                let texture = logo_texture(ui.ctx());
                let (plate, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::hover());
                let center = egui::pos2(plate.left() + 31.0, plate.center().y);
                let painter = ui.painter();
                painter.rect_filled(plate, Rounding::same(12.0), theme::PANEL);
                painter.circle_filled(
                    center,
                    24.0 + (1.0 - self.view_t) * 12.0,
                    theme::with_alpha(theme::ACCENT, (8.0 + 16.0 * self.view_t) as u8),
                );
                painter.image(
                    texture.id(),
                    Rect::from_center_size(center, Vec2::splat(42.0)),
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                painter.text(
                    egui::pos2(plate.left() + 62.0, plate.center().y - 8.0),
                    Align2::LEFT_CENTER,
                    "CLUTCHCORE",
                    FontId::proportional(14.0),
                    theme::TEXT,
                );
                painter.text(
                    egui::pos2(plate.left() + 62.0, plate.center().y + 9.0),
                    Align2::LEFT_CENTER,
                    "COMPETITIVE TUNER",
                    FontId::proportional(7.5),
                    theme::ACCENT,
                );

                ui.add_space(20.0);
                ui.label(
                    RichText::new("WORKSPACE")
                        .size(8.5)
                        .strong()
                        .color(theme::FAINT),
                );
                ui.add_space(4.0);
                if let Some(tab) = self.nav_list(ui) {
                    new_tab = tab;
                }

                ui.add_space(16.0);
                ui.label(
                    RichText::new("ACTIVE PROFILE")
                        .size(8.5)
                        .strong()
                        .color(theme::FAINT),
                );
                ui.add_space(2.0);
                match &active {
                    Some(name) => {
                        ui.label(
                            RichText::new(util::truncate(name, 22))
                                .size(12.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                    }
                    None => {
                        ui.label(
                            RichText::new("No profile applied")
                                .size(11.0)
                                .color(theme::MUTED),
                        );
                    }
                }
                if ui::small_ghost(ui, "Browse profiles").clicked() {
                    new_tab = Tab::Profiles;
                }

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.label(
                        RichText::new(format!("v{} · {}", env!("CARGO_PKG_VERSION"), if admin {
                            "Elevated"
                        } else {
                            "Standard user"
                        }))
                        .size(9.0)
                        .color(theme::FAINT),
                    );
                    ui.add_space(6.0);
                    if ui::ghost_button(ui, "Restore originals").clicked() {
                        restore = true;
                    }
                    if ui::small_ghost(ui, "Export original settings").clicked() {
                        export = true;
                    }
                    ui.add_space(6.0);
                    if ui::ghost_button(ui, "Open data folder").clicked() {
                        open_folder = true;
                    }
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("{capture_count} original value(s) captured"))
                            .size(9.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(util::truncate(&data_dir.display().to_string(), 30))
                            .size(8.5)
                            .color(theme::FAINT),
                    );
                    ui.add_space(8.0);
                    ui::separator(ui);
                });
            });

        if new_tab != self.tab {
            self.tab = new_tab;
            self.selected_preset = None;
        }
        if open_folder {
            util::open_path(&self.data_dir);
        }
        if restore {
            self.ask_restore_all();
        }
        if export {
            self.export_originals();
        }
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some(toast) = self.toast.clone() else {
            return;
        };
        let now = Instant::now();
        if now >= toast.until {
            self.toast = None;
            return;
        }
        let elapsed = now.saturating_duration_since(toast.shown_at).as_secs_f32();
        let remaining = toast.until.saturating_duration_since(now).as_secs_f32();
        let appear = theme::ease_out((elapsed / 0.2).clamp(0.0, 1.0));
        let fade = (remaining / 0.45).clamp(0.0, 1.0);
        let alpha = appear.min(fade);
        let offset = (1.0 - appear) * 26.0;
        let color = match toast.level {
            Level::Success => theme::GREEN,
            Level::Warn => theme::YELLOW,
            Level::Error => theme::RED,
            Level::Info => theme::BLUE,
        };
        egui::Area::new(egui::Id::new("clutchcore-toast"))
            .anchor(
                Align2::RIGHT_BOTTOM,
                Vec2::new(-20.0 + offset, -18.0),
            )
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(theme::with_alpha(theme::PANEL, (250.0 * alpha) as u8))
                    .stroke(Stroke::new(
                        1.0,
                        theme::with_alpha(color, (210.0 * alpha) as u8),
                    ))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(egui::Margin::symmetric(15.0, 11.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui::dot(ui, theme::with_alpha(color, (255.0 * alpha) as u8), 10.0);
                            ui.label(
                                RichText::new(util::truncate(&toast.text, 140))
                                    .size(11.5)
                                    .color(theme::with_alpha(theme::TEXT, (255.0 * alpha) as u8)),
                            );
                        });
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn confirm_ui(&mut self, ctx: &egui::Context, t: f32) {
        let Some(dialog) = self.confirm.clone() else {
            return;
        };
        let mut confirmed = false;
        let mut cancelled = false;

        egui::Area::new(egui::Id::new("clutchcore-confirm-backdrop"))
            .fixed_pos(egui::Pos2::ZERO)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                let _ = ui.allocate_rect(screen, Sense::click());
                ui.painter().rect_filled(
                    screen,
                    0.0,
                    Color32::from_black_alpha((150.0 * t) as u8),
                );
            });

        egui::Area::new(egui::Id::new("clutchcore-confirm"))
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, (1.0 - t) * 18.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(theme::lerp_color(theme::BG, theme::PANEL, t))
                    .stroke(Stroke::new(
                        1.0,
                        theme::with_alpha(theme::BORDER, (255.0 * t) as u8),
                    ))
                    .rounding(Rounding::same(14.0))
                    .inner_margin(egui::Margin::same(20.0))
                    .show(ui, |ui| {
                        ui.set_width(400.0);
                        ui.label(
                            RichText::new("CONFIRM")
                                .size(8.5)
                                .strong()
                                .color(theme::ACCENT),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(dialog.title.as_str())
                                .size(17.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(dialog.body.as_str())
                                .size(11.5)
                                .color(theme::MUTED),
                        );
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            if dialog.danger {
                                if ui::danger_button(ui, dialog.confirm_label.as_str()).clicked() {
                                    confirmed = true;
                                }
                            } else if ui::accent_button(ui, dialog.confirm_label.as_str()).clicked() {
                                confirmed = true;
                            }
                            if ui::ghost_button(ui, "Cancel").clicked() {
                                cancelled = true;
                            }
                        });
                    });
            });

        if confirmed {
            if let Some(dialog) = self.confirm.take() {
                self.run_confirm(dialog.kind);
            }
        } else if cancelled {
            self.confirm = None;
        }
    }
}

fn nav_icon(painter: &egui::Painter, tab: Tab, center: egui::Pos2, color: Color32) {
    let stroke = Stroke::new(1.4, color);
    match tab {
        Tab::Dashboard => {
            for (dx, dy) in [(-3.5f32, -3.5f32), (2.0, -3.5), (-3.5, 2.0), (2.0, 2.0)] {
                painter.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(center.x + dx, center.y + dy),
                        Vec2::splat(4.0),
                    ),
                    Rounding::same(1.2),
                    color,
                );
            }
        }
        Tab::System => {
            painter.rect_stroke(
                Rect::from_center_size(center, Vec2::splat(9.5)),
                Rounding::same(1.5),
                stroke,
            );
            painter.rect_filled(
                Rect::from_center_size(center, Vec2::splat(4.0)),
                Rounding::same(0.8),
                color,
            );
        }
        Tab::Network => {
            for (index, height) in [4.0f32, 7.0, 10.0].into_iter().enumerate() {
                let x = center.x - 4.5 + index as f32 * 4.0;
                painter.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x, center.y + 5.0 - height),
                        Vec2::new(2.4, height),
                    ),
                    Rounding::same(1.2),
                    color,
                );
            }
        }
        Tab::Tweaks => {
            for (index, y) in [-4.0f32, 0.0, 4.0].into_iter().enumerate() {
                painter.line_segment(
                    [
                        egui::pos2(center.x - 5.0, center.y + y),
                        egui::pos2(center.x + 5.0, center.y + y),
                    ],
                    Stroke::new(1.2, color),
                );
                let knob_x = center.x + if index == 1 { 2.5 } else { -2.0 };
                painter.circle_filled(egui::pos2(knob_x, center.y + y), 1.7, color);
            }
        }
        Tab::Profiles => {
            painter.rect_stroke(
                Rect::from_min_size(
                    egui::pos2(center.x - 5.0, center.y - 4.0),
                    Vec2::new(10.0, 7.0),
                ),
                Rounding::same(1.5),
                stroke,
            );
            painter.rect_stroke(
                Rect::from_min_size(
                    egui::pos2(center.x - 3.0, center.y - 1.0),
                    Vec2::new(10.0, 7.0),
                ),
                Rounding::same(1.5),
                Stroke::new(1.1, color),
            );
        }
        Tab::Vault => {
            painter.rect_stroke(
                Rect::from_min_size(
                    egui::pos2(center.x - 4.5, center.y - 1.0),
                    Vec2::new(9.0, 7.0),
                ),
                Rounding::same(1.5),
                stroke,
            );
            painter.circle_stroke(
                egui::pos2(center.x, center.y - 3.0),
                2.8,
                Stroke::new(1.2, color),
            );
        }
        Tab::Activity => {
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(center.x - 5.5, center.y + 1.0),
                    egui::pos2(center.x - 2.0, center.y + 1.0),
                    egui::pos2(center.x, center.y - 4.0),
                    egui::pos2(center.x + 2.5, center.y + 4.0),
                    egui::pos2(center.x + 5.0, center.y - 1.0),
                ],
                Stroke::new(1.3, color),
            ));
        }
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        self.last_probe_summary = network::summarize(&self.probe_results);
        if self
            .toast
            .as_ref()
            .map(|toast| Instant::now() >= toast.until)
            .unwrap_or(false)
        {
            self.toast = None;
        }

        let now = ctx.input(|input| input.time);
        if self.tab != self.shown_tab {
            self.shown_tab = self.tab;
            self.view_started = Some(now);
        }
        let view_start = *self.view_started.get_or_insert(now);
        self.view_t =
            theme::ease_out(((now - view_start) as f32 / 0.34).clamp(0.0, 1.0));

        if self.confirm.is_some() {
            if self.confirm_started.is_none() {
                self.confirm_started = Some(now);
            }
        } else {
            self.confirm_started = None;
        }
        let confirm_t = self
            .confirm_started
            .map(|start| theme::ease_out(((now - start) as f32 / 0.2).clamp(0.0, 1.0)))
            .unwrap_or(1.0);

        self.top_bar(ctx);
        self.sidebar(ctx);

        let tab = self.tab;
        let view_t = self.view_t;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(24.0, 18.0)),
            )
            .show(ctx, |ui| {
                ui.add_space((1.0 - view_t) * 8.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.push_id(tab.label(), |ui| match tab {
                            Tab::Dashboard => self.view_dashboard(ui),
                            Tab::System => self.view_system(ui),
                            Tab::Network => self.view_network(ui),
                            Tab::Tweaks => self.view_tweaks(ui),
                            Tab::Profiles => self.view_profiles(ui),
                            Tab::Vault => self.view_vault(ui),
                            Tab::Activity => self.view_activity(ui),
                        });
                    });
                if view_t < 1.0 {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        theme::with_alpha(theme::BG, ((1.0 - view_t) * 245.0) as u8),
                    );
                }
            });

        self.toast_ui(ctx);
        self.confirm_ui(ctx, confirm_t);

        if view_t < 1.0 || (self.confirm.is_some() && confirm_t < 1.0) {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.scan_running || self.probe_running || self.custom_running {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 840.0])
            .with_min_inner_size([1060.0, 700.0])
            .with_icon(std::sync::Arc::new(make_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "Clutchcore",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

const LOGO_RGBA: &[u8] = include_bytes!("../assets/logo256.rgba");
const LOGO_SIZE: usize = 256;

fn make_icon() -> egui::IconData {
    egui::IconData {
        rgba: LOGO_RGBA.to_vec(),
        width: LOGO_SIZE as u32,
        height: LOGO_SIZE as u32,
    }
}

fn logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("clutchcore-logo-texture");
    if let Some(handle) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return handle;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([LOGO_SIZE, LOGO_SIZE], LOGO_RGBA);
    let handle = ctx.load_texture("clutchcore-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, handle.clone()));
    handle
}
