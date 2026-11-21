//! Visual design tokens and the egui style built from them (design/labely-desktop.figcraft).

use std::{cell::Cell, sync::Arc};

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, Theme, Visuals,
};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The colour tokens of one theme. Read the current one with [`pal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub primary: Color32,
    /// Hover fill of primary buttons (white content on it).
    pub primary_hover: Color32,
    /// Accent text on primary tints.
    pub primary_dark: Color32,
    /// Primary tint: selected rows, active rail item.
    pub primary_tint: Color32,
    pub primary_tint2: Color32,
    /// Window background behind cards.
    pub bg: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub border_soft: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub muted: Color32,
    /// The canvas around the label.
    pub canvas: Color32,
    pub green: Color32,
    pub green_tint: Color32,
    pub amber: Color32,
    pub amber_tint: Color32,
    /// Title bar and tab strip.
    pub chrome: Color32,
    pub red: Color32,
    /// Background of error cards and badges.
    pub red_tint: Color32,
}

pub const LIGHT: Palette = Palette {
    primary: rgb(0x4C6EF5),
    primary_hover: rgb(0x3B5BDB),
    primary_dark: rgb(0x3B5BDB),
    primary_tint: rgb(0xEDF2FF),
    primary_tint2: rgb(0xDBE4FF),
    bg: rgb(0xF5F6F8),
    surface: rgb(0xFFFFFF),
    border: rgb(0xE4E7EC),
    border_soft: rgb(0xEEF0F3),
    text: rgb(0x1B2130),
    text2: rgb(0x475066),
    muted: rgb(0x8A93A6),
    canvas: rgb(0xE8EBF0),
    green: rgb(0x12B886),
    green_tint: rgb(0xE6FCF5),
    amber: rgb(0xF59F00),
    amber_tint: rgb(0xFFF4E0),
    chrome: rgb(0xFBFBFC),
    red: rgb(0xFA5252),
    red_tint: rgb(0xFFF0F0),
};

pub const DARK: Palette = Palette {
    primary: rgb(0x5C7CFA),
    primary_hover: rgb(0x4C6EF5),
    primary_dark: rgb(0x91A7FF),
    primary_tint: rgb(0x1E2640),
    primary_tint2: rgb(0x28345C),
    bg: rgb(0x14171D),
    surface: rgb(0x1C2028),
    border: rgb(0x2E3440),
    border_soft: rgb(0x262B35),
    text: rgb(0xE6E9EF),
    text2: rgb(0xB4BCCB),
    muted: rgb(0x8790A3),
    canvas: rgb(0x101318),
    green: rgb(0x20C997),
    green_tint: rgb(0x0F2E26),
    amber: rgb(0xFAB005),
    amber_tint: rgb(0x33270D),
    chrome: rgb(0x181B22),
    red: rgb(0xFF6B6B),
    red_tint: rgb(0x3A1C1F),
};

impl Palette {
    pub fn of(theme: Theme) -> &'static Palette {
        match theme {
            Theme::Light => &LIGHT,
            Theme::Dark => &DARK,
        }
    }
}

thread_local! {
    /// Whether this thread's UI is dark; set by [`sync`] each frame. Per thread, so parallel
    /// UI tests with different themes do not see each other.
    static DARK_MODE: Cell<bool> = const { Cell::new(false) };
}

/// The palette of the current theme.
pub fn pal() -> &'static Palette {
    if DARK_MODE.with(Cell::get) {
        &DARK
    } else {
        &LIGHT
    }
}

/// Follow the theme egui resolved for this frame. Call at the start of every frame.
pub fn sync(ctx: &Context) {
    DARK_MODE.with(|d| d.set(ctx.theme() == Theme::Dark));
}

/// Apply the user's theme preference.
pub fn set_pref(ctx: &Context, pref: crate::prefs::ThemePref) {
    use crate::prefs::ThemePref;
    ctx.set_theme(match pref {
        ThemePref::System => egui::ThemePreference::System,
        ThemePref::Light => egui::ThemePreference::Light,
        ThemePref::Dark => egui::ThemePreference::Dark,
    });
    sync(ctx);
    ctx.request_repaint();
}

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
        .layout_no_wrap(text.to_uppercase(), bold(11.0), pal().muted);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    let label = text.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &label));
    ui.painter().galley(rect.min, galley, pal().muted);
    ui.add_space(2.0);
    resp
}

/// Surface card with a soft border.
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(pal().surface)
        .stroke(Stroke::new(1.0, pal().border))
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
    .fill(pal().primary)
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

/// The egui visuals of `theme`, built from its palette.
pub fn visuals(theme: Theme) -> Visuals {
    let p = Palette::of(theme);
    let dark = theme == Theme::Dark;
    let mut v = match theme {
        Theme::Light => Visuals::light(),
        Theme::Dark => Visuals::dark(),
    };
    let r = CornerRadius::same(RADIUS);
    v.panel_fill = p.surface;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(RADIUS_LG);
    v.window_shadow = Shadow {
        offset: [0, 12],
        blur: 40,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 140 } else { 40 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 110 } else { 30 }),
    };
    v.menu_corner_radius = r;
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.bg;
    v.code_bg_color = p.bg;
    v.hyperlink_color = p.primary;
    v.selection.bg_fill = p.primary_tint2;
    v.selection.stroke = Stroke::new(1.0, p.primary);
    v.override_text_color = None;
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border_soft);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = p.border;
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = p.primary_tint2;
    w.hovered.weak_bg_fill = p.bg;
    w.hovered.bg_stroke = Stroke::new(1.0, p.primary_tint2);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = p.primary;
    w.active.weak_bg_fill = p.primary_tint2;
    w.active.bg_stroke = Stroke::new(1.0, p.primary);
    w.active.fg_stroke = Stroke::new(1.5, p.primary_dark);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
    w.open.bg_fill = p.primary_tint;
    w.open.weak_bg_fill = p.primary_tint;
    w.open.bg_stroke = Stroke::new(1.0, p.primary_tint2);
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = r;
    v
}

/// Apply fonts, colours, spacing and text sizes for both themes.
pub fn apply(ctx: &Context) {
    ctx.set_fonts(fonts());
    // With no theme reported by the system (and in tests), follow the light design.
    ctx.options_mut(|o| o.fallback_theme = Theme::Light);
    for theme in [Theme::Light, Theme::Dark] {
        ctx.style_mut_of(theme, |s| {
            s.visuals = visuals(theme);
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

/// Paint one line of `text` anchored at `pos` so it fits in `max_width`: first shrink the
/// font (down to 80 %), then cut it with "…". Use it wherever text is painted into a box of
/// fixed width, since longer translations would otherwise run over the box edge.
pub fn fit_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    align: egui::Align2,
    text: impl Into<String>,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> egui::Rect {
    let text = text.into();
    let max_width = max_width.max(1.0);
    let natural = painter
        .layout_no_wrap(text.clone(), font.clone(), color)
        .size()
        .x;
    let mut font = font;
    if natural > max_width {
        font.size = (font.size * max_width / natural).max(font.size * 0.8);
    }
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width);
    let galley = painter.layout_job(job);
    let rect = align.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::ThemePref;

    #[test]
    fn dark_pref_switches_palette_and_visuals() {
        let ctx = Context::default();
        apply(&ctx);
        set_pref(&ctx, ThemePref::System);
        assert_eq!(ctx.theme(), Theme::Light);
        assert_eq!(pal(), &LIGHT);
        set_pref(&ctx, ThemePref::Dark);
        assert_eq!(pal(), &DARK);
        let v = ctx.global_style().visuals.clone();
        assert!(v.dark_mode);
        assert_eq!(v.panel_fill, DARK.surface);
        assert_eq!(v.hyperlink_color, DARK.primary);
        assert_eq!(ctx.style_of(Theme::Light).visuals.panel_fill, LIGHT.surface);
        set_pref(&ctx, ThemePref::Light);
        assert_eq!(pal(), &LIGHT);
        assert!(!ctx.global_style().visuals.dark_mode);
    }
}
