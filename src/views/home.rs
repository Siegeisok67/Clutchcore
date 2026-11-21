use eframe::egui::{self, Align, Layout, RichText};

use crate::analysis;
use crate::app::{App, Tab};
use crate::model::TweakState;
use crate::theme;
use crate::tweaks;
use crate::ui;
use crate::util;

impl App {
    pub(crate) fn view_dashboard(&mut self, ui: &mut egui::Ui) {
        let readiness = analysis::readiness(
            &self.statuses,
            &self.report,
            self.last_probe_summary.as_ref(),
        );
        let suggestions = analysis::suggestions(
            &self.statuses,
            &self.report,
            self.last_probe_summary.as_ref(),
            self.admin,
        );
        let mut chosen: Option<analysis::Suggestion> = None;
        let mut go_tweaks = false;
        let mut go_vault = false;

        ui::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui::score_ring(
                    ui,
                    readiness.overall,
                    150.0,
                    ui::score_color(readiness.overall),
                    "Ready",
                );
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("COMPETITIVE READINESS")
                                .size(9.0)
                                .strong()
                                .color(theme::ACCENT),
                        );
                        ui::admin_badge(ui, self.admin);
                        if let Some(plan) = &self.active_plan {
                            ui::badge(ui, plan.name.to_uppercase(), theme::MUTED);
                        }
                    });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(analysis::grade(readiness.overall))
                            .size(21.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(
                        RichText::new(analysis::grade_note(readiness.overall))
                            .size(11.5)
                            .color(theme::MUTED),
                    );
                    ui.add_space(10.0);
                    ui.columns(2, |cols| {
                        ui::score_bar(&mut cols[0], "INPUT", readiness.input, theme::BLUE);
                        cols[0].add_space(5.0);
                        ui::score_bar(&mut cols[0], "LATENCY", readiness.latency, theme::ACCENT);
                        ui::score_bar(&mut cols[1], "FPS", readiness.fps, theme::GREEN);
                        cols[1].add_space(5.0);
                        ui::score_bar(&mut cols[1], "NETWORK", readiness.network, theme::PURPLE);
                    });
                });
            });
        });

        ui.add_space(12.0);

        let cpu = util::truncate(&self.report.cpu_name, 28);
        let cpu_hint = if self.report.cpu_mhz > 0 {
            format!(
                "{:.1} GHz · {}C / {}T",
                self.report.cpu_mhz as f64 / 1000.0,
                self.report.physical_cores,
                self.report.logical_cores
            )
        } else {
            "Scanning…".into()
        };
        let gpu = self
            .report
            .gpus
            .iter()
            .find(|g| !g.software)
            .map(|g| util::truncate(&g.name, 30))
            .unwrap_or_else(|| "Scanning…".into());
        let gpu_hint = self
            .report
            .gpus
            .iter()
            .find(|g| !g.software)
            .map(|g| {
                if g.vram_mb > 0 {
                    format!("{:.0} GB VRAM", g.vram_mb as f64 / 1024.0)
                } else {
                    g.driver_version.clone()
                }
            })
            .unwrap_or_default();
        let ram = if self.report.ram_total_mb > 0 {
            format!("{:.0} GB", self.report.ram_total_mb as f64 / 1024.0)
        } else {
            "Scanning…".into()
        };
        let ram_hint = if self.live.ram_used_mb > 0 {
            format!("{:.1} GB in use", self.live.ram_used_mb as f64 / 1024.0)
        } else {
            String::new()
        };
        let display = self
            .report
            .displays
            .iter()
            .find(|d| d.primary)
            .or_else(|| self.report.displays.first());
        let display_value = display
            .map(|d| format!("{} Hz", d.refresh))
            .unwrap_or_else(|| "Scanning…".into());
        let display_hint = display
            .map(|d| {
                if d.max_refresh > d.refresh + 1 {
                    format!("{} · max {} Hz", d.resolution(), d.max_refresh)
                } else {
                    d.resolution()
                }
            })
            .unwrap_or_default();

        ui.columns(4, |cols| {
            ui::stat_tile(&mut cols[0], "CPU", &cpu, &cpu_hint);
            ui::stat_tile(&mut cols[1], "GPU", &gpu, &gpu_hint);
            ui::stat_tile(&mut cols[2], "Memory", &ram, &ram_hint);
            ui::stat_tile(&mut cols[3], "Display", &display_value, &display_hint);
        });

        ui.add_space(12.0);

        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Priority fixes",
                "Ordered by impact on the input path and frame delivery",
                |ui| {
                    if ui::small_ghost(ui, "All tweaks").clicked() {
                        go_tweaks = true;
                    }
                },
            );
            ui.add_space(4.0);

            if self.scan_running && self.report.logical_cores == 0 {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new("Scanning hardware, displays and installed games…")
                            .size(11.5)
                            .color(theme::MUTED),
                    );
                });
            } else if suggestions.is_empty() {
                ui::empty_state(
                    ui,
                    "Nothing obvious left",
                    "Every recommendation Clutchcore knows about is already satisfied. Keep the machine stable.",
                );
            } else {
                let shown: Vec<analysis::Suggestion> = suggestions.iter().take(6).cloned().collect();
                let total = shown.len();
                for (index, suggestion) in shown.into_iter().enumerate() {
                    let action_label = match &suggestion.action {
                        analysis::SuggestionAction::ApplyTweak(_) => "Apply",
                        analysis::SuggestionAction::OpenUri(_) => "Open",
                        analysis::SuggestionAction::Navigate(_) => "Open",
                        analysis::SuggestionAction::Elevate => "Elevate",
                        analysis::SuggestionAction::Info => "",
                    };
                    let accent = matches!(
                        suggestion.action,
                        analysis::SuggestionAction::ApplyTweak(_)
                    );
                    if ui::suggestion_row(
                        ui,
                        ui::severity_color(suggestion.severity),
                        &suggestion.title,
                        &suggestion.detail,
                        action_label,
                        accent,
                    ) {
                        chosen = Some(suggestion);
                    }
                    if index + 1 < total {
                        ui::separator(ui);
                    }
                }
            }
        });

        if let Some(suggestion) = chosen {
            self.handle_suggestion(suggestion);
        }
        if go_tweaks {
            self.tab = Tab::Tweaks;
        }

        ui.add_space(12.0);

        let games = self.running_games.clone();
        let boosted = self.boosted.clone();
        let core_ids = [
            "mouse_accel_off",
            "game_dvr_off",
            "power_plan_high",
            "tcp_latency",
        ];
        let core: Vec<(&'static crate::model::TweakSpec, bool, String)> = core_ids
            .iter()
            .filter_map(|id| {
                tweaks::find(id).map(|spec| {
                    let (applied, detail) = self
                        .statuses
                        .get(spec.id)
                        .map(|status| {
                            (
                                status.state == TweakState::Applied,
                                status.detail.clone(),
                            )
                        })
                        .unwrap_or((false, "Not checked".into()));
                    (spec, applied, detail)
                })
            })
            .collect();
        let mut boost_action: Option<(u32, String)> = None;
        let mut tweak_action: Option<String> = None;
        let mut scan_games = false;
        let mut open_all = false;
        let admin = self.admin;

        ui.columns(2, |cols| {
            ui::card(&mut cols[0], |ui| {
                ui::section_header_row(ui, "Live session", "Temporary, per-process priority", |ui| {
                    if ui::small_ghost(ui, "Refresh").clicked() {
                        scan_games = true;
                    }
                });
                ui.add_space(4.0);
                if games.is_empty() {
                    ui::empty_state(
                        ui,
                        "No supported game running",
                        "Launch a game Clutchcore has a preset for and it will appear here with a one-click priority boost.",
                    );
                } else {
                    for game in &games {
                        let on = boosted.contains(&game.pid);
                        ui.horizontal(|ui| {
                            ui::dot(ui, if on { theme::GREEN } else { theme::MUTED }, 10.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&game.game)
                                        .size(12.0)
                                        .strong()
                                        .color(theme::TEXT),
                                );
                                ui.label(
                                    RichText::new(format!("{} · PID {}", game.exe, game.pid))
                                        .size(9.5)
                                        .color(theme::FAINT),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let label = if on { "Set normal" } else { "Boost" };
                                if ui::small_ghost(ui, label).clicked() {
                                    boost_action = Some((game.pid, game.game.clone()));
                                }
                                if on {
                                    ui::badge(ui, "HIGH", theme::GREEN);
                                }
                            });
                        });
                    }
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Priority resets when the game closes. Windows may refuse changes to protected processes.",
                    )
                    .size(9.5)
                    .color(theme::FAINT),
                );
                if !admin {
                    ui.label(
                        RichText::new("Some games need Clutchcore running as administrator.")
                            .size(9.5)
                            .color(theme::YELLOW),
                    );
                }
            });

            ui::card(&mut cols[1], |ui| {
                ui::section_header_row(ui, "Core toggles", "The highest-impact settings", |ui| {
                    if ui::small_ghost(ui, "Open all").clicked() {
                        open_all = true;
                    }
                });
                ui.add_space(2.0);
                for (spec, applied, detail) in &core {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(spec.name)
                                    .size(11.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                RichText::new(util::truncate(detail, 38))
                                    .size(9.5)
                                    .color(theme::FAINT),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui::toggle_switch(ui, *applied).clicked() {
                                tweak_action = Some(spec.id.to_string());
                            }
                        });
                    });
                    ui::separator(ui);
                }
                if let Some(spec) = tweaks::find("hags_on") {
                    let applied = self
                        .statuses
                        .get(spec.id)
                        .map(|s| s.state == TweakState::Applied)
                        .unwrap_or(false);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("GPU scheduling (HAGS)")
                                    .size(11.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                RichText::new("Reboot required · administrator")
                                    .size(9.5)
                                    .color(theme::FAINT),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui::toggle_switch(ui, applied).clicked() {
                                tweak_action = Some(spec.id.to_string());
                            }
                        });
                    });
                }
            });
        });

        if scan_games {
            self.running_games = crate::system::running_games();
            self.toast(
                crate::logging::Level::Info,
                format!("{} supported game process(es) found.", self.running_games.len()),
            );
        }
        if let Some((pid, game)) = boost_action {
            self.toggle_boost(pid, &game);
        }
        if let Some(id) = tweak_action {
            self.toggle_tweak(&id);
        }
        if open_all {
            self.tab = Tab::Tweaks;
        }

        ui.add_space(12.0);

        ui::card_fill(ui, theme::PANEL, |ui| {
            ui.horizontal(|ui| {
                ui::dot(ui, theme::GREEN, 12.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Original settings are always saved first")
                            .size(11.5)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} tweak(s) captured in originals.json · {} snapshot(s) on disk · readable at any time",
                            self.originals.entries.len(),
                            self.snapshots.len()
                        ))
                        .size(10.0)
                        .color(theme::MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::ghost_button(ui, "Open vault").clicked() {
                        go_vault = true;
                    }
                });
            });
        });

        if go_vault {
            self.tab = Tab::Vault;
        }
    }
}
