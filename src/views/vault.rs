use eframe::egui::{self, Align, Layout, RichText};

use crate::app::App;
use crate::backup;
use crate::theme;
use crate::ui;
use crate::util;

impl App {
    pub(crate) fn view_vault(&mut self, ui: &mut egui::Ui) {
        let mut restore_all = false;
        let mut export = false;
        let mut open_folder = false;

        let last_relative = if self.originals.updated_unix > 0 {
            util::fmt_relative(self.originals.updated_unix)
        } else {
            "Never".to_string()
        };
        let last_absolute = if self.originals.updated_unix > 0 {
            util::fmt_datetime(self.originals.updated_unix)
        } else {
            "Nothing changed yet".to_string()
        };

        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Original settings vault",
                "The exact values Windows had before Clutchcore changed anything",
                |_ui| {},
            );
            ui.add_space(6.0);
            ui.columns(3, |cols| {
                ui::stat_tile(
                    &mut cols[0],
                    "Tweaks captured",
                    &format!("{}", self.originals.entries.len()),
                    "originals.json",
                );
                ui::stat_tile(
                    &mut cols[1],
                    "Snapshots",
                    &format!("{}", self.snapshots.len()),
                    "timestamped history",
                );
                ui::stat_tile(
                    &mut cols[2],
                    "Last capture",
                    &last_relative,
                    &last_absolute,
                );
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui::danger_button(ui, "Restore every original").clicked() {
                    restore_all = true;
                }
                if ui::ghost_button(ui, "Export readable copy").clicked() {
                    export = true;
                }
                if ui::ghost_button(ui, "Open data folder").clicked() {
                    open_folder = true;
                }
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Two layers of safety: originals.json keeps the first value Clutchcore ever saw for each tweak, and every apply creates a timestamped snapshot. Both are plain JSON you can open in any editor — the export below is a human-readable copy.",
                )
                .size(10.0)
                .color(theme::MUTED),
            );
        });

        ui.add_space(12.0);

        let mut snapshots = self.snapshots.clone();
        snapshots.sort_by(|a, b| b.created_unix.cmp(&a.created_unix));

        if snapshots.is_empty() {
            ui::card(ui, |ui| {
                ui::empty_state(
                    ui,
                    "No snapshots yet",
                    "Apply a tweak or profile and the pre-change values will appear here, newest first.",
                );
            });
        }

        for snapshot in snapshots {
            let expanded = self.expanded_snapshot.as_deref() == Some(snapshot.id.as_str());
            let mut view = false;
            let mut restore = false;
            let mut export_snapshot = false;
            let ok = snapshot.results.iter().filter(|result| result.ok).count();
            let failed = snapshot.results.iter().filter(|result| !result.ok).count();

            ui::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&snapshot.label)
                                    .size(12.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            if snapshot.restored {
                                ui::badge(ui, "RESTORED", theme::GREEN);
                            }
                            if failed > 0 {
                                ui::badge(ui, format!("{failed} FAILED"), theme::RED);
                            } else if !snapshot.results.is_empty() {
                                ui::badge(ui, format!("{ok} OK"), theme::GREEN);
                            }
                        });
                        ui.label(
                            RichText::new(format!(
                                "{} · {} tweak(s){}",
                                util::fmt_datetime(snapshot.created_unix),
                                snapshot.entries.len(),
                                snapshot
                                    .profile
                                    .as_ref()
                                    .map(|p| format!(" · {p}"))
                                    .unwrap_or_default()
                            ))
                            .size(9.5)
                            .color(theme::MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::small_ghost(ui, "Restore").clicked() {
                            restore = true;
                        }
                        if ui::small_ghost(ui, "Export .txt").clicked() {
                            export_snapshot = true;
                        }
                        if ui::small_ghost(ui, if expanded { "Hide" } else { "View" }).clicked() {
                            view = true;
                        }
                    });
                });

                if expanded {
                    ui::separator(ui);
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for entry in &snapshot.entries {
                                ui.label(
                                    RichText::new(&entry.tweak_name)
                                        .size(10.5)
                                        .strong()
                                        .color(theme::TEXT),
                                );
                                for reg in &entry.regs {
                                    ui.label(
                                        RichText::new(util::truncate(&reg.path_label(), 104))
                                            .font(egui::FontId::monospace(8.5))
                                            .color(theme::FAINT),
                                    )
                                    .on_hover_text(reg.path_label());
                                    let original =
                                        backup::value_or_missing(reg.value.as_ref());
                                    let current = reg.current_value();
                                    let now = backup::value_or_missing(current.as_ref());
                                    let changed = original != now;
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [52.0, 14.0],
                                            egui::Label::new(
                                                RichText::new("ORIGINAL")
                                                    .size(7.5)
                                                    .strong()
                                                    .color(theme::FAINT),
                                            ),
                                        );
                                        ui.label(
                                            RichText::new(util::truncate(&original, 76))
                                                .font(egui::FontId::monospace(9.0))
                                                .color(theme::MUTED),
                                        )
                                        .on_hover_text(original.clone());
                                    });
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [52.0, 14.0],
                                            egui::Label::new(
                                                RichText::new("NOW")
                                                    .size(7.5)
                                                    .strong()
                                                    .color(theme::FAINT),
                                            ),
                                        );
                                        ui.label(
                                            RichText::new(util::truncate(&now, 76))
                                                .font(egui::FontId::monospace(9.0))
                                                .color(if changed {
                                                    theme::YELLOW
                                                } else {
                                                    theme::GREEN
                                                }),
                                        )
                                        .on_hover_text(now.clone());
                                    });
                                }
                                if let Some(plan) = &entry.power_plan {
                                    ui.label(
                                        RichText::new(format!("Power plan GUID: {plan}"))
                                            .size(8.5)
                                            .color(theme::FAINT),
                                    );
                                }
                                ui.add_space(5.0);
                            }
                        });
                }
            });

            if view {
                self.expanded_snapshot = if expanded {
                    None
                } else {
                    Some(snapshot.id.clone())
                };
            }
            if restore {
                self.ask_restore_snapshot(&snapshot);
            }
            if export_snapshot {
                self.export_snapshot(&snapshot.id);
            }
            ui.add_space(8.0);
        }

        ui.add_space(6.0);

        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Files you can read at any time",
                "Plain JSON and a plain-text log — no proprietary format",
                |ui| {
                    if ui::small_ghost(ui, "Open folder").clicked() {
                        open_folder = true;
                    }
                },
            );
            ui.add_space(4.0);
            let files = [
                ("originals.json", "First captured value for every changed tweak"),
                ("snapshots/", "One timestamped backup per apply or revert"),
                ("profiles.json", "Your custom profiles and the active profile id"),
                ("clutchcore.log", "Everything the app did, with timestamps"),
                ("system-report-*.txt", "Exported hardware and network report"),
            ];
            for (name, description) in files {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [150.0, 16.0],
                        egui::Label::new(
                            RichText::new(name)
                                .font(egui::FontId::monospace(9.5))
                                .color(theme::TEXT),
                        ),
                    );
                    ui.label(
                        RichText::new(description)
                            .size(9.5)
                            .color(theme::MUTED),
                    );
                });
            }
            ui.add_space(5.0);
            ui.label(
                RichText::new(self.data_dir.display().to_string())
                    .font(egui::FontId::monospace(9.0))
                    .color(theme::FAINT),
            );
        });

        if restore_all {
            self.ask_restore_all();
        }
        if export {
            self.export_originals();
        }
        if open_folder {
            util::open_path(&self.data_dir);
        }
    }
}
