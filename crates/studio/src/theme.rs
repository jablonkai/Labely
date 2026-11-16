//! Visual design tokens and the egui style built from them (design/labely-desktop.figcraft).

use std::sync::Arc;

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, Visuals,
};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const PRIMARY: Color32 = rgb(0x4C6EF5);
pub const PRIMARY_DARK: Color32 = rgb(0x3B5BDB);
/// Primary tint: selected rows, active rail item.
pub const PRIMARY_TINT: Color32 = rgb(0xEDF2FF);
pub const PRIMARY_TINT2: Color32 = rgb(0xDBE4FF);
/// Window background behind cards.
pub const BG: Color32 = rgb(0xF5F6F8);
pub const SURFACE: Color32 = rgb(0xFFFFFF);
pub const BORDER: Color32 = rgb(0xE4E7EC);
pub const BORDER_SOFT: Color32 = rgb(0xEEF0F3);
pub const TEXT: Color32 = rgb(0x1B2130);
pub const TEXT2: Color32 = rgb(0x475066);
pub const MUTED: Color32 = rgb(0x8A93A6);
/// The canvas around the label.
pub const CANVAS: Color32 = rgb(0xE8EBF0);
pub const GREEN: Color32 = rgb(0x12B886);
pub const GREEN_TINT: Color32 = rgb(0xE6FCF5);
pub const AMBER: Color32 = rgb(0xF59F00);
pub const AMBER_TINT: Color32 = rgb(0xFFF4E0);
/// Title bar and tab strip.
pub const CHROME: Color32 = rgb(0xFBFBFC);
pub const RED: Color32 = rgb(0xFA5252);

pub const RADIUS: u8 = 8;
pub const RADIUS_LG: u8 = 12;

/// Font family for bold text (titles, buttons that need weight).
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Small uppercase section title, as in the inspector ("POSITION AND SIZE").
/// Shown in capitals; screen readers and tests get the text as written.
pub fn section(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add_space(6.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_uppercase(), bold(11.0), MUTED);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    let label = text.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &label));
    ui.painter().galley(rect.min, galley, MUTED);
    ui.add_space(2.0);
    resp
}

/// White card with a soft border.
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(RADIUS_LG))
        .inner_margin(Margin::same(14))
}

/// Primary (filled) button.
pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .font(bold(13.0))
            .color(Color32::WHITE),
    )
    .fill(PRIMARY)
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(RADIUS))
    .min_size(egui::vec2(0.0, 32.0))
}

fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    f.font_data.insert(
        "noto".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../render/fonts/NotoSans-Regular.ttf"
        ))),
    );
    f.font_data.insert(
        "noto-bold".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../render/fonts/NotoSans-Bold.ttf"
        ))),
    );
    let fallback = f
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut prop = vec!["noto".to_owned()];
    prop.extend(fallback.iter().cloned());
    let mut bold = vec!["noto-bold".to_owned()];
    bold.extend(fallback);
    f.families.insert(FontFamily::Proportional, prop);
    f.families.insert(FontFamily::Name("bold".into()), bold);
    f
}

fn visuals() -> Visuals {
    let mut v = Visuals::light();
    let r = CornerRadius::same(RADIUS);
    v.panel_fill = SURFACE;
    v.window_fill = SURFACE;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(RADIUS_LG);
    v.window_shadow = Shadow {
        offset: [0, 12],
        blur: 40,
        spread: 0,
        color: Color32::from_black_alpha(40),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(30),
    };
    v.menu_corner_radius = r;
    v.extreme_bg_color = SURFACE;
    v.faint_bg_color = BG;
    v.code_bg_color = BG;
    v.hyperlink_color = PRIMARY;
    v.selection.bg_fill = PRIMARY_TINT2;
    v.selection.stroke = Stroke::new(1.0, PRIMARY);
    v.override_text_color = None;
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = SURFACE;
    w.noninteractive.weak_bg_fill = SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SOFT);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = BORDER;
    w.inactive.weak_bg_fill = SURFACE;
    w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = PRIMARY_TINT2;
    w.hovered.weak_bg_fill = BG;
    w.hovered.bg_stroke = Stroke::new(1.0, PRIMARY_TINT2);
    w.hovered.fg_stroke = Stroke::new(1.5, TEXT);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = PRIMARY;
    w.active.weak_bg_fill = PRIMARY_TINT2;
    w.active.bg_stroke = Stroke::new(1.0, PRIMARY);
    w.active.fg_stroke = Stroke::new(1.5, PRIMARY_DARK);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
    w.open.bg_fill = PRIMARY_TINT;
    w.open.weak_bg_fill = PRIMARY_TINT;
    w.open.bg_stroke = Stroke::new(1.0, PRIMARY_TINT2);
    w.open.fg_stroke = Stroke::new(1.0, TEXT);
    w.open.corner_radius = r;
    v
}

/// Apply fonts, colours, spacing and text sizes.
pub fn apply(ctx: &Context) {
    ctx.set_fonts(fonts());
    ctx.set_theme(egui::Theme::Light);
    ctx.style_mut_of(egui::Theme::Light, |s| {
        s.visuals = visuals();
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.spacing.menu_margin = Margin::same(6);
        s.spacing.window_margin = Margin::same(18);
        s.spacing.combo_width = 120.0;
        s.text_styles = [
            (TextStyle::Small, regular(11.0)),
            (TextStyle::Body, regular(13.0)),
            (TextStyle::Button, regular(13.0)),
            (TextStyle::Heading, bold(16.0)),
            (TextStyle::Monospace, FontId::monospace(12.0)),
        ]
        .into();
    });
}

/// Right-aligned content in the rest of a row, one control high. Unlike a bare right-to-left
/// `with_layout`, it does not take the whole height of an unbounded parent (a modal).
pub fn trailing<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let h = ui.spacing().interact_size.y.max(32.0);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), h),
        egui::Layout::right_to_left(egui::Align::Center),
        add,
    )
    .inner
}
