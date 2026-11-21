use std::cmp::Ordering;

use eframe::egui::{self, Align, Layout, RichText, Vec2};

use crate::analysis;
use crate::app::App;
use crate::model::TweakState;
use crate::network;
use crate::theme;
use crate::tweaks;
use crate::ui;
use crate::util;

impl App {
    pub(crate) fn view_network(&mut self, ui: &mut egui::Ui) {
        let summary = self.last_probe_summary.clone();
        let results = self.probe_results.clone();
        let last_tested = results.iter().map(|r| r.tested_unix).max().unwrap_or(0);

        ui.columns(3, |cols| {
            match &summary {
                Some(summary) => {
                    ui::stat_tile(
                        &mut cols[0],
                        "Best route",
                        &format!("{:.0} ms", summary.best_ms),
                        &summary.best_label,
                    );
                    ui::stat_tile(
                        &mut cols[1],
                        "Average jitter",
                        &format!("{:.1} ms", summary.avg_jitter),
                        if summary.avg_jitter <= 6.0 {
                            "Stable"
                        } else if summary.avg_jitter <= 15.0 {
                            "Some variation"
                        } else {
                            "Spiky"
                        },
                    );
                    ui::stat_tile(
                        &mut cols[2],
                        "Routes answered",
                        &format!("{}", summary.tested),
                        &if last_tested > 0 {
                            format!("Tested {}", util::fmt_relative(last_tested))
                        } else {
                            "This session".into()
                        },
                    );
                }
                None => {
                    ui::stat_tile(&mut cols[0], "Best route", "—", "No route test yet");
                    ui::stat_tile(&mut cols[1], "Average jitter", "—", "Jitter beats average ping");
                    ui::stat_tile(&mut cols[2], "Routes answered", "0", "Run a probe to measure");
                }
            }
        });

        ui.add_space(12.0);

        let running = self.probe_running;
        let completed = self.probe_completed;
        let expected = self.probe_expected;
        let scope = self.probe_provider.clone();
        let scope_label = match &scope {
            Some(provider) => format!("Run {provider} routes"),
            None => "Run all routes".into(),
        };
        let mut run = false;
        let mut provider_change: Option<Option<String>> = None;
        let mut clear = false;

        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Route probe",
                "TCP handshake to each provider's public gateway — a practical proxy for the real path",
                |ui| {
                    if ui::accent_button(ui, scope_label.clone()).clicked() {
                        run = true;
                    }
                    if ui::small_ghost(ui, "Clear").clicked() {
                        clear = true;
                    }
                },
            );
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui::chip(ui, "All providers", scope.is_none()).clicked() {
                    provider_change = Some(None);
                }
                for provider in network::providers() {
                    if ui::chip(ui, provider, scope.as_deref() == Some(provider)).clicked() {
                        provider_change = Some(Some(provider.to_string()));
                    }
                }
            });
            ui.add_space(4.0);
            if running {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new(format!(
                            "Testing {} route(s) in parallel… {} finished",
                            expected, completed
                        ))
                        .size(10.5)
                        .color(theme::MUTED),
                    );
                });
                let target = if expected > 0 {
                    completed as f32 / expected as f32
                } else {
                    0.0
                };
                let fraction = ui.ctx().animate_value_with_time(
                    egui::Id::new("probe-progress"),
                    target,
                    0.35,
                );
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(ui.available_width())
                        .fill(theme::ACCENT)
                        .show_percentage(),
                );
            } else {
                ui.label(
                    RichText::new(
                        "Each result is eight TCP handshakes with a 1.1 s timeout, run in parallel. Your browser, game launcher and Discord are not touched. In-game UDP latency will be a few milliseconds lower, but the ranking between routes usually matches.",
                    )
                    .size(10.0)
                    .color(theme::MUTED),
                );
            }
        });

        if let Some(change) = provider_change {
            self.probe_provider = change;
        }
        if clear {
            self.probe_results.retain(|r| r.provider == "Custom");
            self.last_probe_summary = network::summarize(&self.probe_results);
        }
        if run {
            self.start_probe_all(self.probe_provider.clone());
        }

        ui.add_space(12.0);

        let mut sorted = results;
        sorted.sort_by(|a, b| match (a.ok(), b.ok()) {
            (true, true) => a
                .avg_ms
                .partial_cmp(&b.avg_ms)
                .unwrap_or(Ordering::Equal),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => a.label.cmp(&b.label),
        });
        let best = sorted.iter().find(|r| r.ok()).map(|r| (r.host.clone(), r.port));

        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Measured routes",
                &if last_tested > 0 {
                    format!(
                        "Lowest TCP handshake first · last tested {}",
                        util::fmt_relative(last_tested)
                    )
                } else {
                    "Nothing measured yet".into()
                },
                |_ui| {},
            );
            ui.add_space(6.0);
            if sorted.is_empty() {
                ui::empty_state(
                    ui,
                    "No route data",
                    "Run a probe, or add a custom host below. Results are cached in last-probe.json.",
                );
            } else {
                egui::Grid::new("probe-result-grid")
                    .num_columns(6)
                    .spacing(Vec2::new(14.0, 5.0))
                    .show(ui, |ui| {
                        for header in ["ROUTE", "ENDPOINT", "PING", "JITTER", "LOSS", "TRACE"] {
                            ui.label(
                                RichText::new(header)
                                    .size(8.5)
                                    .strong()
                                    .color(theme::FAINT),
                            );
                        }
                        ui.end_row();

                        for result in &sorted {
                            let is_best = best
                                .as_ref()
                                .map(|(host, port)| *host == result.host && *port == result.port)
                                .unwrap_or(false);
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&result.label)
                                            .size(11.0)
                                            .strong()
                                            .color(theme::TEXT),
                                    );
                                    if is_best {
                                        ui::badge(ui, "BEST", theme::GREEN);
                                    }
                                });
                                ui.label(
                                    RichText::new(&result.region)
                                        .size(8.5)
                                        .color(theme::FAINT),
                                );
                            });
                            ui.label(
                                RichText::new(
                                    result.ip.clone().unwrap_or_else(|| "—".into()),
                                )
                                .font(egui::FontId::monospace(9.5))
                                .color(theme::MUTED),
                            );
                            if let Some(error) = &result.error {
                                ui.label(
                                    RichText::new("failed")
                                        .size(10.0)
                                        .color(theme::RED),
                                )
                                .on_hover_text(error);
                            } else {
                                ui.label(
                                    RichText::new(format!("{:.0} ms", result.avg_ms))
                                        .size(11.0)
                                        .strong()
                                        .color(ui::ping_color(result.avg_ms)),
                                )
                                .on_hover_text(format!(
                                    "min {:.0} ms · max {:.0} ms · {} samples",
                                    result.min_ms,
                                    result.max_ms,
                                    result.samples.len()
                                ));
                            }
                            ui.label(
                                RichText::new(format!("{:.1} ms", result.jitter_ms))
                                    .size(10.0)
                                    .color(ui::jitter_color(result.jitter_ms)),
                            );
                            ui.label(
                                RichText::new(format!("{:.0}%", result.loss_pct))
                                    .size(10.0)
                                    .color(if result.loss_pct > 0.0 {
                                        theme::YELLOW
                                    } else {
                                        theme::MUTED
                                    }),
                            );
                            ui::sparkline(
                                ui,
                                &result.samples,
                                54.0,
                                13.0,
                                if result.ok() {
                                    ui::ping_color(result.avg_ms)
                                } else {
                                    theme::FAINT
                                },
                            );
                            ui.end_row();
                        }
                    });
            }
        });

        ui.add_space(12.0);

        let custom_result = self.custom_result.clone();
        let custom_running = self.custom_running;
        let mut host = self.custom_host.clone();
        let mut probe_custom = false;

        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Custom host",
                "Probe a game server, relay IP or anything you can reach",
                |_ui| {},
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut host)
                        .hint_text("game.example.com:443")
                        .desired_width(300.0),
                );
                let enter = response.lost_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if ui::ghost_button(ui, "Probe host").clicked() || enter {
                    probe_custom = true;
                }
                if custom_running {
                    ui.spinner();
                    ui.label(
                        RichText::new("Probing…")
                            .size(10.0)
                            .color(theme::MUTED),
                    );
                }
            });
            if let Some(result) = &custom_result {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui::dot(
                        ui,
                        if result.ok() {
                            ui::ping_color(result.avg_ms)
                        } else {
                            theme::RED
                        },
                        10.0,
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(util::truncate(&result.host, 52))
                                .size(11.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                        if result.ok() {
                            ui.label(
                                RichText::new(format!(
                                    "{:.1} ms average · {:.1} ms jitter · {:.0}% loss · {}",
                                    result.avg_ms,
                                    result.jitter_ms,
                                    result.loss_pct,
                                    result.ip.clone().unwrap_or_default()
                                ))
                                .size(9.5)
                                .color(theme::MUTED),
                            );
                        } else {
                            ui.label(
                                RichText::new(
                                    result
                                        .error
                                        .clone()
                                        .unwrap_or_else(|| "Probe failed".into()),
                                )
                                .size(9.5)
                                .color(theme::RED),
                            );
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if result.ok() {
                            ui::badge(
                                ui,
                                analysis::probe_quality(result.avg_ms).0.to_uppercase(),
                                ui::ping_color(result.avg_ms),
                            );
                        } else {
                            ui::badge(ui, "FAILED", theme::RED);
                        }
                    });
                });
            }
        });

        self.custom_host = host;
        if probe_custom {
            self.start_custom_probe();
        }

        ui.add_space(12.0);

        let tweak_ids = ["tcp_latency", "network_throttling_off", "nic_power_off"];
        let mut toggle: Option<String> = None;
        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Network tweaks",
                "Windows TCP behaviour — every original value is backed up",
                |_ui| {},
            );
            ui.add_space(2.0);
            for id in tweak_ids {
                let Some(spec) = tweaks::find(id) else {
                    continue;
                };
                let status = self.statuses.get(id).cloned();
                let applied = status
                    .as_ref()
                    .map(|s| s.state == TweakState::Applied)
                    .unwrap_or(false);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(spec.name)
                                    .size(11.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            if spec.requires_admin {
                                ui::badge(ui, "ADMIN", theme::YELLOW);
                            }
                        });
                        ui.label(
                            RichText::new(status.map(|s| s.detail).unwrap_or_default())
                                .size(9.5)
                                .color(ui::state_color(if applied {
                                    TweakState::Applied
                                } else {
                                    TweakState::NotApplied
                                })),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::toggle_switch(ui, applied).clicked() {
                            toggle = Some(id.to_string());
                        }
                    });
                });
                ui::separator(ui);
            }
            ui.label(
                RichText::new(
                    "Nagle and delayed ACK changes apply per active interface. A VPN or a new adapter may need the tweak re-applied.",
                )
                .size(9.0)
                .color(theme::FAINT),
            );
        });
        if let Some(id) = toggle {
            self.toggle_tweak(&id);
        }

        ui.add_space(12.0);

        let mut open_network = false;
        let mut open_advanced = false;
        ui::card_fill(ui, theme::PANEL, |ui| {
            ui::section_header_row(
                ui,
                "How to read this",
                "What actually moves ping, and what does not",
                |_ui| {},
            );
            ui.add_space(4.0);
            for line in [
                "Geography wins. The closest server region will beat every TCP tweak on this page.",
                "Jitter beats average. A steady 45 ms route feels better than a 20 ms route that spikes to 80 ms.",
                "Ethernet is the single biggest consistency upgrade. On Wi-Fi, use 5/6 GHz and disable adapter power saving.",
                "Bufferbloat test: start a large download, then watch your ping. If it balloons, enable SQM/QoS on the router and prioritise the gaming PC.",
                "VPNs, overlays, cloud sync and Windows Update downloads all add hops or saturate the uplink. Pause them.",
                "Clutchcore's numbers are TCP handshakes to provider gateways, not in-game UDP. Use them to compare routes, not as an absolute in-game ping.",
            ] {
                ui.label(
                    RichText::new(format!("•  {line}"))
                        .size(10.5)
                        .color(theme::MUTED),
                );
                ui.add_space(2.0);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui::ghost_button(ui, "Network settings").clicked() {
                    open_network = true;
                }
                if ui::ghost_button(ui, "Advanced network settings").clicked() {
                    open_advanced = true;
                }
            });
        });

        if open_network {
            util::open_uri("ms-settings:network-status");
        }
        if open_advanced {
            util::open_uri("ms-settings:network-advancedsettings");
        }
    }
}
