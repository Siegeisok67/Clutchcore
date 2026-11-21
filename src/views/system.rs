use eframe::egui::{self, Align, Layout, RichText, Vec2};

use crate::app::App;
use crate::model::Category;
use crate::presets;
use crate::theme;
use crate::ui;
use crate::util;

impl App {
    pub(crate) fn view_system(&mut self, ui: &mut egui::Ui) {
        let mut export = false;
        let mut rescan = false;
        let mut display_settings = false;
        let mut startup_settings = false;
        let mut open_profiles = false;
        let mut goto_network_tweaks = false;

        ui.horizontal(|ui| {
            if ui::accent_button(ui, "Export system report").clicked() {
                export = true;
            }
            if ui::ghost_button(ui, "Re-scan").clicked() {
                rescan = true;
            }
            if ui::ghost_button(ui, "Display settings").clicked() {
                display_settings = true;
            }
            if ui::ghost_button(ui, "Startup apps").clicked() {
                startup_settings = true;
            }
            if self.scan_running {
                ui.spinner();
                ui.label(
                    RichText::new("Scanning…")
                        .size(10.5)
                        .color(theme::MUTED),
                );
            }
        });

        ui.add_space(12.0);

        let report = &self.report;
        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Hardware",
                "Read directly from Windows — nothing is estimated",
                |_ui| {},
            );
            ui.add_space(4.0);
            egui::Grid::new("spec-grid")
                .num_columns(2)
                .spacing(Vec2::new(24.0, 3.0))
                .show(ui, |ui| {
                    spec(ui, "CPU", &report.cpu_name);
                    spec(
                        ui,
                        "GPU",
                        report
                            .gpus
                            .iter()
                            .find(|g| !g.software)
                            .map(|g| g.name.as_str())
                            .unwrap_or("Not detected"),
                    );
                    ui.end_row();
                    spec(
                        ui,
                        "Cores",
                        &format!(
                            "{} physical · {} logical",
                            report.physical_cores, report.logical_cores
                        ),
                    );
                    spec(
                        ui,
                        "Memory",
                        &format!(
                            "{:.0} GB installed · {:.1} GB in use",
                            report.ram_total_mb as f64 / 1024.0,
                            report.ram_used_mb as f64 / 1024.0
                        ),
                    );
                    ui.end_row();
                    spec(
                        ui,
                        "Motherboard",
                        if report.board.is_empty() {
                            "Unknown"
                        } else {
                            &report.board
                        },
                    );
                    spec(ui, "Operating system", &report.os_name);
                    ui.end_row();
                    spec(
                        ui,
                        "BIOS",
                        if report.bios.is_empty() {
                            "Unknown"
                        } else {
                            &report.bios
                        },
                    );
                    spec(ui, "Build", &report.os_version);
                    ui.end_row();
                    spec(
                        ui,
                        "Model",
                        if report.machine.is_empty() {
                            "Custom / unknown"
                        } else {
                            &report.machine
                        },
                    );
                    spec(ui, "Uptime", &util::human_uptime(report.uptime_secs));
                    ui.end_row();
                    spec(
                        ui,
                        "Power plan",
                        report
                            .power_plan
                            .as_ref()
                            .map(|p| p.name.as_str())
                            .unwrap_or("Unknown"),
                    );
                    spec(
                        ui,
                        "Computer",
                        if report.hostname.is_empty() {
                            "This PC"
                        } else {
                            &report.hostname
                        },
                    );
                    ui.end_row();
                });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui::admin_badge(ui, report.admin);
                if !report.admin {
                    ui.label(
                        RichText::new(
                            "Some scheduling, GPU and network tweaks stay locked until you elevate.",
                        )
                        .size(10.0)
                        .color(theme::MUTED),
                    );
                }
            });
        });

        ui.add_space(12.0);

        let displays = self.report.displays.clone();
        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Displays",
                "Refresh rate is the first thing any tuning depends on",
                |ui| {
                    if ui::small_ghost(ui, "Open display settings").clicked() {
                        display_settings = true;
                    }
                },
            );
            ui.add_space(2.0);
            if displays.is_empty() {
                ui::empty_state(ui, "No displays found", "Windows did not return an attached display.");
            }
            for display in &displays {
                let gap = display.max_refresh > display.refresh + 1 && display.max_refresh >= 100;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&display.name)
                                    .size(12.0)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            if display.primary {
                                ui::badge(ui, "PRIMARY", theme::ACCENT);
                            }
                            if gap {
                                ui::badge(ui, "BELOW MAX", theme::YELLOW);
                            }
                        });
                        ui.label(
                            RichText::new(format!(
                                "{} × {} · {} Hz{}",
                                display.width,
                                display.height,
                                display.refresh,
                                if display.max_refresh > display.refresh {
                                    format!(" · panel supports up to {} Hz", display.max_refresh)
                                } else {
                                    String::new()
                                }
                            ))
                            .size(10.0)
                            .color(theme::MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if gap && ui::small_ghost(ui, "Fix").clicked() {
                            display_settings = true;
                        }
                        ui::badge(
                            ui,
                            format!("{} HZ", display.refresh),
                            if display.refresh >= 120 {
                                theme::GREEN
                            } else {
                                theme::YELLOW
                            },
                        );
                    });
                });
                ui::separator(ui);
            }
            ui.label(
                RichText::new(
                    "If Windows reports 60 Hz on a 144 Hz monitor, change it in Settings → System → Display → Advanced display.",
                )
                .size(9.5)
                .color(theme::FAINT),
            );
        });

        ui.add_space(12.0);

        let gpus = self.report.gpus.clone();
        ui.columns(2, |cols| {
            ui::card(&mut cols[0], |ui| {
                ui::section_header_row(ui, "Graphics", "Adapters Windows will use", |_ui| {});
                ui.add_space(2.0);
                if gpus.is_empty() {
                    ui::empty_state(ui, "No adapter reported", "Install the GPU vendor driver.");
                }
                for gpu in &gpus {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(util::truncate(&gpu.name, 34))
                                    .size(11.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "Driver {}{}",
                                    if gpu.driver_version.is_empty() {
                                        "unknown"
                                    } else {
                                        &gpu.driver_version
                                    },
                                    if gpu.vram_mb > 0 {
                                        format!(" · {:.0} GB VRAM", gpu.vram_mb as f64 / 1024.0)
                                    } else {
                                        String::new()
                                    }
                                ))
                                .size(9.5)
                                .color(theme::MUTED),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if gpu.software {
                                ui::badge(ui, "SOFTWARE", theme::RED);
                            } else {
                                ui::badge(ui, "ACTIVE", theme::GREEN);
                            }
                        });
                    });
                }
            });

            let disks = self.report.disks.clone();
            ui::card(&mut cols[1], |ui| {
                ui::section_header_row(ui, "Storage", "Free space and drive type", |_ui| {});
                ui.add_space(2.0);
                if disks.is_empty() {
                    ui::empty_state(ui, "No drives returned", "Windows did not report any fixed drives.");
                }
                for disk in &disks {
                    let used = if disk.total_gb > 0.0 {
                        ((disk.total_gb - disk.free_gb) / disk.total_gb * 100.0) as u32
                    } else {
                        0
                    };
                    let low = disk.total_gb > 0.0 && disk.free_gb / disk.total_gb < 0.1;
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("{} {}", disk.mount, disk.kind))
                                    .size(11.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} free of {}",
                                    util::human_gb(disk.free_gb),
                                    util::human_gb(disk.total_gb)
                                ))
                                .size(9.5)
                                .color(if low { theme::YELLOW } else { theme::MUTED }),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{}% used", used))
                                    .size(10.0)
                                    .color(if low { theme::YELLOW } else { theme::MUTED }),
                            );
                        });
                    });
                    ui.add_space(3.0);
                    let width = ui.available_width();
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(width, 5.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, egui::Rounding::same(2.5), theme::RING_BG);
                    let fill_width = rect.width() * (used as f32 / 100.0);
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            rect.min,
                            Vec2::new(fill_width, rect.height()),
                        ),
                        egui::Rounding::same(2.5),
                        if low { theme::YELLOW } else { theme::BLUE },
                    );
                    ui.add_space(3.0);
                }
            });
        });

        ui.add_space(12.0);

        let adapters = self.report.adapters.clone();
        let detected = self.report.detected_games.clone();
        ui.columns(2, |cols| {
            ui::card(&mut cols[0], |ui| {
                ui::section_header_row(ui, "Network adapters", "Power saving can add jitter", |ui| {
                    if ui::small_ghost(ui, "Network tweaks").clicked() {
                        goto_network_tweaks = true;
                    }
                });
                ui.add_space(2.0);
                if adapters.is_empty() {
                    ui::empty_state(ui, "No adapters found", "Check the vendor driver.");
                }
                let mut hidden_virtual = 0;
                for adapter in &adapters {
                    if adapter.is_virtual {
                        hidden_virtual += 1;
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(util::truncate(&adapter.name, 32))
                                    .size(11.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{}{}",
                                    adapter.driver,
                                    adapter
                                        .power_management
                                        .map(|v| format!(" · power flags {v}"))
                                        .unwrap_or_default()
                                ))
                                .size(9.5)
                                .color(theme::MUTED),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            match adapter.power_management {
                                Some(24) => ui::badge(ui, "POWER SAVE OFF", theme::GREEN),
                                Some(_) => ui::badge(ui, "POWER SAVE ON", theme::YELLOW),
                                None => ui::badge(ui, "DEFAULT", theme::MUTED),
                            };
                        });
                    });
                    ui::separator(ui);
                }
                if hidden_virtual > 0 {
                    ui.label(
                        RichText::new(format!(
                            "{hidden_virtual} virtual adapter(s) hidden from this list."
                        ))
                        .size(9.0)
                        .color(theme::FAINT),
                    );
                }
                if !self.report.dns_servers.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "DNS: {}",
                            self.report.dns_servers.join(" · ")
                        ))
                        .size(9.5)
                        .color(theme::FAINT),
                    );
                }
            });

            ui::card(&mut cols[1], |ui| {
                ui::section_header_row(ui, "Detected games", "Matched on disk and in the registry", |ui| {
                    if ui::small_ghost(ui, "Presets").clicked() {
                        open_profiles = true;
                    }
                });
                ui.add_space(2.0);
                if detected.is_empty() {
                    ui::empty_state(
                        ui,
                        "No supported games detected",
                        "That does not stop you using the global tweaks or building a custom profile.",
                    );
                }
                for game in &detected {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&game.game)
                                .size(11.5)
                                .strong()
                                .color(theme::TEXT),
                        );
                        ui.label(
                            RichText::new(util::truncate(&game.path, 54))
                                .size(9.0)
                                .color(theme::MUTED),
                        )
                        .on_hover_text(&game.path);
                    });
                    ui.add_space(3.0);
                }
                let count = presets::all().len();
                ui.label(
                    RichText::new(format!(
                        "{} game presets available · {}/{} detected",
                        count,
                        detected.len(),
                        count
                    ))
                    .size(9.0)
                    .color(theme::FAINT),
                );
            });
        });

        ui.add_space(12.0);

        let items = self.report.startup_items.clone();
        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Startup load",
                "Everything that boots with Windows",
                |ui| {
                    if ui::small_ghost(ui, "Manage").clicked() {
                        startup_settings = true;
                    }
                },
            );
            ui.add_space(2.0);
            if items.is_empty() {
                ui::empty_state(ui, "Nothing in the usual startup locations", "Clean.");
            } else {
                ui.label(
                    RichText::new(format!(
                        "{} item(s). Disable what you do not need before a match — everything stays installed.",
                        items.len()
                    ))
                    .size(10.0)
                    .color(theme::MUTED),
                );
                ui.add_space(4.0);
                for item in items.iter().take(12) {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(util::truncate(&item.name, 40))
                                    .size(10.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                RichText::new(util::truncate(&item.command, 72))
                                    .size(9.0)
                                    .color(theme::FAINT),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui::badge(ui, item.scope.as_str(), theme::MUTED);
                        });
                    });
                }
                if items.len() > 12 {
                    ui.label(
                        RichText::new(format!("… and {} more.", items.len() - 12))
                            .size(9.0)
                            .color(theme::FAINT),
                    );
                }
            }
        });

        if !self.report.errors.is_empty() {
            ui.add_space(12.0);
            let errors = self.report.errors.clone();
            ui::card_fill(ui, theme::YELLOW_TINT, |ui| {
                ui.label(
                    RichText::new("Scan notes")
                        .size(12.0)
                        .strong()
                        .color(theme::YELLOW),
                );
                for error in &errors {
                    ui.label(RichText::new(error).size(10.0).color(theme::MUTED));
                }
            });
        }

        if export {
            self.export_report();
        }
        if rescan {
            self.start_scan();
        }
        if display_settings {
            util::open_uri("ms-settings:display-advanced");
        }
        if startup_settings {
            util::open_uri("ms-settings:startupapps");
        }
        if open_profiles {
            self.tab = crate::app::Tab::Profiles;
        }
        if goto_network_tweaks {
            self.tweak_filter = Some(Category::Network);
            self.tab = crate::app::Tab::Tweaks;
        }
    }
}

fn spec(ui: &mut egui::Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [86.0, 16.0],
            egui::Label::new(
                RichText::new(key)
                    .size(9.5)
                    .strong()
                    .color(theme::MUTED),
            ),
        );
        ui.label(
            RichText::new(util::truncate(value, 44))
                .size(11.0)
                .color(theme::TEXT),
        )
        .on_hover_text(value);
    });
}
