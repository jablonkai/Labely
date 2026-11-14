//! Rect, ellipse and line drawing with hard (non anti-aliased) edges.

use labely_core::document::{BorderProps, BorderStyle, LineProps, ShapeProps};
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

/// Stroke `path` with width `w` (dots) and optional dash pattern.
fn stroke(pm: &mut Pixmap, path: &Path, w: f32, dash: Option<(Vec<f32>, LineCap)>, ts: Transform) {
    let mut st = Stroke {
        width: w.max(1.0),
        ..Stroke::default()
    };
    if let Some((d, cap)) = dash {
        st.dash = StrokeDash::new(d, 0.0);
        st.line_cap = cap;
    }
    pm.stroke_path(path, &black(), &st, ts, None);
}

/// Rectangle with concave (quarter-circle) corners of radius `r`.
fn ticket_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let (x1, y1) = (x + w, y + h);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x1 - r, y);
    pb.cubic_to(x1 - r, y + k, x1 - k, y + r, x1, y + r);
    pb.line_to(x1, y1 - r);
    pb.cubic_to(x1 - k, y1 - r, x1 - r, y1 - k, x1 - r, y1);
    pb.line_to(x + r, y1);
    pb.cubic_to(x + r, y1 - k, x + k, y1 - r, x, y1 - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x + k, y + r, x + r, y + k, x + r, y);
    pb.close();
    pb.finish()
}

/// Closed scalloped outline: bumps of about `pitch` dots bulging outwards from the rectangle.
fn scallop_path(x: f32, y: f32, w: f32, h: f32, pitch: f32) -> Option<Path> {
    let corners = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    for i in 0..4 {
        let (ax, ay) = corners[i];
        let (bx, by) = corners[(i + 1) % 4];
        let len = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
        let n = (len / pitch).round().max(1.0) as usize;
        // Outward normal of a clockwise rectangle edge.
        let (nx, ny) = ((by - ay) / len, -(bx - ax) / len);
        let bump = len / n as f32 * 0.6;
        for j in 0..n {
            let t0 = j as f32 / n as f32;
            let t1 = (j + 1) as f32 / n as f32;
            let tm = (t0 + t1) / 2.0;
            pb.quad_to(
                ax + (bx - ax) * tm + nx * bump,
                ay + (by - ay) * tm + ny * bump,
                ax + (bx - ax) * t1,
                ay + (by - ay) * t1,
            );
        }
    }
    pb.close();
    pb.finish()
}

/// Draw a decorative border inside a `w`×`h` frame placed by `ts`.
pub(crate) fn draw_border(
    pm: &mut Pixmap,
    p: &BorderProps,
    w: f32,
    h: f32,
    dots_per_mm: f32,
    ts: Transform,
) {
    let s = (p.stroke_mm as f32 * dots_per_mm).round().max(1.0);
    let r = (p.corner_radius_mm as f32 * dots_per_mm).max(0.0);
    let rect = |inset: f32| {
        rounded_rect(
            inset,
            inset,
            w - 2.0 * inset,
            h - 2.0 * inset,
            (r - inset).max(0.0),
        )
    };
    match p.style {
        BorderStyle::Single => {
            if let Some(path) = rect(s / 2.0) {
                stroke(pm, &path, s, None, ts);
            }
        }
        BorderStyle::Double => {
            for inset in [s / 2.0, s * 2.5] {
                if let Some(path) = rect(inset) {
                    stroke(pm, &path, s, None, ts);
                }
            }
        }
        BorderStyle::ThickThin => {
            if let Some(path) = rect(s) {
                stroke(pm, &path, 2.0 * s, None, ts);
            }
            if let Some(path) = rect(s * 3.5) {
                stroke(pm, &path, s, None, ts);
            }
        }
        BorderStyle::Dashed => {
            if let Some(path) = rect(s / 2.0) {
                stroke(
                    pm,
                    &path,
                    s,
                    Some((vec![s * 4.0, s * 2.0], LineCap::Butt)),
                    ts,
                );
            }
        }
        BorderStyle::Dotted => {
            if let Some(path) = rect(s / 2.0) {
                stroke(pm, &path, s, Some((vec![s, s * 1.5], LineCap::Butt)), ts);
            }
        }
        BorderStyle::Corners => {
            let len = (w.min(h) * 0.2).max(s * 3.0);
            let i = s / 2.0;
            let mut pb = PathBuilder::new();
            for (cx, cy, dx, dy) in [
                (i, i, 1.0, 1.0),
                (w - i, i, -1.0, 1.0),
                (w - i, h - i, -1.0, -1.0),
                (i, h - i, 1.0, -1.0),
            ] {
                pb.move_to(cx + dx * len, cy);
                pb.line_to(cx, cy);
                pb.line_to(cx, cy + dy * len);
            }
            if let Some(path) = pb.finish() {
                stroke(pm, &path, s, None, ts);
            }
        }
        BorderStyle::Ticket => {
            let rr = r.max(w.min(h) * 0.12);
            let i = s / 2.0;
            if let Some(path) = ticket_path(i, i, w - s, h - s, rr) {
                stroke(pm, &path, s, None, ts);
            }
        }
        BorderStyle::Scallop => {
            let pitch = (3.0 * dots_per_mm).max(s * 4.0);
            let i = s / 2.0 + pitch * 0.35;
            if let Some(path) = scallop_path(i, i, w - 2.0 * i, h - 2.0 * i, pitch) {
                stroke(pm, &path, s, None, ts);
            }
        }
    }
}

/// Draw a black horizontal or vertical bar `[x0, x1) × [y0, y1)` in frame space.
pub(crate) fn bar(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, ts: Transform) {
    if let Some(r) = Rect::from_ltrb(x0, y0, x1.max(x0 + 1.0), y1.max(y0 + 1.0)) {
        pm.fill_rect(r, &black(), ts, None);
    }
}
