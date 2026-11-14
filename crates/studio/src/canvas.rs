//! The label canvas: the exact 1-bit print raster, selection, and move / resize / rotate
//! gestures. A gesture is a run of coalesced commands, so it is one undo step.

use std::sync::Arc;

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use labely_core::{Document, Element, ElementId};
use serde_json::json;

use crate::app::{Gesture, LabelyApp, Raster};

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
    let mut gray = Vec::with_capacity(w * h);
    for y in 0..out.bitmap.height() {
        for x in 0..out.bitmap.width() {
            gray.push(if out.bitmap.get(x, y) { 0u8 } else { 255 });
        }
    }
    let image = egui::ColorImage::from_gray([w, h], &gray);
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

/// Draw the canvas and handle its input.
pub fn canvas(app: &mut LabelyApp, ui: &mut Ui) {
    let Some(doc) = app.session.doc_arc(None) else {
        return;
    };
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let area = resp.rect;
    let (lw, lh) = (doc.label.width_mm as f32, doc.label.height_mm as f32);
    if app.ui.fit_pending && lw > 0.0 && lh > 0.0 {
        app.ui.zoom = ((area.width() - 48.0) / lw)
            .min((area.height() - 48.0) / lh)
            .clamp(1.0, 80.0);
        app.ui.fit_pending = false;
    }
    if resp.hovered() {
        let z = ui.input(|i| i.zoom_delta());
        if z != 1.0 {
            app.ui.zoom = (app.ui.zoom * z).clamp(1.0, 80.0);
        }
    }
    let zoom = app.ui.zoom;
    let label = Rect::from_center_size(area.center(), vec2(lw, lh) * zoom);
    let view = View {
        origin: label.min,
        zoom,
    };
    app.ui.last_view = Some(view);

    painter.rect_filled(area, 0.0, ui.visuals().extreme_bg_color);
    let radius = CornerRadius::same((doc.label.corner_radius_mm as f32 * zoom).min(120.0) as u8);
    painter.rect_filled(
        label.translate(vec2(3.0, 3.0)),
        radius,
        Color32::from_black_alpha(40),
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
    if let Some(r) = &app.raster {
        painter.image(
            r.texture.id(),
            label,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    painter.rect_stroke(
        label,
        radius,
        Stroke::new(1.0, Color32::GRAY),
        StrokeKind::Outside,
    );

    let selection = app.selection();
    let accent = ui.visuals().selection.stroke.color;
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
    if resp.drag_stopped() {
        app.ui.gesture = None;
        app.ui.guides = (Vec::new(), Vec::new());
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
