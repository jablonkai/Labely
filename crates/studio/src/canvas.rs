//! The label canvas: an anti-aliased render at screen resolution, selection, and move / resize / rotate
//! gestures. A gesture is a run of coalesced commands, so it is one undo step.

use std::sync::Arc;

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use labely_core::{Document, Element, ElementId};
use serde_json::json;

use crate::app::{Gesture, LabelyApp, Raster};
use crate::prefs::Units;

/// An element frame in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub rot: f64,
}

impl Frame {
    pub fn of(e: &Element) -> Self {
        Self {
            x: e.x_mm,
            y: e.y_mm,
            w: e.width_mm,
            h: e.height_mm,
            rot: e.rotation_deg,
        }
    }

    fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Point in element-local coordinates (origin at the centre, unrotated).
    fn to_local(self, px: f64, py: f64) -> (f64, f64) {
        let (cx, cy) = self.center();
        let (s, c) = (-self.rot.to_radians()).sin_cos();
        let (dx, dy) = (px - cx, py - cy);
        (dx * c - dy * s, dx * s + dy * c)
    }

    fn to_world(self, lx: f64, ly: f64) -> (f64, f64) {
        let (cx, cy) = self.center();
        let (s, c) = self.rot.to_radians().sin_cos();
        (cx + lx * c - ly * s, cy + lx * s + ly * c)
    }

    fn contains(&self, px: f64, py: f64, slack: f64) -> bool {
        let (lx, ly) = self.to_local(px, py);
        lx.abs() <= self.w / 2.0 + slack && ly.abs() <= self.h / 2.0 + slack
    }

    /// The frame after dragging handle `(sx, sy)` (each −1, 0 or 1) to `(px, py)`; the
    /// opposite side stays fixed.
    pub fn resized(&self, handle: (i8, i8), px: f64, py: f64) -> Frame {
        const MIN: f64 = 0.5;
        let (lx, ly) = self.to_local(px, py);
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        let axis = |s: i8, p: f64, half: f64| -> (f64, f64) {
            if s == 0 {
                return (half * 2.0, 0.0);
            }
            let anchor = -f64::from(s) * half;
            let mut edge = p;
            if (edge - anchor).abs() < MIN {
                edge = anchor + f64::from(s) * MIN;
            }
            ((edge - anchor).abs(), (edge + anchor) / 2.0)
        };
        let (w, cx) = axis(handle.0, lx, hw);
        let (h, cy) = axis(handle.1, ly, hh);
        let (wx, wy) = self.to_world(cx, cy);
        Frame {
            x: wx - w / 2.0,
            y: wy - h / 2.0,
            w,
            h,
            rot: self.rot,
        }
    }
}

/// mm ↔ screen mapping.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub origin: Pos2,
    pub zoom: f32,
}

impl View {
    pub fn to_screen(&self, x: f64, y: f64) -> Pos2 {
        self.origin + vec2(x as f32, y as f32) * self.zoom
    }

    pub fn to_mm(&self, p: Pos2) -> (f64, f64) {
        let v = (p - self.origin) / self.zoom;
        (f64::from(v.x), f64::from(v.y))
    }
}

const HANDLES: [(i8, i8); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
];

/// Screen position of a handle; `(0, -2)` is the rotation handle above the top edge.
fn handle_pos(view: &View, f: &Frame, h: (i8, i8)) -> Pos2 {
    let ly = if h == (0, -2) {
        -f.h / 2.0 - 18.0 / f64::from(view.zoom)
    } else {
        f64::from(h.1) * f.h / 2.0
    };
    let (x, y) = f.to_world(f64::from(h.0) * f.w / 2.0, ly);
    view.to_screen(x, y)
}

/// Topmost visible element under `(x, y)` mm.
/// Visible, unlocked elements of the first page whose bounds touch `rect` (x0, y0, x1, y1 mm).
pub fn marquee_hits(doc: &Document, rect: (f64, f64, f64, f64)) -> Vec<ElementId> {
    let (x0, y0, x1, y1) = rect;
    doc.pages
        .first()
        .map(|page| {
            page.elements
                .iter()
                .filter(|e| !e.hidden && !e.locked)
                .filter(|e| {
                    let b = e.bounds();
                    b.0 <= x1 && b.2 >= x0 && b.1 <= y1 && b.3 >= y0
                })
                .map(|e| e.id)
                .collect()
        })
        .unwrap_or_default()
}

pub fn hit(doc: &Document, x: f64, y: f64, zoom: f32) -> Option<ElementId> {
    let slack = 3.0 / f64::from(zoom);
    doc.pages
        .first()?
        .elements
        .iter()
        .rev()
        .filter(|e| !e.hidden)
        .find(|e| Frame::of(e).contains(x, y, slack))
        .map(|e| e.id)
}

/// Binding values for showing 1-based `record` (clamped to the data), as printing all
/// records would fill them.
pub fn record_values(doc: &Document, record: usize) -> std::collections::BTreeMap<String, String> {
    let rows = doc.data.as_ref().map_or(0, |d| d.rows.len());
    let record = record.clamp(1, rows.max(1));
    let row = (rows > 0).then(|| record - 1);
    labely_core::commands::binding_values(doc, row, (record - 1) as u64)
}

/// Keep the cached raster in step with the document.
fn ensure_raster(app: &mut LabelyApp, ctx: &egui::Context, doc: &Arc<Document>) {
    let values = record_values(doc, app.ui.record);
    if app
        .raster
        .as_ref()
        .is_some_and(|r| Arc::ptr_eq(&r.doc, doc) && r.values == values)
    {
        return;
    }
    let services = app.session.services().clone();
    let dpi = doc
        .label
        .printer_model
        .as_deref()
        .and_then(|id| services.devices.models().into_iter().find(|m| m.id == id))
        .map_or(labely_core::commands::DEFAULT_DPI, |m| m.dpi);
    let opts = labely_core::services::RenderOptions {
        dpi,
        page: 0,
        values: values.clone(),
    };
    let Ok(out) = services.renderer.render(doc, &opts) else {
        app.raster = None;
        return;
    };
    let (w, h) = (out.bitmap.width() as usize, out.bitmap.height() as usize);
    let mut rgb = Vec::with_capacity(w * h * 3);
    for y in 0..out.bitmap.height() {
        for x in 0..out.bitmap.width() {
            let red = out.red.as_ref().is_some_and(|r| r.get(x, y));
            rgb.extend(match (out.bitmap.get(x, y), red) {
                (_, true) => INK_RED,
                (true, false) => [0, 0, 0],
                (false, false) => [255, 255, 255],
            });
        }
    }
    let image = egui::ColorImage::from_rgb([w, h], &rgb);
    match &mut app.raster {
        Some(r) if r.size == [w, h] => {
            r.texture.set(image, egui::TextureOptions::NEAREST);
            r.doc = doc.clone();
            r.dpi = dpi;
            r.values = values;
        }
        _ => {
            let texture = ctx.load_texture("label-raster", image, egui::TextureOptions::NEAREST);
            app.raster = Some(Raster {
                doc: doc.clone(),
                dpi,
                values,
                texture,
                size: [w, h],
            });
        }
    }
}

/// On-screen colour of the dots that print red.
const INK_RED: [u8; 3] = [214, 40, 40];

/// Channel `c` of the red ink at gray `g` (255 white, 0 full ink).
fn tint(c: u8, g: u8) -> u8 {
    let ink = 255 - u16::from(g);
    (255 - (ink * (255 - u16::from(c)) + 127) / 255) as u8
}

/// Largest side of the on-screen render, pixels.
const SMOOTH_MAX_SIDE: f64 = 4096.0;

/// Keep the canvas's anti-aliased render in step with the document and the screen scale.
/// It renders at twice the screen resolution and averages 2×2 blocks, so the hard-edged
/// shapes come out smooth; `None` when the renderer has no gray stage.
fn ensure_smooth(app: &mut LabelyApp, ctx: &egui::Context, doc: &Arc<Document>, px_per_mm: f32) {
    let values = record_values(doc, app.ui.record);
    let longest = doc.label.width_mm.max(doc.label.height_mm).max(1.0);
    // Quantise so small zoom steps reuse the texture (linear filtering covers the rest).
    let want = f64::from(px_per_mm) * 25.4 * 2.0;
    let cap = SMOOTH_MAX_SIDE * 2.0 * 25.4 / longest;
    let dpi = (want.min(cap) / 50.0).ceil().max(1.0) as u32 * 50;
    if app
        .smooth
        .as_ref()
        .is_some_and(|s| Arc::ptr_eq(&s.doc, doc) && s.values == values && s.dpi == dpi)
    {
        return;
    }
    let services = app.session.services().clone();
    let opts = labely_core::services::RenderOptions {
        dpi,
        page: 0,
        values: values.clone(),
    };
    let Ok((out, Some(gray))) = services.renderer.render_with_preview(doc, &opts) else {
        app.smooth = None;
        return;
    };
    let (gw, gh) = (gray.width as usize, gray.height as usize);
    let red = out
        .red
        .filter(|r| (r.width() as usize, r.height() as usize) == (gw, gh));
    let (w, h) = (gw.div_ceil(2).max(1), gh.div_ceil(2).max(1));
    let mut px = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            let (mut sum, mut n) = ([0u32; 3], 0u32);
            for (sx, sy) in [
                (2 * x, 2 * y),
                (2 * x + 1, 2 * y),
                (2 * x, 2 * y + 1),
                (2 * x + 1, 2 * y + 1),
            ] {
                if sx < gw && sy < gh {
                    let g = gray.data[sy * gw + sx];
                    let is_red = red.as_ref().is_some_and(|r| r.get(sx as u32, sy as u32));
                    for (c, total) in sum.iter_mut().enumerate() {
                        *total += u32::from(if is_red { tint(INK_RED[c], g) } else { g });
                    }
                    n += 1;
                }
            }
            px.extend(sum.map(|v| (v / n.max(1)) as u8));
        }
    }
    let image = egui::ColorImage::from_rgb([w, h], &px);
    match &mut app.smooth {
        Some(s) if s.size == [w, h] => {
            s.texture.set(image, egui::TextureOptions::LINEAR);
            s.doc = doc.clone();
            s.dpi = dpi;
            s.values = values;
        }
        _ => {
            let texture = ctx.load_texture("label-smooth", image, egui::TextureOptions::LINEAR);
            app.smooth = Some(crate::app::Smooth {
                doc: doc.clone(),
                dpi,
                values,
                texture,
                size: [w, h],
            });
        }
    }
}

/// The printable band across the label width, mm, when the printhead is narrower than the
/// label (`None` when everything prints or no model is set). Same rule as `doc.check`.
pub fn printable_span(app: &LabelyApp, doc: &Document) -> Option<(f64, f64)> {
    let id = doc.label.printer_model.as_deref()?;
    let m = app
        .session
        .services()
        .devices
        .models()
        .into_iter()
        .find(|m| m.id == id)?;
    let head_mm = f64::from(m.head_dots) * 25.4 / f64::from(m.dpi);
    (doc.label.width_mm > head_mm + 0.01).then(|| {
        let left = (doc.label.width_mm - head_mm) / 2.0;
        (left, left + head_mm)
    })
}

/// Ruler thickness, points.
pub const RULER: f32 = 18.0;

/// Tick spacing for rulers: (minor, labelled) in mm, so minor ticks are ≥ 5 px and labels
/// ≥ 40 px apart.
pub fn ruler_steps(units: Units, zoom: f32) -> (f64, f64) {
    let steps: &[f64] = match units {
        Units::Mm => &[0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0],
        Units::Inch => &[
            25.4 / 16.0,
            25.4 / 8.0,
            25.4 / 4.0,
            25.4 / 2.0,
            25.4,
            50.8,
            127.0,
            254.0,
        ],
    };
    let px = |mm: f64| mm * f64::from(zoom);
    let pick = |min: f64| {
        steps
            .iter()
            .copied()
            .find(|s| px(*s) >= min)
            .unwrap_or(steps[steps.len() - 1])
    };
    (pick(5.0), pick(40.0))
}

/// Rulers along the top and left of `area`, zero at the label's top-left corner.
fn rulers(painter: &egui::Painter, ui: &Ui, area: Rect, view: &View, units: Units) {
    let vis = ui.visuals();
    let bg = vis.panel_fill;
    let ink = vis.weak_text_color();
    let font = egui::FontId::proportional(9.0);
    let top = Rect::from_min_size(area.min, vec2(area.width(), RULER));
    let left = Rect::from_min_size(area.min, vec2(RULER, area.height()));
    painter.rect_filled(top, 0.0, bg);
    painter.rect_filled(left, 0.0, bg);
    let (minor, major) = ruler_steps(units, view.zoom);
    let stroke = Stroke::new(0.5, ink);
    let label = |mm: f64| {
        let v = units.from_mm(mm);
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    };
    // Horizontal ruler.
    let (start, _) = view.to_mm(pos2(top.left() + RULER, 0.0));
    let (end, _) = view.to_mm(pos2(top.right(), 0.0));
    let mut i = (start / minor).floor() as i64;
    while (i as f64) * minor <= end {
        let mm = i as f64 * minor;
        let x = view.to_screen(mm, 0.0).x;
        let is_major = ((mm / major).round() * major - mm).abs() < 1e-6;
        let len = if is_major { RULER * 0.6 } else { RULER * 0.25 };
        if x >= top.left() + RULER {
            painter.line_segment([pos2(x, top.bottom() - len), pos2(x, top.bottom())], stroke);
            if is_major {
                painter.text(
                    pos2(x + 2.0, top.top() + 1.0),
                    egui::Align2::LEFT_TOP,
                    label(mm),
                    font.clone(),
                    ink,
                );
            }
        }
        i += 1;
    }
    // Vertical ruler.
    let (_, start) = view.to_mm(pos2(0.0, left.top() + RULER));
    let (_, end) = view.to_mm(pos2(0.0, left.bottom()));
    let mut i = (start / minor).floor() as i64;
    while (i as f64) * minor <= end {
        let mm = i as f64 * minor;
        let y = view.to_screen(0.0, mm).y;
        let is_major = ((mm / major).round() * major - mm).abs() < 1e-6;
        let len = if is_major { RULER * 0.6 } else { RULER * 0.25 };
        if y >= left.top() + RULER {
            painter.line_segment([pos2(left.right() - len, y), pos2(left.right(), y)], stroke);
            if is_major {
                painter.text(
                    pos2(left.left() + 1.0, y + 1.0),
                    egui::Align2::LEFT_TOP,
                    label(mm),
                    font.clone(),
                    ink,
                );
            }
        }
        i += 1;
    }
    let corner = Rect::from_min_size(area.min, Vec2::splat(RULER));
    painter.rect_filled(corner, 0.0, bg);
    painter.text(
        corner.center(),
        egui::Align2::CENTER_CENTER,
        units.symbol(),
        font,
        ink,
    );
    painter.line_segment([top.left_bottom(), top.right_bottom()], stroke);
    painter.line_segment([left.right_top(), left.right_bottom()], stroke);
}

/// Draw the canvas and handle its input.
pub fn canvas(app: &mut LabelyApp, ui: &mut Ui) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let area = resp.rect;
    let (lw, lh) = (doc.label.width_mm as f32, doc.label.height_mm as f32);
    // Keep a fitted label fitted when the canvas changes size (panels opening, window resize).
    if app.ui.fitted
        && app
            .ui
            .canvas_size
            .is_some_and(|s| (s - area.size()).length() > 1.0)
    {
        app.ui.fit_pending = true;
    }
    app.ui.canvas_size = Some(area.size());
    if app.ui.fit_pending && lw > 0.0 && lh > 0.0 {
        app.ui.fitted = true;
        // Leave room for the rulers and the floating size and zoom controls below the label.
        app.ui.zoom = ((area.width() - 64.0) / lw)
            .min((area.height() - 130.0) / lh)
            .clamp(1.0, 80.0);
        app.ui.fit_pending = false;
    }
    if resp.hovered() {
        let z = ui.input(|i| i.zoom_delta());
        if z != 1.0 {
            app.ui.fitted = false;
            app.ui.zoom = (app.ui.zoom * z).clamp(1.0, 80.0);
        }
    }
    let zoom = app.ui.zoom;
    let label = Rect::from_center_size(area.center() - vec2(0.0, 22.0), vec2(lw, lh) * zoom);
    let view = View {
        origin: label.min,
        zoom,
    };
    app.ui.last_view = Some(view);

    painter.rect_filled(area, 0.0, crate::theme::pal().canvas);
    let radius = CornerRadius::same((doc.label.corner_radius_mm as f32 * zoom).min(120.0) as u8);
    painter.add(
        egui::Shadow {
            offset: [0, 10],
            blur: 30,
            spread: 0,
            color: Color32::from_black_alpha(34),
        }
        .as_shape(label, radius),
    );
    let [r, g, b] = app.ui.label_color;
    painter.rect_filled(label, radius, Color32::from_rgb(r, g, b));
    if app.ui.show_grid && app.ui.grid_mm > 0.0 {
        let step = app.ui.grid_mm as f32 * zoom;
        // Thin out dense grids: draw every n-th line so lines stay ≥ 6 px apart.
        let every = (6.0 / step).ceil().max(1.0);
        let grid = Stroke::new(0.5, Color32::from_black_alpha(28));
        let mut i = every;
        while i * step < label.width() {
            let x = label.left() + i * step;
            painter.line_segment([pos2(x, label.top()), pos2(x, label.bottom())], grid);
            i += every;
        }
        let mut i = every;
        while i * step < label.height() {
            let y = label.top() + i * step;
            painter.line_segment([pos2(label.left(), y), pos2(label.right(), y)], grid);
            i += every;
        }
    }
    ensure_raster(app, ui.ctx(), &doc);
    ensure_smooth(app, ui.ctx(), &doc, zoom * ui.ctx().pixels_per_point());
    let texture = app
        .smooth
        .as_ref()
        .map(|s| s.texture.id())
        .or_else(|| app.raster.as_ref().map(|r| r.texture.id()));
    if let Some(texture) = texture {
        painter.image(
            texture,
            label,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    painter.rect_stroke(
        label,
        radius,
        Stroke::new(1.0, crate::theme::pal().border),
        StrokeKind::Outside,
    );
    if let Some((x0, x1)) = printable_span(app, &doc) {
        // Outside the printhead: shaded, with a dashed edge.
        let shade = Color32::from_rgba_unmultiplied(220, 60, 60, 36);
        let edge = Stroke::new(1.0, Color32::from_rgba_unmultiplied(200, 40, 40, 160));
        for (a, b) in [(0.0, x0), (x1, doc.label.width_mm)] {
            let r = Rect::from_min_max(
                view.to_screen(a, 0.0),
                view.to_screen(b, doc.label.height_mm),
            );
            painter.rect_filled(r, 0.0, shade);
        }
        for x in [x0, x1] {
            let top = view.to_screen(x, 0.0);
            let bottom = view.to_screen(x, doc.label.height_mm);
            painter.add(egui::Shape::dashed_line(&[top, bottom], edge, 4.0, 3.0));
        }
    }

    let selection = app.selection();
    let accent = crate::theme::pal().primary;
    for e in doc.elements().filter(|e| !e.hidden) {
        let f = Frame::of(e);
        let corners = [(-1, -1), (1, -1), (1, 1), (-1, 1)].map(|h| handle_pos(&view, &f, h));
        let selected = selection.contains(&e.id);
        let stroke = if selected {
            Stroke::new(1.5, accent)
        } else {
            Stroke::new(0.5, Color32::from_rgba_unmultiplied(80, 120, 200, 70))
        };
        painter.add(egui::Shape::closed_line(corners.to_vec(), stroke));
    }
    let single = match selection.as_slice() {
        [id] => doc
            .element(*id)
            .filter(|e| !e.locked)
            .map(Frame::of)
            .map(|f| (*id, f)),
        _ => None,
    };
    if let Some((_, f)) = &single {
        let top = handle_pos(&view, f, (0, -1));
        let rot = handle_pos(&view, f, (0, -2));
        painter.line_segment([top, rot], Stroke::new(1.0, accent));
        painter.circle(rot, 4.5, Color32::WHITE, Stroke::new(1.5, accent));
        for h in HANDLES {
            let p = handle_pos(&view, f, h);
            painter.rect(
                Rect::from_center_size(p, Vec2::splat(7.0)),
                1.0,
                Color32::WHITE,
                Stroke::new(1.5, accent),
                StrokeKind::Middle,
            );
        }
    }

    if app.ui.gesture.is_some() {
        let guide = Stroke::new(1.0, Color32::from_rgb(230, 60, 140));
        for x in &app.ui.guides.0 {
            let p = view.to_screen(*x, 0.0).x;
            painter.line_segment([pos2(p, area.top()), pos2(p, area.bottom())], guide);
        }
        for y in &app.ui.guides.1 {
            let p = view.to_screen(0.0, *y).y;
            painter.line_segment([pos2(area.left(), p), pos2(area.right(), p)], guide);
        }
    }
    for e in doc.elements().filter(|e| e.locked && !e.hidden) {
        let (x, y) = Frame::of(e).center();
        painter.text(
            view.to_screen(x, y),
            egui::Align2::CENTER_CENTER,
            "🔒",
            egui::FontId::proportional(11.0),
            Color32::from_black_alpha(110),
        );
    }
    if app.ui.show_rulers {
        rulers(&painter, ui, area, &view, app.units());
    }

    // Input.
    let shift = ui.input(|i| i.modifiers.shift);
    if resp.drag_started()
        && let Some(p) = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos())
    {
        start_gesture(app, &doc, &view, single, p, shift);
    }
    if resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let alt = ui.input(|i| i.modifiers.alt);
        update_gesture(app, &view, p, shift, alt);
    }
    if let Some(Gesture::Marquee { start, current, .. }) = &app.ui.gesture {
        let r = egui::Rect::from_two_pos(*start, *current);
        painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(40, 120, 220, 30));
        painter.rect_stroke(
            r,
            0.0,
            egui::Stroke::new(1.0, Color32::from_rgb(40, 120, 220)),
            egui::StrokeKind::Inside,
        );
    }
    if resp.drag_stopped() {
        app.ui.gesture = None;
        app.ui.guides = (Vec::new(), Vec::new());
    }
    if let Some(el) = resp.dnd_release_payload::<crate::menu::ElementDrag>()
        && let Some(p) = ui.input(|i| i.pointer.hover_pos())
    {
        let (x, y) = view.to_mm(p);
        crate::menu::drop_element(app, el.0, x, y);
    } else if resp
        .dnd_hover_payload::<crate::menu::ElementDrag>()
        .is_some()
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Copy);
    }
    if let Some(col) = resp.dnd_release_payload::<crate::menu::ColumnDrag>()
        && let Some(p) = ui.input(|i| i.pointer.hover_pos())
    {
        let (x, y) = view.to_mm(p);
        crate::menu::drop_column(app, &col.0, x, y, zoom);
    } else if resp
        .dnd_hover_payload::<crate::menu::ColumnDrag>()
        .is_some()
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Copy);
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let (x, y) = view.to_mm(p);
        match hit(&doc, x, y, zoom) {
            Some(id) if shift => {
                app.run("selection.set", json!({"ids": [id], "mode": "toggle"}));
            }
            Some(id) => app.select(&[id]),
            None => app.select(&[]),
        }
    }
    crate::shell::canvas_overlays(app, ui, area);
    if resp.hovered()
        && let Some(p) = ui.input(|i| i.pointer.hover_pos())
    {
        let (x, y) = view.to_mm(p);
        let near_handle = single.is_some_and(|(_, f)| {
            HANDLES
                .iter()
                .chain(&[(0, -2)])
                .any(|h| handle_pos(&view, &f, *h).distance(p) < 7.0)
        });
        if near_handle || app.ui.gesture.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        } else if hit(&doc, x, y, zoom).is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
}

fn start_gesture(
    app: &mut LabelyApp,
    doc: &Document,
    view: &View,
    single: Option<(ElementId, Frame)>,
    p: Pos2,
    shift: bool,
) {
    if let Some((id, f)) = single {
        if handle_pos(view, &f, (0, -2)).distance(p) < 8.0 {
            let c = {
                let (cx, cy) = f.center();
                view.to_screen(cx, cy)
            };
            let key = app.next_key("rotate");
            app.ui.gesture = Some(Gesture::Rotate {
                id,
                center: c,
                start_angle: (p - c).angle(),
                orig_deg: f.rot,
                key,
            });
            return;
        }
        if let Some(h) = HANDLES
            .iter()
            .find(|h| handle_pos(view, &f, **h).distance(p) < 7.0)
        {
            let key = app.next_key("resize");
            app.ui.gesture = Some(Gesture::Resize {
                id,
                handle: *h,
                orig: f,
                key,
            });
            return;
        }
    }
    let (x, y) = view.to_mm(p);
    let Some(target) = hit(doc, x, y, view.zoom) else {
        let base = if shift { app.selection() } else { Vec::new() };
        app.ui.gesture = Some(Gesture::Marquee {
            start: p,
            current: p,
            base,
        });
        return;
    };
    let mut sel = app.selection();
    if !sel.contains(&target) {
        if shift {
            sel.push(target);
        } else {
            sel = vec![target];
        }
        app.select(&sel);
        sel = app.selection();
    }
    let originals: Vec<_> = sel
        .iter()
        .filter_map(|id| doc.element(*id))
        .filter(|e| !e.locked)
        .map(|e| (e.id, e.x_mm, e.y_mm))
        .collect();
    if originals.is_empty() {
        return;
    }
    let bounds = union_bounds(doc, originals.iter().map(|o| o.0));
    let key = app.next_key("move");
    app.ui.gesture = Some(Gesture::Move {
        start: p,
        originals,
        bounds,
        key,
    });
}

/// Round to 0.1 mm so dragged values stay tidy.
fn tidy(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Union of the elements' rotated bounding boxes, mm.
fn union_bounds(doc: &Document, ids: impl Iterator<Item = ElementId>) -> (f64, f64, f64, f64) {
    ids.filter_map(|id| doc.element(id))
        .map(labely_core::Element::bounds)
        .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, b| {
            (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
        })
}

/// A snapped drag offset and the guide lines it snapped to.
#[derive(Debug, PartialEq)]
pub struct Snapped {
    pub dx: f64,
    pub dy: f64,
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
}

/// Snap a drag of `bounds` by (dx, dy): edges and centres go to the label's edges and centre
/// and to other elements' edges and centres within `thr` mm; otherwise the left/top edge goes
/// to the `grid` when one is shown.
pub fn snap(
    doc: &Document,
    moving: &[ElementId],
    bounds: (f64, f64, f64, f64),
    dx: f64,
    dy: f64,
    thr: f64,
    grid: Option<f64>,
) -> Snapped {
    let (w, h) = (doc.label.width_mm, doc.label.height_mm);
    let mut cx = vec![0.0, w / 2.0, w];
    let mut cy = vec![0.0, h / 2.0, h];
    for e in doc
        .elements()
        .filter(|e| !e.hidden && !moving.contains(&e.id))
    {
        let b = e.bounds();
        cx.extend([b.0, (b.0 + b.2) / 2.0, b.2]);
        cy.extend([b.1, (b.1 + b.3) / 2.0, b.3]);
    }
    let axis = |lo: f64, hi: f64, d: f64, cands: &[f64]| -> (f64, Option<f64>) {
        let mine = [lo + d, (lo + hi) / 2.0 + d, hi + d];
        let best = cands
            .iter()
            .flat_map(|c| mine.iter().map(move |m| (c - m, *c)))
            .filter(|(off, _)| off.abs() <= thr)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
        match best {
            Some((off, line)) => (d + off, Some(line)),
            None => match grid {
                Some(g) if g > 0.0 => (((lo + d) / g).round() * g - lo, None),
                _ => (d, None),
            },
        }
    };
    let (dx, gx) = axis(bounds.0, bounds.2, dx, &cx);
    let (dy, gy) = axis(bounds.1, bounds.3, dy, &cy);
    Snapped {
        dx,
        dy,
        xs: gx.into_iter().collect(),
        ys: gy.into_iter().collect(),
    }
}

fn update_gesture(app: &mut LabelyApp, view: &View, p: Pos2, shift: bool, alt: bool) {
    let Some(g) = app.ui.gesture.clone() else {
        return;
    };
    match g {
        Gesture::Marquee { start, base, .. } => {
            let (x0, y0) = view.to_mm(start);
            let (x1, y1) = view.to_mm(p);
            let mut sel = base.clone();
            if let Some(doc) = app.session.doc(None) {
                for id in marquee_hits(doc, (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))) {
                    if !sel.contains(&id) {
                        sel.push(id);
                    }
                }
            }
            if sel != app.selection() {
                app.select(&sel);
            }
            app.ui.gesture = Some(Gesture::Marquee {
                start,
                current: p,
                base,
            });
        }
        Gesture::Move {
            start,
            originals,
            bounds,
            key,
        } => {
            let d = (p - start) / view.zoom;
            let (mut dx, mut dy) = (f64::from(d.x), f64::from(d.y));
            app.ui.guides = (Vec::new(), Vec::new());
            // Alt drags freely.
            if app.ui.snap
                && !alt
                && let Some(doc) = app.session.doc(None)
            {
                let moving: Vec<ElementId> = originals.iter().map(|o| o.0).collect();
                let grid = app.ui.show_grid.then_some(app.ui.grid_mm);
                let s = snap(
                    doc,
                    &moving,
                    bounds,
                    dx,
                    dy,
                    6.0 / f64::from(view.zoom),
                    grid,
                );
                (dx, dy) = (s.dx, s.dy);
                app.ui.guides = (s.xs, s.ys);
            }
            for (id, x0, y0) in originals {
                app.run(
                    "element.move",
                    json!({"id": id, "x_mm": tidy(x0 + dx), "y_mm": tidy(y0 + dy), "coalesce": key}),
                );
            }
        }
        Gesture::Resize {
            id,
            handle,
            orig,
            key,
        } => {
            let (x, y) = view.to_mm(p);
            let f = orig.resized(handle, x, y);
            app.run(
                "element.set_props",
                json!({"id": id, "props": {"x_mm": tidy(f.x), "y_mm": tidy(f.y), "width_mm": tidy(f.w), "height_mm": tidy(f.h)}, "coalesce": key}),
            );
        }
        Gesture::Rotate {
            id,
            center,
            start_angle,
            orig_deg,
            key,
        } => {
            let delta = f64::from((p - center).angle() - start_angle).to_degrees();
            let mut deg = (orig_deg + delta).rem_euclid(360.0);
            deg = if shift {
                (deg / 15.0).round() * 15.0
            } else {
                deg.round()
            };
            app.run(
                "element.rotate",
                json!({"id": id, "rotation_deg": deg.rem_euclid(360.0), "coalesce": key}),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_keeps_opposite_corner() {
        let f = Frame {
            x: 10.0,
            y: 10.0,
            w: 20.0,
            h: 10.0,
            rot: 0.0,
        };
        let r = f.resized((1, 1), 40.0, 30.0);
        assert_eq!((r.x, r.y, r.w, r.h), (10.0, 10.0, 30.0, 20.0));
        let r = f.resized((-1, 0), 0.0, 99.0);
        assert_eq!((r.x, r.y, r.w, r.h), (0.0, 10.0, 30.0, 10.0));
        // Rotated 90°: the local +x axis points down; the fixed corner stays put in world.
        let f = Frame { rot: 90.0, ..f };
        let fixed = f.to_world(-10.0, -5.0);
        let r = f.resized((1, 1), 15.0, 35.0);
        let after = r.to_world(-r.w / 2.0, -r.h / 2.0);
        assert!((fixed.0 - after.0).abs() < 1e-9 && (fixed.1 - after.1).abs() < 1e-9);
    }

    #[test]
    fn snapping_prefers_nearby_lines_then_grid() {
        let mut s = labely_core::Session::new(labely_printers::default_services());
        s.execute("doc.new", json!({"width_mm": 40, "height_mm": 30}));
        s.execute(
            "element.add_rect",
            json!({"x_mm": 10, "y_mm": 10, "width_mm": 4, "height_mm": 4}),
        );
        s.execute(
            "element.add_rect",
            json!({"x_mm": 30, "y_mm": 2, "width_mm": 5, "height_mm": 5}),
        );
        let doc = s.doc(None).unwrap();
        let me = [ElementId(1)];
        let b = (10.0, 10.0, 14.0, 14.0);
        // Right edge 14 + 15.7 = 29.7 snaps to the other's left edge at 30.
        let r = snap(doc, &me, b, 15.7, 0.0, 0.5, None);
        assert!((r.dx - 16.0).abs() < 1e-9 && r.xs == vec![30.0], "{r:?}");
        // Centre 12 + 7.9 = 19.9 snaps to the label centre 20 (y untouched: 10+0.2 → 10.2).
        let r = snap(doc, &me, b, 7.9, 0.2, 0.5, None);
        assert!((r.dx - 8.0).abs() < 1e-9 && r.ys.is_empty());
        // Nothing near: the grid rounds the left/top edge.
        let r = snap(doc, &me, b, 3.3, 3.3, 0.1, Some(1.0));
        assert!((r.dx - 3.0).abs() < 1e-9 && (r.dy - 3.0).abs() < 1e-9);
        assert_eq!(snap(doc, &me, b, 3.3, 3.3, 0.1, None).dx, 3.3);
    }

    #[test]
    fn hit_testing_respects_rotation_and_z() {
        let mut doc = Document::new("t", labely_core::LabelSpec::default());
        let mut s = labely_core::Session::new(labely_printers::default_services());
        let _ = &mut doc;
        s.execute("doc.new", json!({"width_mm": 50, "height_mm": 30}));
        s.execute(
            "element.add_rect",
            json!({"x_mm": 0, "y_mm": 0, "width_mm": 20, "height_mm": 4}),
        );
        s.execute(
            "element.add_rect",
            json!({"x_mm": 5, "y_mm": 0, "width_mm": 4, "height_mm": 4}),
        );
        let doc = s.doc(None).unwrap();
        assert_eq!(hit(doc, 7.0, 2.0, 10.0), Some(ElementId(2)), "topmost wins");
        assert_eq!(hit(doc, 15.0, 2.0, 10.0), Some(ElementId(1)));
        assert_eq!(hit(doc, 40.0, 20.0, 10.0), None);
    }
}
