use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, RichText, Rounding, Sense, Stroke, Vec2,
};

use crate::app::{App, ProfileDraft, Tab};
use crate::model::{Category, Risk, TweakState};
use crate::presets;
use crate::profiles;
use crate::theme;
use crate::tweaks;
use crate::ui;
use crate::util;

impl App {
    pub(crate) fn view_profiles(&mut self, ui: &mut egui::Ui) {
        if self.editing.is_some() {
            self.view_profile_editor(ui);
            return;
        }

        let active = profiles::active_name(&self.profiles_store);
        let mut select: Option<String> = None;

        ui::section_header_row(
            ui,
            "Game presets",
            "A curated Windows set per game — applied in one click, backed up first",
            |ui| match &active {
                Some(name) => ui::badge(
                    ui,
                    format!("ACTIVE · {}", name.to_uppercase()),
                    theme::GREEN,
                ),
                None => ui::badge(ui, "NO ACTIVE PROFILE", theme::MUTED),
            },
        );
        ui.add_space(8.0);

        for pair in presets::all().chunks(2) {
            ui.columns(2, |cols| {
                for (index, preset) in pair.iter().enumerate() {
                    if index >= cols.len() {
                        break;
                    }
                    let detected = self
                        .report
                        .detected_games
                        .iter()
                        .find(|game| game.preset_id == preset.id);
                    let selected = self.selected_preset.as_deref() == Some(preset.id);
                    if preset_card(
                        &mut cols[index],
                        preset,
                        detected.map(|game| game.path.as_str()),
                        selected,
                    ) {
                        select = Some(preset.id.to_string());
                    }
                }
            });
            ui.add_space(8.0);
        }

        if let Some(id) = select {
            self.selected_preset = Some(id);
        }

        if let Some(id) = self.selected_preset.clone() {
            if let Some(preset) = presets::by_id(&id) {
                self.preset_detail(ui, preset);
            }
        }

        ui.add_space(18.0);
        ui::section_header_row(
            ui,
            "System presets",
            "No game required — these target the whole machine",
            |_ui| {},
        );
        ui.add_space(6.0);

        for system_preset in presets::system_all() {
            let profile = profiles::from_system(system_preset);
            let mut apply = false;
            let applied = profile
                .tweak_ids
                .iter()
                .filter(|id| {
                    self.statuses
                        .get(*id)
                        .map(|status| status.state == TweakState::Applied)
                        .unwrap_or(false)
                })
                .count();
            ui::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(system_preset.name)
                                    .size(13.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            if applied == profile.tweak_ids.len() && applied > 0 {
                                ui::badge(ui, "ACTIVE", theme::GREEN);
                            }
                        });
                        ui.label(
                            RichText::new(system_preset.tagline)
                                .size(10.0)
                                .color(theme::MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::ghost_button(ui, "Apply").clicked() {
                            apply = true;
                        }
                        ui::badge(
                            ui,
                            format!("{} TWEAKS", profile.tweak_ids.len()),
                            theme::BLUE,
                        );
                    });
                });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(system_preset.description)
                        .size(10.0)
                        .color(theme::MUTED),
                );
                let admin_count = profile
                    .tweak_ids
                    .iter()
                    .filter_map(|id| tweaks::find(id))
                    .filter(|spec| spec.requires_admin)
                    .count();
                let risky = tweaks::risky_in(&profile.tweak_ids);
                if admin_count > 0 || !risky.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if admin_count > 0 {
                            ui::badge(ui, "ADMIN", theme::YELLOW);
                        }
                        if !risky.is_empty() {
                            ui::badge(
                                ui,
                                format!("{} RISK CHANGES", risky.len()),
                                theme::YELLOW,
                            );
                        }
                        ui::badge(
                            ui,
                            format!("{applied}/{} ACTIVE", profile.tweak_ids.len()),
                            theme::MUTED,
                        );
                    });
                }
            });
            if apply {
                self.ask_apply_profile(&profile);
            }
            ui.add_space(8.0);
        }

        ui.add_space(18.0);

        let customs = self.profiles_store.profiles.clone();
        let active_id = self.profiles_store.active.clone();
        let mut new_profile = false;

        ui::section_header_row(
            ui,
            "Custom profiles",
            "Your own combination of tweaks, saved to profiles.json",
            |ui| {
                if ui::accent_button(ui, "New profile").clicked() {
                    new_profile = true;
                }
            },
        );
        ui.add_space(6.0);

        if customs.is_empty() {
            ui::card(ui, |ui| {
                ui::empty_state(
                    ui,
                    "No custom profiles yet",
                    "Build one from any combination of the 16 tweaks, or clone a game preset and adjust it.",
                );
                if ui::ghost_button(ui, "Create a profile").clicked() {
                    new_profile = true;
                }
            });
        }

        for profile in customs {
            let is_active = active_id.as_deref() == Some(profile.id.as_str());
            let applied = profile
                .tweak_ids
                .iter()
                .filter(|id| {
                    self.statuses
                        .get(*id)
                        .map(|status| status.state == TweakState::Applied)
                        .unwrap_or(false)
                })
                .count();
            let mut apply = false;
            let mut edit = false;
            let mut delete = false;

            ui::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&profile.name)
                                    .size(13.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            if is_active {
                                ui::badge(ui, "ACTIVE", theme::GREEN);
                            }
                        });
                        if !profile.description.is_empty() {
                            ui.label(
                                RichText::new(&profile.description)
                                    .size(10.0)
                                    .color(theme::MUTED),
                            );
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} tweaks · {}/{} active · built {}",
                                profile.tweak_ids.len(),
                                applied,
                                profile.tweak_ids.len(),
                                util::fmt_datetime(profile.created_unix)
                            ))
                            .size(9.0)
                            .color(theme::FAINT),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui::small_ghost(ui, "Delete").clicked() {
                            delete = true;
                        }
                        if ui::small_ghost(ui, "Edit").clicked() {
                            edit = true;
                        }
                        if ui::accent_button(ui, "Apply").clicked() {
                            apply = true;
                        }
                    });
                });
            });

            if apply {
                self.ask_apply_profile(&profile);
            }
            if edit {
                self.editing = Some(ProfileDraft::from_profile(&profile));
            }
            if delete {
                self.ask_delete_profile(&profile);
            }
            ui.add_space(8.0);
        }

        if new_profile {
            self.editing = Some(ProfileDraft::new());
        }
    }

    fn preset_detail(&mut self, ui: &mut egui::Ui, preset: &presets::GamePreset) {
        let profile = profiles::from_game(preset);
        let detected = self
            .report
            .detected_games
            .iter()
            .find(|game| game.preset_id == preset.id)
            .cloned();
        let applied = profile
            .tweak_ids
            .iter()
            .filter(|id| {
                self.statuses
                    .get(*id)
                    .map(|status| status.state == TweakState::Applied)
                    .unwrap_or(false)
            })
            .count();
        let mut apply = false;
        let mut clone = false;
        let mut close = false;
        let mut test_routes = false;

        ui.add_space(10.0);
        ui::card(ui, |ui| {
            ui.horizontal(|ui| {
                let accent = Color32::from_rgb(preset.accent.0, preset.accent.1, preset.accent.2);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    Rounding::same(9.0),
                    Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 38),
                );
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    preset.game.chars().next().unwrap_or('C').to_string(),
                    FontId::proportional(16.0),
                    accent,
                );
                ui.add_space(2.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(preset.game)
                                .size(17.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                        ui::badge(ui, preset.genre.to_uppercase(), theme::MUTED);
                        if detected.is_some() {
                            ui::badge(ui, "DETECTED", theme::GREEN);
                        }
                    });
                    ui.label(
                        RichText::new(preset.tagline)
                            .size(10.5)
                            .color(theme::MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::small_ghost(ui, "Close").clicked() {
                        close = true;
                    }
                    ui::badge(
                        ui,
                        format!("{applied}/{} ACTIVE", profile.tweak_ids.len()),
                        if applied == profile.tweak_ids.len() {
                            theme::GREEN
                        } else {
                            theme::MUTED
                        },
                    );
                });
            });

            if let Some(game) = &detected {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Installed at {}", util::truncate(&game.path, 86)))
                        .font(FontId::monospace(9.0))
                        .color(theme::FAINT),
                )
                .on_hover_text(&game.path);
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui::accent_button(ui, "Apply preset").clicked() {
                    apply = true;
                }
                if ui::ghost_button(ui, "Clone as custom").clicked() {
                    clone = true;
                }
                if ui::ghost_button(ui, "Test this game's routes").clicked() {
                    test_routes = true;
                }
                ui.label(
                    RichText::new(if profile.tweak_ids.iter().any(|id| {
                        tweaks::find(id)
                            .map(|spec| spec.requires_admin)
                            .unwrap_or(false)
                    }) {
                        "Needs administrator for some items"
                    } else {
                        "No elevation needed"
                    })
                    .size(9.0)
                    .color(theme::FAINT),
                );
            });

            ui.add_space(10.0);
            ui.columns(2, |cols| {
                cols[0].label(
                    RichText::new("LAUNCH OPTIONS")
                        .size(8.5)
                        .strong()
                        .color(theme::FAINT),
                );
                ui::code(&mut cols[0], preset.launch_options);
                cols[0].horizontal(|ui| {
                    ui::copy_button(ui, preset.launch_options);
                });
                cols[0].add_space(6.0);
                cols[0].label(
                    RichText::new("CONFIG FILE")
                        .size(8.5)
                        .strong()
                        .color(theme::FAINT),
                );
                ui::code(&mut cols[0], preset.config_path);
                cols[0].horizontal(|ui| {
                    ui::copy_button(ui, preset.config_path);
                });

                cols[1].label(
                    RichText::new("IN-GAME CHECKLIST")
                        .size(8.5)
                        .strong()
                        .color(theme::FAINT),
                );
                for tip in preset.in_game {
                    cols[1].label(
                        RichText::new(format!("•  {tip}"))
                            .size(9.5)
                            .color(theme::MUTED),
                    );
                    cols[1].add_space(1.0);
                }
            });

            ui.add_space(8.0);
            ui.label(
                RichText::new("INCLUDED WINDOWS TWEAKS")
                    .size(8.5)
                    .strong()
                    .color(theme::FAINT),
            );
            let half = (profile.tweak_ids.len() + 1) / 2;
            ui.columns(2, |cols| {
                for (index, id) in profile.tweak_ids.iter().enumerate() {
                    let column = if index < half { 0 } else { 1 };
                    if let Some(spec) = tweaks::find(id) {
                        let is_applied = self
                            .statuses
                            .get(id)
                            .map(|status| status.state == TweakState::Applied)
                            .unwrap_or(false);
                        cols[column].horizontal(|ui| {
                            ui::dot(
                                ui,
                                if is_applied { theme::GREEN } else { theme::FAINT },
                                8.0,
                            );
                            ui.label(
                                RichText::new(util::truncate(spec.name, 42))
                                    .size(9.0)
                                    .color(if is_applied {
                                        theme::MUTED
                                    } else {
                                        theme::TEXT
                                    }),
                            );
                        });
                    }
                }
            });

            if !preset.notes.is_empty() {
                ui.add_space(6.0);
                for note in preset.notes {
                    ui.label(
                        RichText::new(format!("Note: {note}"))
                            .size(9.0)
                            .color(theme::FAINT),
                    );
                }
            }
        });

        if apply {
            self.ask_apply_profile(&profile);
        }
        if clone {
            self.editing = Some(ProfileDraft {
                id: None,
                name: format!("{} (custom)", preset.game),
                description: format!("Based on the {} preset.", preset.game),
                selected: preset.tweak_ids.iter().map(|id| id.to_string()).collect(),
                error: None,
            });
        }
        if test_routes {
            self.probe_provider = Some(preset.provider.to_string());
            self.tab = Tab::Network;
            self.start_probe_all(Some(preset.provider.to_string()));
        }
        if close {
            self.selected_preset = None;
        }
    }

    fn view_profile_editor(&mut self, ui: &mut egui::Ui) {
        let Some(mut draft) = self.editing.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;

        ui::card(ui, |ui| {
            ui::section_header_row(
                ui,
                if draft.id.is_some() {
                    "Edit custom profile"
                } else {
                    "New custom profile"
                },
                "Pick exactly which tweaks this profile applies",
                |_ui| {},
            );
            ui.add_space(8.0);

            ui.label(
                RichText::new("NAME")
                    .size(8.5)
                    .strong()
                    .color(theme::FAINT),
            );
            ui.add(
                egui::TextEdit::singleline(&mut draft.name)
                    .hint_text("e.g. Nightly ranked / low-ping mode")
                    .desired_width(380.0),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("DESCRIPTION")
                    .size(8.5)
                    .strong()
                    .color(theme::FAINT),
            );
            ui.add(
                egui::TextEdit::singleline(&mut draft.description)
                    .hint_text("Optional — what is this profile for?")
                    .desired_width(520.0),
            );

            let selected_specs: Vec<&'static crate::model::TweakSpec> = draft
                .selected
                .iter()
                .filter_map(|id| tweaks::find(id))
                .collect();
            let medium = selected_specs
                .iter()
                .filter(|spec| spec.risk == Risk::Medium)
                .count();
            let admin = selected_specs
                .iter()
                .filter(|spec| spec.requires_admin)
                .count();
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui::badge(
                    ui,
                    format!("{} TWEAKS SELECTED", selected_specs.len()),
                    theme::BLUE,
                );
                if medium > 0 {
                    ui::badge(ui, format!("{medium} MEDIUM RISK"), theme::YELLOW);
                }
                if admin > 0 {
                    ui::badge(ui, format!("{admin} NEED ADMIN"), theme::YELLOW);
                }
                if selected_specs.is_empty() {
                    ui::badge(ui, "NOTHING SELECTED", theme::RED);
                }
            });

            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for category in Category::ALL {
                        ui.label(
                            RichText::new(category.label().to_uppercase())
                                .size(8.5)
                                .strong()
                                .color(ui::category_color(category)),
                        );
                        for spec in tweaks::specs()
                            .iter()
                            .filter(|spec| spec.category == category)
                        {
                            let mut checked = draft.selected.contains(spec.id);
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut checked, spec.name).changed() {
                                    if checked {
                                        draft.selected.insert(spec.id.to_string());
                                    } else {
                                        draft.selected.remove(spec.id);
                                    }
                                }
                                if spec.requires_admin {
                                    ui::badge(ui, "ADMIN", theme::YELLOW);
                                }
                                if spec.requires_reboot {
                                    ui::badge(ui, "REBOOT", theme::BLUE);
                                }
                                if spec.risk != Risk::Low {
                                    ui::badge(
                                        ui,
                                        spec.risk.label().to_uppercase(),
                                        ui::risk_color(spec.risk),
                                    );
                                }
                            });
                        }
                        ui.add_space(6.0);
                    }
                });

            if let Some(error) = &draft.error {
                ui.add_space(4.0);
                ui.label(RichText::new(error).size(10.5).color(theme::RED));
            }

            ui::separator(ui);
            ui.horizontal(|ui| {
                if ui::accent_button(ui, "Save profile").clicked() {
                    save = true;
                }
                if ui::ghost_button(ui, "Cancel").clicked() {
                    cancel = true;
                }
                ui.label(
                    RichText::new("Saved to profiles.json — applying it is a separate click.")
                        .size(9.0)
                        .color(theme::FAINT),
                );
            });
        });

        if cancel {
            return;
        }
        if save {
            self.finish_editor(draft);
        } else {
            self.editing = Some(draft);
        }
    }

    fn finish_editor(&mut self, mut draft: ProfileDraft) {
        if draft.name.trim().is_empty() {
            draft.error = Some("Give the profile a name.".into());
            self.editing = Some(draft);
            return;
        }
        if draft.selected.is_empty() {
            draft.error = Some("Select at least one tweak.".into());
            self.editing = Some(draft);
            return;
        }
        let ids: Vec<String> = draft.selected.iter().cloned().collect();
        match profiles::upsert_custom(
            &mut self.profiles_store,
            draft.id.as_deref(),
            &draft.name,
            &draft.description,
            ids,
        ) {
            Ok(profile) => {
                self.editing = None;
                self.toast(
                    crate::logging::Level::Success,
                    format!("Saved '{}'. Apply it whenever you are ready.", profile.name),
                );
            }
            Err(error) => {
                draft.error = Some(error);
                self.editing = Some(draft);
            }
        }
    }
}

fn preset_card(
    ui: &mut egui::Ui,
    preset: &presets::GamePreset,
    detected: Option<&str>,
    selected: bool,
) -> bool {
    let mut open = false;
    let accent = Color32::from_rgb(preset.accent.0, preset.accent.1, preset.accent.2);
    egui::Frame::none()
        .fill(if selected {
            theme::ACCENT_TINT
        } else {
            theme::CARD
        })
        .stroke(Stroke::new(
            1.0,
            if selected {
                theme::ACCENT_DIM
            } else {
                theme::BORDER
            },
        ))
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(82.0);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    Rounding::same(8.0),
                    Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 38),
                );
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    preset.game.chars().next().unwrap_or('C').to_string(),
                    FontId::proportional(14.0),
                    accent,
                );
                ui.add_space(2.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(preset.game)
                            .size(12.5)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(
                        RichText::new(preset.tagline)
                            .size(9.5)
                            .color(theme::MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui::badge(ui, preset.genre.to_uppercase(), theme::MUTED);
                });
            });
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui::badge(
                    ui,
                    format!("{} TWEAKS", preset.tweak_ids.len()),
                    theme::BLUE,
                );
                if detected.is_some() {
                    ui::badge(ui, "DETECTED", theme::GREEN);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui::small_ghost(ui, if selected { "Close" } else { "View preset" })
                        .clicked()
                    {
                        open = true;
                    }
                });
            });
            if let Some(path) = detected {
                ui.label(
                    RichText::new(util::truncate(path, 56))
                        .size(8.5)
                        .color(theme::FAINT),
                )
                .on_hover_text(path);
            }
        });
    open
}

