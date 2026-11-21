use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, TextStyle,
};

pub const BG: Color32 = Color32::from_rgb(8, 10, 14);
pub const SIDEBAR: Color32 = Color32::from_rgb(12, 15, 20);
pub const PANEL: Color32 = Color32::from_rgb(16, 20, 26);
pub const CARD: Color32 = Color32::from_rgb(19, 23, 31);
pub const CARD_HOVER: Color32 = Color32::from_rgb(26, 31, 41);
pub const CHIP: Color32 = Color32::from_rgb(27, 32, 42);
pub const INPUT_BG: Color32 = Color32::from_rgb(11, 14, 19);
pub const BORDER: Color32 = Color32::from_rgb(33, 39, 50);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(26, 31, 40);
pub const TEXT: Color32 = Color32::from_rgb(233, 237, 243);
pub const MUTED: Color32 = Color32::from_rgb(142, 152, 168);
pub const FAINT: Color32 = Color32::from_rgb(94, 104, 120);

pub const ACCENT: Color32 = Color32::from_rgb(255, 112, 28);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(122, 62, 24);
pub const ACCENT_TINT: Color32 = Color32::from_rgb(44, 27, 18);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(24, 13, 5);

pub const GREEN: Color32 = Color32::from_rgb(76, 214, 148);
pub const GREEN_TINT: Color32 = Color32::from_rgb(19, 40, 30);
pub const YELLOW: Color32 = Color32::from_rgb(247, 191, 74);
pub const YELLOW_TINT: Color32 = Color32::from_rgb(44, 35, 16);
pub const RED: Color32 = Color32::from_rgb(255, 99, 99);
pub const RED_TINT: Color32 = Color32::from_rgb(46, 22, 24);
pub const BLUE: Color32 = Color32::from_rgb(104, 168, 255);
pub const BLUE_TINT: Color32 = Color32::from_rgb(19, 31, 48);
pub const PURPLE: Color32 = Color32::from_rgb(177, 149, 255);
pub const RING_BG: Color32 = Color32::from_rgb(33, 39, 50);

pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| -> u8 {
        (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(
        lerp(from.r(), to.r()),
        lerp(from.g(), to.g()),
        lerp(from.b(), to.b()),
        lerp(from.a(), to.a()),
    )
}

pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    install_style(ctx);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    if let Ok(data) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        fonts
            .font_data
            .insert("segoe-ui".to_owned(), FontData::from_owned(data));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe-ui".to_owned());
        }
    }

    if let Ok(data) = std::fs::read(r"C:\Windows\Fonts\consola.ttf") {
        fonts
            .font_data
            .insert("consolas".to_owned(), FontData::from_owned(data));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
            list.insert(0, "consolas".to_owned());
        }
    }

    ctx.set_fonts(fonts);
}

fn install_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = INPUT_BG;
    visuals.faint_bg_color = CARD;
    visuals.override_text_color = Some(TEXT);
    visuals.window_rounding = Rounding::same(12.0);
    visuals.menu_rounding = Rounding::same(8.0);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.weak_bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.rounding = Rounding::same(8.0);

    visuals.widgets.inactive.bg_fill = CHIP;
    visuals.widgets.inactive.weak_bg_fill = CARD_HOVER;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.rounding = Rounding::same(7.0);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 45, 56);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(32, 38, 48);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(66, 76, 92));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.hovered.rounding = Rounding::same(7.0);

    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.weak_bg_fill = ACCENT_DIM;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.active.rounding = Rounding::same(7.0);

    visuals.widgets.open.bg_fill = CHIP;
    visuals.widgets.open.weak_bg_fill = CARD_HOVER;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.open.rounding = Rounding::same(7.0);

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(11.0, 5.0);
    style.spacing.interact_size = egui::vec2(40.0, 26.0);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Body, FontId::proportional(13.5)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Heading, FontId::proportional(22.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();

    ctx.set_style(style);
}
