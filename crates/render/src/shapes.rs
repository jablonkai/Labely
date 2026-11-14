//! Rect, ellipse and line drawing with hard (non anti-aliased) edges.

use labely_core::document::{LineProps, ShapeProps};
use tiny_skia::{
    FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, StrokeDash, Transform,
};

fn black() -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(0, 0, 0, 255);
    p.anti_alias = false;
    p
}

/// Rounded rectangle path (cubic corner approximation).
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    if r <= 0.0 {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let k = 0.552_284_8 * r;
    let (x1, y1) = (x + w, y + h);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x1 - r, y);
    pb.cubic_to(x1 - r + k, y, x1, y + r - k, x1, y + r);
    pb.line_to(x1, y1 - r);
    pb.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    pb.line_to(x + r, y1);
    pb.cubic_to(x + r - k, y1, x, y1 - r + k, x, y1 - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

fn oval(x: f32, y: f32, w: f32, h: f32) -> Option<Path> {
    PathBuilder::from_oval(Rect::from_xywh(x, y, w, h)?)
}

/// Draw a rect (`ellipse == false`) or ellipse filling a `w`×`h` frame placed by `ts`.
pub(crate) fn draw_shape(
    pm: &mut Pixmap,
    p: &ShapeProps,
    ellipse: bool,
    w: f32,
    h: f32,
    dots_per_mm: f32,
    ts: Transform,
) {
    let paint = black();
    let s = (p.stroke_mm as f32 * dots_per_mm).round().max(1.0);
    let r = (p.corner_radius_mm as f32 * dots_per_mm).max(0.0);
    let make = |inset: f32| {
        let (x, y, pw, ph) = (inset, inset, w - 2.0 * inset, h - 2.0 * inset);
        if ellipse {
            oval(x, y, pw, ph)
        } else {
            rounded_rect(x, y, pw, ph, (r - inset).max(0.0))
        }
    };
    // Stroke stays inside the frame; a stroke covering the whole frame is a fill.
    if p.fill || s * 2.0 >= w.min(h) {
        if let Some(path) = make(0.0) {
            pm.fill_path(&path, &paint, FillRule::Winding, ts, None);
        }
        return;
    }
    if let Some(path) = make(s / 2.0) {
        let stroke = Stroke {
            width: s,
            ..Stroke::default()
        };
        pm.stroke_path(&path, &paint, &stroke, ts, None);
    }
}

/// Draw a line from `(0, 0)` to `(dx, dy)` in frame space placed by `ts`.
pub(crate) fn draw_line(
    pm: &mut Pixmap,
    p: &LineProps,
    dx: f32,
    dy: f32,
    dots_per_mm: f32,
    ts: Transform,
) {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0);
    pb.line_to(dx, dy);
    let Some(path) = pb.finish() else { return };
    let mut stroke = Stroke {
        width: (p.stroke_mm as f32 * dots_per_mm).round().max(1.0),
        line_cap: LineCap::Butt,
        ..Stroke::default()
    };
    if let Some([dash, gap]) = p.dash_mm {
        let (d, g) = (dash as f32 * dots_per_mm, gap as f32 * dots_per_mm);
        if d > 0.0 && g > 0.0 {
            stroke.dash = StrokeDash::new(vec![d, g], 0.0);
        }
    }
    pm.stroke_path(&path, &black(), &stroke, ts, None);
}
