//! Stroke icons from the design, drawn with the egui painter: a small SVG path parser (M L H V
//! C S Q A Z, absolute and relative) turns each icon into polylines on a 24-unit grid.

use std::collections::HashMap;
use std::sync::OnceLock;

use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

use crate::icon_paths::ICONS;
use crate::theme;

/// One subpath: points on the 24-unit grid, closed or open.
type Poly = (Vec<Pos2>, bool);

fn tokens(d: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let b = d.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            out.push(Tok::Cmd(c));
            i += 1;
        } else if c == '-' || c == '.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut dot = c == '.';
            while i < b.len() {
                let ch = b[i] as char;
                if ch.is_ascii_digit() {
                    i += 1;
                } else if ch == '.' && !dot {
                    dot = true;
                    i += 1;
                } else if (ch == 'e' || ch == 'E') && i + 1 < b.len() {
                    i += 1;
                    if b[i] == b'-' || b[i] == b'+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(v) = d[start..i].parse::<f32>() {
                out.push(Tok::Num(v));
            }
        } else {
            i += 1;
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
enum Tok {
    Cmd(char),
    Num(f32),
}

fn cubic(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, out: &mut Vec<Pos2>) {
    for i in 1..=12 {
        let t = i as f32 / 12.0;
        let u = 1.0 - t;
        let p = p0.to_vec2() * (u * u * u)
            + p1.to_vec2() * (3.0 * u * u * t)
            + p2.to_vec2() * (3.0 * u * t * t)
            + p3.to_vec2() * (t * t * t);
        out.push(p.to_pos2());
    }
}

/// SVG elliptical arc (spec F.6.5) as line segments.
#[allow(clippy::too_many_arguments)]
fn arc(
    p0: Pos2,
    rx: f32,
    ry: f32,
    rot: f32,
    large: bool,
    sweep: bool,
    p1: Pos2,
    out: &mut Vec<Pos2>,
) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-4 || ry < 1e-4 {
        out.push(p1);
        return;
    }
    let phi = rot.to_radians();
    let (s, c) = phi.sin_cos();
    let dx = (p0.x - p1.x) / 2.0;
    let dy = (p0.y - p1.y) / 2.0;
    let x1 = c * dx + s * dy;
    let y1 = -s * dx + c * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = if den > 0.0 {
        (num / den).max(0.0).sqrt()
    } else {
        0.0
    };
    if large == sweep {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let cx = c * cx1 - s * cy1 + (p0.x + p1.x) / 2.0;
    let cy = s * cx1 + c * cy1 + (p0.y + p1.y) / 2.0;
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = angle(
        (x1 - cx1) / rx,
        (y1 - cy1) / ry,
        (-x1 - cx1) / rx,
        (-y1 - cy1) / ry,
    );
    let tau = std::f32::consts::TAU;
    if !sweep && dt > 0.0 {
        dt -= tau;
    } else if sweep && dt < 0.0 {
        dt += tau;
    }
    let n = ((dt.abs() / tau) * 32.0).ceil().max(2.0) as usize;
    for i in 1..=n {
        let t = t1 + dt * i as f32 / n as f32;
        let (st, ct) = t.sin_cos();
        out.push(pos2(
            cx + c * rx * ct - s * ry * st,
            cy + s * rx * ct + c * ry * st,
        ));
    }
}

fn parse(d: &str) -> Vec<Poly> {
    let toks = tokens(d);
    let mut polys: Vec<Poly> = Vec::new();
    let mut cur: Vec<Pos2> = Vec::new();
    let mut p = Pos2::ZERO;
    let mut start = Pos2::ZERO;
    let mut last_ctrl: Option<Pos2> = None;
    let mut cmd = 'M';
    let mut i = 0;
    let flush = |cur: &mut Vec<Pos2>, polys: &mut Vec<Poly>, closed: bool| {
        if !cur.is_empty() {
            polys.push((std::mem::take(cur), closed));
        }
    };
    while i < toks.len() {
        if let Tok::Cmd(c) = toks[i] {
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                flush(&mut cur, &mut polys, true);
                p = start;
                last_ctrl = None;
                continue;
            }
        }
        let n = match cmd.to_ascii_uppercase() {
            'H' | 'V' => 1,
            'M' | 'L' | 'T' => 2,
            'S' | 'Q' => 4,
            'C' => 6,
            'A' => 7,
            _ => {
                i += 1;
                continue;
            }
        };
        let mut v = [0.0f32; 7];
        let mut got = 0;
        while got < n {
            match toks.get(i) {
                Some(Tok::Num(x)) => {
                    v[got] = *x;
                    got += 1;
                    i += 1;
                }
                _ => break,
            }
        }
        if got < n {
            break;
        }
        let rel = cmd.is_ascii_lowercase();
        let o = if rel { p.to_vec2() } else { Vec2::ZERO };
        match cmd.to_ascii_uppercase() {
            'M' => {
                flush(&mut cur, &mut polys, false);
                p = pos2(v[0], v[1]) + o;
                start = p;
                cur.push(p);
                // Further pairs are line-tos.
                cmd = if rel { 'l' } else { 'L' };
                last_ctrl = None;
            }
            'L' => {
                p = pos2(v[0], v[1]) + o;
                cur.push(p);
                last_ctrl = None;
            }
            'H' => {
                p.x = if rel { p.x + v[0] } else { v[0] };
                cur.push(p);
                last_ctrl = None;
            }
            'V' => {
                p.y = if rel { p.y + v[0] } else { v[0] };
                cur.push(p);
                last_ctrl = None;
            }
            'C' => {
                let c1 = pos2(v[0], v[1]) + o;
                let c2 = pos2(v[2], v[3]) + o;
                let e = pos2(v[4], v[5]) + o;
                cubic(p, c1, c2, e, &mut cur);
                last_ctrl = Some(c2);
                p = e;
            }
            'S' => {
                let c1 = last_ctrl.map_or(p, |c| p + (p - c));
                let c2 = pos2(v[0], v[1]) + o;
                let e = pos2(v[2], v[3]) + o;
                cubic(p, c1, c2, e, &mut cur);
                last_ctrl = Some(c2);
                p = e;
            }
            'Q' => {
                let q = pos2(v[0], v[1]) + o;
                let e = pos2(v[2], v[3]) + o;
                let c1 = p + (q - p) * (2.0 / 3.0);
                let c2 = e + (q - e) * (2.0 / 3.0);
                cubic(p, c1, c2, e, &mut cur);
                last_ctrl = None;
                p = e;
            }
            'A' => {
                let e = pos2(v[5], v[6]) + o;
                arc(p, v[0], v[1], v[2], v[3] != 0.0, v[4] != 0.0, e, &mut cur);
                p = e;
                last_ctrl = None;
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut polys, false);
    polys
}

fn cache() -> &'static HashMap<&'static str, Vec<Poly>> {
    static CACHE: OnceLock<HashMap<&'static str, Vec<Poly>>> = OnceLock::new();
    CACHE.get_or_init(|| ICONS.iter().map(|(n, d)| (*n, parse(d))).collect())
}

/// Whether `name` is a known icon.
pub fn exists(name: &str) -> bool {
    cache().contains_key(name)
}

/// Paint icon `name` into `rect` (square), stroke `width` in screen points.
pub fn paint(painter: &Painter, rect: Rect, name: &str, color: Color32, width: f32) {
    let Some(polys) = cache().get(name) else {
        return;
    };
    let s = rect.width().min(rect.height()) / 24.0;
    let origin = rect.center() - vec2(12.0, 12.0) * s;
    let stroke = Stroke::new(width, color);
    for (pts, closed) in polys {
        let pts: Vec<Pos2> = pts.iter().map(|q| origin + q.to_vec2() * s).collect();
        let len: f32 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
        if len < 0.6 {
            // A dot ("h.01").
            if let Some(q) = pts.first() {
                painter.circle_filled(*q, width * 0.7, color);
            }
        } else if *closed {
            painter.add(Shape::closed_line(pts, stroke));
        } else {
            painter.add(Shape::line(pts, stroke));
            // Round the joins a little: dots on interior vertices.
        }
    }
}

/// An icon of `size` points as a widget (no interaction).
pub fn show(ui: &mut Ui, name: &str, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint(ui.painter(), rect, name, color, 1.5);
    resp
}

/// Square icon button with a hover background; `label` is the tooltip and accessible name.
pub fn button(ui: &mut Ui, name: &str, label: &str, active: bool) -> Response {
    button_sized(ui, name, label, active, 30.0)
}

pub fn button_sized(ui: &mut Ui, name: &str, label: &str, active: bool, size: f32) -> Response {
    button_named(ui, name, label, label, active, size)
}

/// Icon button whose accessible name differs from its tooltip.
pub fn button_named(
    ui: &mut Ui,
    name: &str,
    a11y: &str,
    label: &str,
    active: bool,
    size: f32,
) -> Response {
    let enabled = ui.is_enabled();
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let label_owned = a11y.to_owned();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, enabled, active, &label_owned)
    });
    let bg = if active {
        theme::pal().primary_tint
    } else if resp.hovered() && enabled {
        theme::pal().bg
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(theme::RADIUS), bg);
    let color = if !enabled {
        theme::pal().border
    } else if active {
        theme::pal().primary
    } else {
        theme::pal().text2
    };
    paint(ui.painter(), rect.shrink(size * 0.22), name, color, 1.5);
    if label.is_empty() {
        resp
    } else {
        resp.on_hover_text(label)
    }
}

/// Button with an icon before its text; `fill` None draws the plain outlined style.
pub fn text_button(
    ui: &mut Ui,
    name: &str,
    text: &str,
    fill: Option<Color32>,
    fg: Color32,
    width: Option<f32>,
) -> Response {
    let font = theme::bold(13.0);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let w = width.unwrap_or(galley.size().x + 48.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let enabled = ui.is_enabled();
    let label = text.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label));
    let hovered = resp.hovered();
    let (bg, stroke) = match fill {
        Some(c) if hovered && c == theme::pal().primary => {
            (theme::pal().primary_hover, Stroke::NONE)
        }
        Some(c) => (c, Stroke::NONE),
        None => (
            if hovered {
                theme::pal().bg
            } else {
                theme::pal().surface
            },
            Stroke::new(1.0, theme::pal().border),
        ),
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(theme::RADIUS),
        bg,
        stroke,
        egui::StrokeKind::Inside,
    );
    let total = 16.0 + 8.0 + galley.size().x;
    let x0 = rect.center().x - total / 2.0;
    paint(
        ui.painter(),
        Rect::from_center_size(pos2(x0 + 8.0, rect.center().y), vec2(16.0, 16.0)),
        name,
        fg,
        1.6,
    );
    ui.painter().galley(
        pos2(x0 + 24.0, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_parses() {
        for (name, _) in ICONS {
            let polys = &cache()[name];
            assert!(!polys.is_empty(), "{name}");
            for (pts, _) in polys {
                for p in pts {
                    assert!(p.x.is_finite() && p.y.is_finite(), "{name}");
                    assert!((-2.0..=26.0).contains(&p.x), "{name} {p:?}");
                    assert!((-2.0..=26.0).contains(&p.y), "{name} {p:?}");
                }
            }
        }
    }
}
