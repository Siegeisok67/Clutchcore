use eframe::egui::{self, Align, Color32, Layout, RichText, Vec2};

use crate::app::App;
use crate::model::{Category, Risk, TweakSpec, TweakState, TweakStatus};
use crate::theme;
use crate::tweaks;
use crate::ui;
use crate::util;

enum CardAction {
    Toggle,
    Details,
}

impl App {
    pub(crate) fn view_tweaks(&mut self, ui: &mut egui::Ui) {
        let applied = self
            .statuses
            .values()
            .filter(|s| s.state == TweakState::Applied)
            .count();
        let total = tweaks::specs().len().max(1);
        let mut apply_safe = false;
        let mut restore_all = false;

        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                "Tweak library",
                &format!("{applied} of {total} tweaks active on this machine"),
                |ui| {
                    if ui::accent_button(ui, "Apply safe set").clicked() {
                        apply_safe = true;
                    }
                    if ui::ghost_button(ui, "Restore originals").clicked() {
                        restore_all = true;
                    }
                },
            );
            ui.add_space(4.0);
            ui::score_bar(
                ui,
                "ACTIVE TWEAKS",
                ((applied as f32 / total as f32) * 100.0) as u32,
                theme::ACCENT,
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Every change below is captured first. Toggle a setting off to restore its original value, not a generic default.",
                )
                .size(10.0)
                .color(theme::MUTED),
            );
        });

        ui.add_space(12.0);

        let mut filter_change: Option<Option<Category>> = None;
        ui.horizontal_wrapped(|ui| {
            if ui::chip(ui, "All", self.tweak_filter.is_none()).clicked() {
                filter_change = Some(None);
            }
            for category in Category::ALL {
                if ui::chip(ui, category.label(), self.tweak_filter == Some(category)).clicked() {
                    filter_change = Some(Some(category));
                }
            }
        });
        if let Some(change) = filter_change {
            self.tweak_filter = change;
        }

        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.tweak_search)
                    .hint_text("Search tweaks, effects or registry paths…")
                    .desired_width(330.0),
            );
            ui.add_space(4.0);
            let filter = self.tweak_filter;
            let search = self.tweak_search.to_lowercase();
            let visible = tweaks::specs()
                .iter()
                .filter(|spec| {
                    filter.map(|category| spec.category == category).unwrap_or(true)
                        && (search.is_empty()
                            || spec.name.to_lowercase().contains(&search)
                            || spec.summary.to_lowercase().contains(&search)
                            || spec.detail.to_lowercase().contains(&search)
                            || spec.id.contains(&search))
                })
                .count();
            ui.label(
                RichText::new(format!("{visible} shown"))
                    .size(10.0)
                    .color(theme::FAINT),
            );
        });

        ui.add_space(10.0);

        let filter = self.tweak_filter;
        let search = self.tweak_search.to_lowercase();
        let visible: Vec<&'static TweakSpec> = tweaks::specs()
            .iter()
            .filter(|spec| {
                filter.map(|category| spec.category == category).unwrap_or(true)
                    && (search.is_empty()
                        || spec.name.to_lowercase().contains(&search)
                        || spec.summary.to_lowercase().contains(&search)
                        || spec.detail.to_lowercase().contains(&search)
                        || spec.id.contains(&search))
            })
            .collect();

        if visible.is_empty() {
            ui::card(ui, |ui| {
                ui::empty_state(
                    ui,
                    "No tweaks match",
                    "Clear the search or switch category.",
                );
            });
        }

        for spec in visible {
            let status = self
                .statuses
                .get(spec.id)
                .cloned()
                .unwrap_or(TweakStatus {
                    state: TweakState::Unknown,
                    detail: "Not checked yet".into(),
                });
            let expanded = self.expanded_tweaks.contains(spec.id);
            match tweak_card(ui, spec, &status, expanded) {
                Some(CardAction::Toggle) => self.toggle_tweak(spec.id),
                Some(CardAction::Details) => {
                    if expanded {
                        self.expanded_tweaks.remove(spec.id);
                    } else {
                        self.expanded_tweaks.insert(spec.id.to_string());
                    }
                }
                None => {}
            }
            ui.add_space(8.0);
        }

        if apply_safe {
            let ids: Vec<String> = tweaks::specs()
                .iter()
                .filter(|spec| spec.risk == Risk::Low)
                .filter(|spec| {
                    self.statuses
                        .get(spec.id)
                        .map(|status| status.state != TweakState::Applied)
                        .unwrap_or(true)
                })
                .map(|spec| spec.id.to_string())
                .collect();
            if ids.is_empty() {
                self.toast(
                    crate::logging::Level::Info,
                    "Every low-risk tweak is already active.",
                );
            } else {
                self.apply_ids(ids, "Safe tweak set", None);
            }
        }
        if restore_all {
            self.ask_restore_all();
        }
    }
}

fn tweak_card(
    ui: &mut egui::Ui,
    spec: &'static TweakSpec,
    status: &TweakStatus,
    expanded: bool,
) -> Option<CardAction> {
    let mut action = None;
    let applied = status.state == TweakState::Applied;
    let border = if applied {
        Color32::from_rgb(30, 58, 44)
    } else if spec.risk == Risk::Medium {
        Color32::from_rgb(52, 45, 28)
    } else {
        theme::BORDER
    };

    egui::Frame::none()
        .fill(theme::CARD)
        .stroke(egui::Stroke::new(1.0, border))
        .rounding(egui::Rounding::same(12.0))
        .inner_margin(egui::Margin::same(15.0))
        .show(ui, |ui| {
            let width = ui.available_width();
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new((width - 175.0).max(180.0), 24.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.label(
                            RichText::new(util::truncate(spec.name, 56))
                                .size(13.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                    },
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::toggle_switch(ui, applied).clicked() {
                        action = Some(CardAction::Toggle);
                    }
                    ui.label(
                        RichText::new(status.state.label().to_uppercase())
                            .size(8.5)
                            .strong()
                            .color(ui::state_color(status.state)),
                    );
                });
            });
            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                ui::badge(
                    ui,
                    spec.category.label().to_uppercase(),
                    ui::category_color(spec.category),
                );
                ui::badge(ui, tweaks::impact_text(spec).to_uppercase(), theme::MUTED);
                if spec.requires_admin {
                    ui::badge(ui, "ADMIN", theme::YELLOW);
                }
                if spec.requires_reboot {
                    ui::badge(ui, "REBOOT", theme::BLUE);
                }
                if spec.risk != Risk::Low {
                    ui::badge(ui, spec.risk.label().to_uppercase(), ui::risk_color(spec.risk));
                }
            });
            ui.add_space(3.0);
            ui.label(
                RichText::new(spec.summary)
                    .size(10.5)
                    .color(theme::MUTED),
            );
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(util::truncate(&status.detail, 74))
                        .size(9.0)
                        .color(ui::state_color(status.state)),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::small_ghost(ui, if expanded { "Hide details" } else { "Details" })
                        .clicked()
                    {
                        action = Some(CardAction::Details);
                    }
                });
            });

            if expanded {
                ui::separator(ui);
                ui.label(
                    RichText::new(spec.detail)
                        .size(10.5)
                        .color(theme::TEXT),
                );
                ui.add_space(7.0);
                ui.label(
                    RichText::new("WHAT THIS CHANGES")
                        .size(8.5)
                        .strong()
                        .color(theme::FAINT),
                );
                for line in tweaks::describe(spec) {
                    ui.label(
                        RichText::new(util::truncate(&line, 108))
                            .font(egui::FontId::monospace(9.5))
                            .color(theme::MUTED),
                    )
                    .on_hover_text(line);
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!("Revert: {}", spec.revert_hint))
                        .size(9.0)
                        .color(theme::FAINT),
                );
                ui.label(
                    RichText::new(format!(
                        "Risk: {} · {} · {}",
                        spec.risk.label(),
                        spec.risk.describes(),
                        if spec.requires_admin {
                            "Run as administrator to apply."
                        } else {
                            "No elevation needed."
                        }
                    ))
                    .size(9.0)
                    .color(ui::risk_color(spec.risk)),
                );
            }
        });

    action
}
