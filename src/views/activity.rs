use eframe::egui::{self, RichText};

use crate::app::App;
use crate::logging::Level;
use crate::theme;
use crate::ui;
use crate::util;

impl App {
    pub(crate) fn view_activity(&mut self, ui: &mut egui::Ui) {
        let mut open_log = false;
        let mut open_folder = false;
        let mut restore = false;

        ui.horizontal(|ui| {
            if ui::accent_button(ui, "Open log file").clicked() {
                open_log = true;
            }
            if ui::ghost_button(ui, "Open data folder").clicked() {
                open_folder = true;
            }
            if ui::ghost_button(ui, "Restore originals").clicked() {
                restore = true;
            }
        });

        ui.add_space(12.0);

        if let Some(report) = self.last_report.clone() {
            ui::card(ui, |ui| {
                ui::section_header_row(
                    ui,
                    "Last action",
                    &report.label,
                    |ui| {
                        let failed = report.failed();
                        ui::badge(
                            ui,
                            if failed > 0 {
                                format!("{failed} FAILED")
                            } else {
                                "SUCCESS".into()
                            },
                            if failed > 0 { theme::RED } else { theme::GREEN },
                        );
                        if report.needs_reboot {
                            ui::badge(ui, "REBOOT REQUIRED", theme::BLUE);
                        }
                    },
                );
                ui.add_space(4.0);
                for result in &report.results {
                    let (label, color) = if !result.ok {
                        ("FAIL", theme::RED)
                    } else if result.skipped {
                        ("SKIP", theme::MUTED)
                    } else {
                        ("OK", theme::GREEN)
                    };
                    ui.horizontal(|ui| {
                        ui::badge(ui, label, color);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(&result.name)
                                    .size(10.5)
                                    .color(theme::TEXT),
                            );
                            if let Some(error) = &result.error {
                                ui.label(
                                    RichText::new(util::truncate(error, 110))
                                        .size(9.0)
                                        .color(theme::RED),
                                )
                                .on_hover_text(error);
                            }
                        });
                    });
                }
                if let Some(id) = &report.snapshot_id {
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new(format!("Backup snapshot: {id}"))
                            .font(egui::FontId::monospace(9.0))
                            .color(theme::FAINT),
                    );
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new(report.summary())
                        .size(9.5)
                        .color(theme::MUTED),
                );
            });
            ui.add_space(12.0);
        }

        let entries = self.log.clone();
        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Activity log",
                &format!("{} entries in memory · full log in clutchcore.log", entries.len()),
                |_ui| {},
            );
            ui.add_space(4.0);
            if entries.is_empty() {
                ui::empty_state(ui, "Nothing logged yet", "Actions will appear here as you use the app.");
            }
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for entry in entries.iter().rev().take(220) {
                        let color = match entry.level {
                            Level::Success => theme::GREEN,
                            Level::Warn => theme::YELLOW,
                            Level::Error => theme::RED,
                            Level::Info => theme::BLUE,
                        };
                        ui.horizontal_top(|ui| {
                            ui.add_sized(
                                [54.0, 14.0],
                                egui::Label::new(
                                    RichText::new(&entry.time)
                                        .font(egui::FontId::monospace(8.5))
                                        .color(theme::FAINT),
                                ),
                            );
                            ui.add_sized(
                                [38.0, 14.0],
                                egui::Label::new(
                                    RichText::new(entry.level.tag())
                                        .size(8.0)
                                        .strong()
                                        .color(color),
                                ),
                            );
                            ui.label(
                                RichText::new(&entry.message)
                                    .size(9.5)
                                    .color(theme::MUTED),
                            );
                        });
                    }
                });
        });

        ui.add_space(12.0);

        ui.columns(2, |cols| {
            ui::card_fill(&mut cols[0], theme::GREEN_TINT, |ui| {
                ui.label(
                    RichText::new("Safety model")
                        .size(12.5)
                        .strong()
                        .color(theme::GREEN),
                );
                ui.add_space(3.0);
                for line in [
                    "No telemetry. The only network traffic is the route probe you start.",
                    "Backups are written before any registry value is touched.",
                    "Every toggle-off restores the captured original, not a guessed default.",
                    "Clutchcore never reads or changes game files, anti-cheat or process memory.",
                    "Profiles never apply themselves; you choose when to apply them.",
                ] {
                    ui.label(
                        RichText::new(format!("•  {line}"))
                            .size(9.5)
                            .color(theme::MUTED),
                    );
                    ui.add_space(1.0);
                }
            });

            ui::card_fill(&mut cols[1], theme::PANEL, |ui| {
                ui.label(
                    RichText::new("Headless commands")
                        .size(12.5)
                        .strong()
                        .color(theme::TEXT),
                );
                ui.add_space(3.0);
                ui.label(
                    RichText::new(
                        "The same engine is available without the GUI, useful for scripted checks.",
                    )
                    .size(9.5)
                    .color(theme::MUTED),
                );
                ui.add_space(5.0);
                for command in [
                    "Clutchcore.exe --scan",
                    "Clutchcore.exe --tweaks",
                    "Clutchcore.exe --probe",
                    "Clutchcore.exe --profiles",
                    "Clutchcore.exe --selftest",
                ] {
                    ui::code(ui, command);
                }
            });
        });

        if open_log {
            util::open_uri(&crate::logging::log_file().display().to_string());
        }
        if open_folder {
            util::open_path(&self.data_dir);
        }
        if restore {
            self.ask_restore_all();
        }
    }
}
