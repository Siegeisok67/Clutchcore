use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Rect, Response, RichText, Rounding, Sense,
    Stroke, Ui, Vec2,
};

use crate::analysis::Severity;
use crate::model::{Category, Risk, TweakState};
use crate::theme;
use crate::util;

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_fill(ui, theme::CARD, add)
}

pub fn card_fill<R>(ui: &mut Ui, fill: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let inner = egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, add);

    // A one pixel highlight along the top edge keeps cards feeling layered
    // instead of flat against the background.
    let rect = inner.response.rect;
    let line = Rect::from_min_max(
        egui::pos2(rect.left() + 14.0, rect.top() + 1.0),
        egui::pos2(rect.right() - 14.0, rect.top() + 2.0),
    );
    if line.width() > 1.0 {
        ui.painter().rect_filled(
            line,
            Rounding::same(1.0),
            theme::with_alpha(Color32::WHITE, 12),
        );
    }

    inner.inner
}

pub fn section(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(16.0).strong().color(theme::TEXT));
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).size(11.5).color(theme::MUTED));
    }
}

pub fn badge(ui: &mut Ui, text: impl Into<String>, color: Color32) {
    let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 30);
    egui::Frame::none()
        .fill(bg)
        .rounding(Rounding::same(5.0))
        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text.into())
                    .size(9.5)
                    .strong()
                    .color(color),
            );
        });
}

pub fn chip(ui: &mut Ui, text: impl Into<String>, selected: bool) -> Response {
    let (fill, fg, stroke) = if selected {
        (theme::ACCENT_TINT, theme::ACCENT, Stroke::new(1.0, theme::ACCENT_DIM))
    } else {
        (theme::CHIP, theme::MUTED, Stroke::new(1.0, theme::BORDER))
    };
    ui.add(
        egui::Button::new(RichText::new(text.into()).size(11.0).color(fg))
            .fill(fill)
            .stroke(stroke)
            .rounding(Rounding::same(999.0))
            .min_size(Vec2::new(0.0, 25.0)),
    )
}

pub fn accent_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    button(ui, text, theme::ACCENT, theme::ACCENT_TEXT, false, 30.0)
}

pub fn ghost_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    button(ui, text, theme::CHIP, theme::TEXT, true, 30.0)
}

pub fn danger_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    button(ui, text, theme::RED_TINT, theme::RED, true, 30.0)
}

pub fn small_ghost(ui: &mut Ui, text: impl Into<String>) -> Response {
    button(ui, text, Color32::TRANSPARENT, theme::MUTED, false, 24.0)
}

pub fn button(
    ui: &mut Ui,
    text: impl Into<String>,
    fill: Color32,
    fg: Color32,
    border: bool,
    min_height: f32,
) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text.into()).size(12.0).strong().color(fg))
            .fill(fill)
            .stroke(if border {
                Stroke::new(1.0, theme::BORDER)
            } else {
                Stroke::NONE
            })
            .rounding(Rounding::same(7.0))
            .min_size(Vec2::new(0.0, min_height)),
    )
}

pub fn stat_tile(ui: &mut Ui, label: &str, value: &str, hint: &str) {
    egui::Frame::none()
        .fill(theme::CARD)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .rounding(Rounding::same(11.0))
        .inner_margin(egui::Margin::same(13.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(58.0);
            let (dash, _) = ui.allocate_exact_size(Vec2::new(16.0, 2.0), Sense::hover());
            ui.painter()
                .rect_filled(dash, Rounding::same(1.0), theme::ACCENT);
            ui.add_space(5.0);
            ui.label(
                RichText::new(label.to_uppercase())
                    .size(9.0)
                    .strong()
                    .color(theme::MUTED),
            );
            ui.add_space(2.0);
            let resp = ui.label(
                RichText::new(value)
                    .size(14.5)
                    .strong()
                    .color(theme::TEXT),
            );
            if !hint.is_empty() {
                ui.add_space(1.0);
                ui.label(RichText::new(hint).size(10.0).color(theme::FAINT));
            }
            resp.on_hover_text(value);
        });
}

pub fn key_value(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [96.0, 17.0],
            egui::Label::new(RichText::new(key).size(10.5).color(theme::MUTED)),
        );
        ui.label(
            RichText::new(util::truncate(value, 62))
                .size(11.5)
                .color(theme::TEXT),
        );
    });
}

pub fn separator(ui: &mut Ui) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(0.5), theme::BORDER_SOFT);
    ui.add_space(4.0);
}

pub fn dot(ui: &mut Ui, color: Color32, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), size * 0.38, color);
    response
}

pub fn score_ring(ui: &mut Ui, score: u32, size: f32, color: Color32, caption: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let animated = ui
        .ctx()
        .animate_value_with_time(ui.id().with("score-ring"), score as f32, 0.7);
    let center = rect.center();
    let radius = size * 0.39;
    let stroke = Stroke::new(size * 0.062, theme::RING_BG);
    ui.painter().circle_stroke(
        center,
        radius + 4.0,
        Stroke::new(6.0, theme::with_alpha(color, 14)),
    );
    ui.painter().circle_stroke(center, radius, stroke);

    if animated > 0.5 {
        let steps = 96;
        let sweep = std::f32::consts::TAU * (animated / 100.0);
        let mut points = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let angle = -std::f32::consts::FRAC_PI_2 + sweep * t;
            points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
        }
        ui.painter()
            .add(egui::Shape::line(points, Stroke::new(size * 0.062, color)));
    }

    ui.painter().text(
        center + Vec2::new(0.0, -size * 0.045),
        Align2::CENTER_CENTER,
        format!("{animated:.0}"),
        FontId::proportional(size * 0.26),
        theme::TEXT,
    );
    ui.painter().text(
        center + Vec2::new(0.0, size * 0.16),
        Align2::CENTER_CENTER,
        caption.to_uppercase(),
        FontId::proportional(size * 0.062),
        theme::MUTED,
    );
}

pub fn score_bar(ui: &mut Ui, label: &str, score: u32, color: Color32) {
    let animated = ui.ctx().animate_value_with_time(
        ui.id().with(("score-bar", label)),
        score as f32,
        0.7,
    );
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(9.5)
                .strong()
                .color(theme::MUTED),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(score.to_string())
                    .size(10.5)
                    .strong()
                    .color(color),
            );
        });
    });
    ui.add_space(1.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 5.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::same(2.5), theme::RING_BG);
    let width = (rect.width() * (animated / 100.0)).max(2.0);
    let fill = Rect::from_min_size(rect.min, Vec2::new(width, rect.height()));
    ui.painter().rect_filled(fill, Rounding::same(2.5), color);
    if width > 6.0 {
        let head = Rect::from_min_size(
            egui::pos2(fill.right() - 3.0, fill.top()),
            Vec2::new(3.0, fill.height()),
        );
        ui.painter()
            .rect_filled(head, Rounding::same(1.5), theme::with_alpha(Color32::WHITE, 90));
    }
}

pub fn sparkline(ui: &mut Ui, samples: &[f64], width: f32, height: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if samples.len() < 2 {
        return;
    }
    let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (max - min).max(0.001);
    let mut points = Vec::with_capacity(samples.len());
    for (index, value) in samples.iter().enumerate() {
        let x = rect.left() + rect.width() * (index as f32 / (samples.len() - 1) as f32);
        let ratio = ((value - min) / span) as f32;
        let y = rect.bottom() - rect.height() * ratio;
        points.push(egui::pos2(x, y.clamp(rect.top(), rect.bottom())));
    }

    // Soft area under the line so the shape reads at a glance.
    let wash = theme::with_alpha(color, 26);
    let mut mesh = egui::Mesh::default();
    for pair in points.windows(2) {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(pair[0], wash);
        mesh.colored_vertex(egui::pos2(pair[0].x, rect.bottom()), wash);
        mesh.colored_vertex(pair[1], wash);
        mesh.colored_vertex(egui::pos2(pair[1].x, rect.bottom()), wash);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(1.5, color)));
}

pub fn toggle_switch(ui: &mut Ui, on: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(38.0, 21.0), Sense::click());
    let t = ui.ctx().animate_bool(response.id, on);
    let bg = if on { theme::ACCENT } else { theme::CHIP };
    ui.painter()
        .rect_filled(rect, Rounding::same(rect.height() / 2.0), bg);
    let x = egui::lerp((rect.left() + 10.5)..=(rect.right() - 10.5), t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), 8.0, Color32::WHITE);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn suggestion_row(
    ui: &mut Ui,
    color: Color32,
    title: &str,
    detail: &str,
    action_label: &str,
    accent: bool,
) -> bool {
    let mut clicked = false;
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.set_min_height(46.0);
        ui.add_space(2.0);
        dot(ui, color, 12.0);
        ui.allocate_ui_with_layout(
            Vec2::new((width - 172.0).max(160.0), 42.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    RichText::new(title)
                        .size(12.5)
                        .strong()
                        .color(theme::TEXT),
                );
                let response = ui.label(
                    RichText::new(util::truncate(detail, 94))
                        .size(10.5)
                        .color(theme::MUTED),
                );
                response.on_hover_text(detail);
            },
        );
        if !action_label.is_empty() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let button = egui::Button::new(
                    RichText::new(action_label)
                        .size(11.0)
                        .strong()
                        .color(if accent { theme::ACCENT_TEXT } else { theme::TEXT }),
                )
                .fill(if accent { theme::ACCENT } else { theme::CHIP })
                .stroke(if accent {
                    Stroke::NONE
                } else {
                    Stroke::new(1.0, theme::BORDER)
                })
                .rounding(Rounding::same(7.0))
                .min_size(Vec2::new(96.0, 27.0));
                if ui.add(button).clicked() {
                    clicked = true;
                }
            });
        }
    });
    clicked
}

pub fn empty_state(ui: &mut Ui, title: &str, body: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(13.0).strong().color(theme::TEXT));
    ui.label(RichText::new(body).size(11.5).color(theme::MUTED));
    ui.add_space(6.0);
}

pub fn code(ui: &mut Ui, text: &str) {
    egui::Frame::none()
        .fill(theme::INPUT_BG)
        .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
        .rounding(Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(9.0, 6.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::monospace(11.0))
                    .color(theme::TEXT),
            );
        });
}

pub fn copy_button(ui: &mut Ui, text: &str) -> Response {
    let response = small_ghost(ui, "Copy");
    if response.clicked() {
        ui.output_mut(|output| output.copied_text = text.to_string());
    }
    response
}

pub fn score_color(score: u32) -> Color32 {
    if score >= 85 {
        theme::GREEN
    } else if score >= 65 {
        theme::ACCENT
    } else if score >= 45 {
        theme::YELLOW
    } else {
        theme::RED
    }
}

pub fn ping_color(ms: f64) -> Color32 {
    if ms <= 20.0 {
        theme::GREEN
    } else if ms <= 40.0 {
        Color32::from_rgb(160, 220, 110)
    } else if ms <= 70.0 {
        theme::YELLOW
    } else {
        theme::RED
    }
}

pub fn jitter_color(ms: f64) -> Color32 {
    if ms <= 4.0 {
        theme::GREEN
    } else if ms <= 10.0 {
        theme::YELLOW
    } else {
        theme::RED
    }
}

pub fn state_color(state: TweakState) -> Color32 {
    match state {
        TweakState::Applied => theme::GREEN,
        TweakState::Partial => theme::YELLOW,
        TweakState::NotApplied => theme::MUTED,
        TweakState::Unknown => theme::RED,
    }
}

pub fn risk_color(risk: Risk) -> Color32 {
    match risk {
        Risk::Low => theme::GREEN,
        Risk::Medium => theme::YELLOW,
        Risk::High => theme::RED,
    }
}

pub fn category_color(category: Category) -> Color32 {
    match category {
        Category::Input => theme::BLUE,
        Category::Latency => theme::ACCENT,
        Category::Network => theme::PURPLE,
        Category::Visuals => Color32::from_rgb(236, 144, 220),
        Category::System => theme::GREEN,
    }
}

pub fn severity_color(severity: Severity) -> Color32 {
    match severity {
        Severity::Critical => theme::RED,
        Severity::Recommended => theme::ACCENT,
        Severity::Optional => theme::BLUE,
        Severity::Note => theme::MUTED,
    }
}

pub fn admin_badge(ui: &mut Ui, admin: bool) {
    if admin {
        badge(ui, "ADMIN", theme::GREEN);
    } else {
        badge(ui, "STANDARD USER", theme::MUTED);
    }
}

pub fn section_header_row(ui: &mut Ui, title: &str, subtitle: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let (bar, _) = ui.allocate_exact_size(Vec2::new(3.0, 26.0), Sense::hover());
        ui.painter()
            .rect_filled(bar, Rounding::same(1.5), theme::ACCENT);
        ui.add_space(3.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(title).size(15.0).strong().color(theme::TEXT));
            if !subtitle.is_empty() {
                ui.label(RichText::new(subtitle).size(10.5).color(theme::MUTED));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}
