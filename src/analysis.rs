use std::collections::BTreeMap;

use crate::model::*;
use crate::network::ProbeSummary;
use crate::tweaks;
use crate::util;

#[derive(Clone, Copy, Debug)]
pub struct Readiness {
    pub overall: u32,
    pub input: u32,
    pub latency: u32,
    pub fps: u32,
    pub network: u32,
}

fn weights(id: &str) -> [f32; 4] {
    // input, latency, fps, network
    match id {
        "mouse_accel_off" => [45.0, 5.0, 0.0, 0.0],
        "fullscreen_optimizations_off" => [20.0, 10.0, 0.0, 0.0],
        "win32_priority_separation" => [15.0, 10.0, 0.0, 0.0],
        "game_dvr_off" => [10.0, 15.0, 6.0, 0.0],
        "power_plan_high" => [0.0, 10.0, 10.0, 0.0],
        "mmcss_games" => [0.0, 10.0, 5.0, 0.0],
        "hags_on" => [0.0, 5.0, 8.0, 0.0],
        "game_mode_on" => [0.0, 5.0, 5.0, 0.0],
        "power_throttling_off" => [0.0, 0.0, 8.0, 0.0],
        "visual_effects_performance" => [0.0, 0.0, 12.0, 0.0],
        "transparency_off" => [0.0, 0.0, 5.0, 0.0],
        "background_apps_off" => [0.0, 0.0, 8.0, 0.0],
        "xbox_bar_off" => [0.0, 0.0, 5.0, 0.0],
        "tcp_latency" => [0.0, 0.0, 0.0, 35.0],
        "network_throttling_off" => [0.0, 0.0, 0.0, 25.0],
        "nic_power_off" => [0.0, 0.0, 0.0, 15.0],
        _ => [0.0; 4],
    }
}

fn clamp_score(value: f32) -> u32 {
    value.round().clamp(0.0, 100.0) as u32
}

pub fn readiness(
    statuses: &BTreeMap<String, TweakStatus>,
    report: &SystemReport,
    summary: Option<&ProbeSummary>,
) -> Readiness {
    let mut input = 100.0f32;
    let mut latency = 100.0f32;
    let mut fps = 100.0f32;
    let mut network = 100.0f32;

    for spec in tweaks::specs() {
        let applied = statuses
            .get(spec.id)
            .map(|s| s.state == TweakState::Applied)
            .unwrap_or(false);
        if applied {
            continue;
        }
        let [wi, wl, wf, wn] = weights(spec.id);
        let scale = if spec.requires_admin && !report.admin {
            0.5
        } else {
            1.0
        };
        input -= wi * scale;
        latency -= wl * scale;
        fps -= wf * scale;
        network -= wn * scale;
    }

    if let Some(primary) = report.displays.iter().find(|d| d.primary).or_else(|| report.displays.first()) {
        if primary.max_refresh > primary.refresh + 1 && primary.max_refresh >= 100 {
            fps -= 15.0;
        }
        if primary.refresh <= 60 {
            fps -= 5.0;
        }
    }

    if report.ram_total_mb > 0 && report.ram_total_mb < 8192 {
        fps -= 15.0;
    } else if report.ram_total_mb > 0 && report.ram_total_mb < 16384 {
        fps -= 8.0;
    }
    if report.logical_cores > 0 && report.logical_cores <= 4 {
        fps -= 5.0;
    }
    if let Some(disk) = report.disks.iter().find(|d| d.mount.starts_with('C')) {
        if disk.total_gb > 0.0 && disk.free_gb / disk.total_gb < 0.08 {
            fps -= 5.0;
        }
    }
    if report.uptime_secs > 7 * 86400 {
        latency -= 5.0;
    }
    if report.startup_items.len() > 15 {
        latency -= 5.0;
    }

    if let Some(summary) = summary {
        if summary.best_ms > 120.0 {
            network -= 35.0;
        } else if summary.best_ms > 80.0 {
            network -= 25.0;
        } else if summary.best_ms > 50.0 {
            network -= 15.0;
        } else if summary.best_ms > 35.0 {
            network -= 8.0;
        }
        if summary.avg_jitter > 20.0 {
            network -= 10.0;
        } else if summary.avg_jitter > 10.0 {
            network -= 5.0;
        }
        if summary.worst_loss > 2.0 {
            network -= 15.0;
        }
    } else {
        network -= 10.0;
    }

    let input = clamp_score(input);
    let latency = clamp_score(latency);
    let fps = clamp_score(fps);
    let network = clamp_score(network);
    let overall = clamp_score(
        input as f32 * 0.25 + latency as f32 * 0.30 + fps as f32 * 0.20 + network as f32 * 0.25,
    );
    Readiness {
        overall,
        input,
        latency,
        fps,
        network,
    }
}

pub fn grade(score: u32) -> &'static str {
    match score {
        90..=100 => "Locked in",
        75..=89 => "Competitive",
        60..=74 => "Warming up",
        40..=59 => "Needs work",
        _ => "Stock",
    }
}

pub fn grade_note(score: u32) -> &'static str {
    match score {
        90..=100 => "Nothing obvious is holding this machine back. Keep it stable.",
        75..=89 => "A few latency items are still on the table.",
        60..=74 => "Several competitive defaults are still fighting you.",
        40..=59 => "Windows is running mostly stock. Start with the priority fixes.",
        _ => "Plenty of free headroom. Apply the safe set first.",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Critical,
    Recommended,
    Optional,
    Note,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Critical => "Priority",
            Severity::Recommended => "Recommended",
            Severity::Optional => "Optional",
            Severity::Note => "Note",
        }
    }
}

#[derive(Clone, Debug)]
pub enum SuggestionAction {
    ApplyTweak(&'static str),
    OpenUri(&'static str),
    Navigate(&'static str),
    Elevate,
    Info,
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub severity: Severity,
    pub action: SuggestionAction,
}

pub fn suggestions(
    statuses: &BTreeMap<String, TweakStatus>,
    report: &SystemReport,
    summary: Option<&ProbeSummary>,
    admin: bool,
) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = Vec::new();

    if let Some(primary) = report.displays.iter().find(|d| d.primary).or_else(|| report.displays.first()) {
        if primary.max_refresh > primary.refresh + 1 && primary.max_refresh >= 100 {
            out.push(Suggestion {
                id: "display-refresh".into(),
                title: format!(
                    "Run your display at {} Hz, not {} Hz",
                    primary.max_refresh, primary.refresh
                ),
                detail: format!(
                    "{} supports {} Hz at {}×{}, but Windows is sending {} Hz. No amount of tuning beats a higher refresh. Set it in Settings → System → Display → Advanced display.",
                    primary.name, primary.max_refresh, primary.width, primary.height, primary.refresh
                ),
                severity: Severity::Critical,
                action: SuggestionAction::OpenUri("ms-settings:display-advanced"),
            });
        }
    }

    let locked_admin: Vec<&TweakSpec> = tweaks::specs()
        .iter()
        .filter(|s| s.requires_admin)
        .filter(|s| {
            statuses
                .get(s.id)
                .map(|st| st.state != TweakState::Applied)
                .unwrap_or(true)
        })
        .collect();
    if !admin && !locked_admin.is_empty() {
        out.push(Suggestion {
            id: "elevate".into(),
            title: format!(
                "Run as administrator to unlock {} more tweaks",
                locked_admin.len()
            ),
            detail: "Network stack, GPU scheduling and scheduler priority changes need an elevated process. Clutchcore will relaunch itself with Windows' approval prompt.".into(),
            severity: Severity::Recommended,
            action: SuggestionAction::Elevate,
        });
    }

    if let Some(summary) = summary {
        if summary.best_ms > 80.0 {
            out.push(Suggestion {
                id: "ping".into(),
                title: format!(
                    "Your best route measures {:.0} ms ({})",
                    summary.best_ms, summary.best_label
                ),
                detail: "That is high for a nearby game region. Try a wired connection, test another route, and check whether anyone else on the network is saturating the uplink.".into(),
                severity: Severity::Critical,
                action: SuggestionAction::Navigate("network"),
            });
        } else if summary.avg_jitter > 12.0 {
            out.push(Suggestion {
                id: "jitter".into(),
                title: format!("Jitter is averaging {:.1} ms", summary.avg_jitter),
                detail: "Steady ping beats low ping. Jitter usually comes from Wi-Fi, power-saving adapters or a busy upstream. The network tweaks and adapter power setting can help.".into(),
                severity: Severity::Recommended,
                action: SuggestionAction::Navigate("network"),
            });
        }
    } else {
        out.push(Suggestion {
            id: "probe".into(),
            title: "Measure your network route".into(),
            detail: "Clutchcore can test regional game gateways and tell you which route is actually fastest from this machine.".into(),
            severity: Severity::Optional,
            action: SuggestionAction::Navigate("network"),
        });
    }

    if report.ram_total_mb > 0 && report.ram_total_mb < 16384 {
        out.push(Suggestion {
            id: "ram".into(),
            title: format!(
                "{} MB of RAM limits heavy games",
                report.ram_total_mb
            ),
            detail: "16 GB is the practical floor for modern shooters with Discord and a browser open. Close background apps, or add memory if your motherboard has free slots.".into(),
            severity: if report.ram_total_mb < 8192 {
                Severity::Critical
            } else {
                Severity::Recommended
            },
            action: SuggestionAction::Info,
        });
    }

    if report.uptime_secs > 7 * 86400 {
        out.push(Suggestion {
            id: "uptime".into(),
            title: "This PC has been up for over a week".into(),
            detail: "Long uptimes accumulate background work and driver state. A restart before a session is free and often smooths frame times.".into(),
            severity: Severity::Optional,
            action: SuggestionAction::OpenUri("ms-settings:windowsupdate"),
        });
    }

    if report.startup_items.len() > 15 {
        out.push(Suggestion {
            id: "startup".into(),
            title: format!("{} startup items are loading with Windows", report.startup_items.len()),
            detail: "You do not need most of them before a match. Trim the list in Task Manager → Startup apps; everything stays installed.".into(),
            severity: Severity::Recommended,
            action: SuggestionAction::OpenUri("ms-settings:startupapps"),
        });
    }

    if let Some(disk) = report.disks.iter().find(|d| d.mount.starts_with('C')) {
        if disk.total_gb > 0.0 && disk.free_gb / disk.total_gb < 0.08 {
            out.push(Suggestion {
                id: "disk".into(),
                title: format!("C: has only {:.0} GB free", disk.free_gb),
                detail: "Windows gets sluggish when the system drive fills up. Clear shader caches and old downloads before they turn into stutter.".into(),
                severity: Severity::Recommended,
                action: SuggestionAction::OpenUri("ms-settings:storagesense"),
            });
        }
    }

    let only_software = !report.gpus.is_empty() && report.gpus.iter().all(|g| g.software);
    if only_software {
        out.push(Suggestion {
            id: "gpu-driver".into(),
            title: "Windows is using a software display adapter".into(),
            detail: "Install the GPU vendor's driver (NVIDIA, AMD or Intel). Nothing else will move frame rate until the real adapter is active.".into(),
            severity: Severity::Critical,
            action: SuggestionAction::OpenUri("ms-settings:windowsupdate"),
        });
    }

    let mut pending: Vec<(&TweakSpec, f32)> = Vec::new();
    for spec in tweaks::specs() {
        let applied = statuses
            .get(spec.id)
            .map(|s| s.state == TweakState::Applied)
            .unwrap_or(false);
        if applied {
            continue;
        }
        let weights = weights(spec.id);
        let priority = weights.iter().cloned().fold(0.0, f32::max);
        pending.push((spec, priority));
    }
    pending.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (spec, priority) in pending.into_iter().take(8) {
        let severity = if priority >= 25.0 {
            Severity::Critical
        } else if priority >= 10.0 {
            Severity::Recommended
        } else {
            Severity::Optional
        };
        let admin_note = if spec.requires_admin && !admin {
            " Needs administrator access."
        } else if spec.requires_reboot {
            " Needs a reboot to take effect."
        } else {
            ""
        };
        let detail = format!("{}{}", spec.summary, admin_note);
        out.push(Suggestion {
            id: format!("tweak:{}", spec.id),
            title: spec.name.to_string(),
            detail,
            severity,
            action: SuggestionAction::ApplyTweak(spec.id),
        });
    }

    out
}

pub fn category_score(impact: Impact, readiness: &Readiness) -> u32 {
    match impact {
        Impact::InputDelay => readiness.input,
        Impact::FramePacing => readiness.fps,
        Impact::Fps => readiness.fps,
        Impact::Ping => readiness.network,
    }
}

pub fn probe_quality(ping_ms: f64) -> (&'static str, u8) {
    if ping_ms <= 20.0 {
        ("Excellent", 0)
    } else if ping_ms <= 35.0 {
        ("Good", 1)
    } else if ping_ms <= 55.0 {
        ("Playable", 2)
    } else if ping_ms <= 90.0 {
        ("High", 3)
    } else {
        ("Poor", 4)
    }
}

pub fn time_ago(unix: u64) -> String {
    util::fmt_relative(unix)
}
