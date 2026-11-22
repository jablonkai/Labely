//! Read-only import of NiimBlue saved labels and templates (ADR-0043).
//!
//! A NiimBlue export is `{label, canvas: {objects}, title?, csv?}`, or an array of them. The
//! canvas is Fabric.js JSON in printer dots, and the file does not say at which dpi.

use base64::Engine as _;
use serde_json::{Map, Value};

use crate::commands::element::{MAX_IMAGE_BYTES, sha256_hex, sniff_mime, validate_element};
use crate::document::{
    Asset, BarcodeProps, Code2dProps, DataSource, Dither, Document, EccLevel, Element, ElementId,
    ElementKind, HAlign, HumanReadable, ImageFit, ImageProps, LabelSpec, LineProps, ShapeProps,
    Symbology1d, Symbology2d, TextFit, TextProps, VAlign,
};

/// Dpi assumed when the caller does not give one: NiimBlue's default 8 dots per mm.
pub const DEFAULT_DPI: f64 = 203.2;

/// Corner radius NiimBlue draws for a `rounded_rect` label, in dots.
const ROUND_RADIUS_DOTS: f64 = 10.0;

/// An imported document and what did not come across.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub doc: Document,
    pub warnings: Vec<String>,
}

/// Whether `v` looks like a NiimBlue export rather than a `.labely` document.
pub fn detect(v: &Value) -> bool {
    match v {
        Value::Array(items) => items.first().is_some_and(is_label),
        v => is_label(v),
    }
}

fn is_label(v: &Value) -> bool {
    v.get("format_version").is_none() && v.get("canvas").is_some() && v.get("label").is_some()
}

/// Convert a NiimBlue export. `fallback_name` names the document when the label has no
/// title; `dpi` converts dots to mm.
pub fn import(v: &Value, fallback_name: &str, dpi: f64) -> Result<Import, String> {
    let mut warnings = Vec::new();
    let label = match v {
        Value::Array(items) => {
            let first = items.first().ok_or("the label list is empty")?;
            if items.len() > 1 {
                warnings.push(format!(
                    "the file holds {} labels; only the first was opened",
                    items.len()
                ));
            }
            first
        }
        v => v,
    };
    let label = label.as_object().ok_or("a label must be a JSON object")?;
    let mut im = Importer {
        dpmm: dpi / 25.4,
        warnings,
    };
    let doc = im.label(label, fallback_name)?;
    Ok(Import {
        doc,
        warnings: im.warnings,
    })
}

struct Importer {
    dpmm: f64,
    warnings: Vec<String>,
}

/// Number field, or `default` when absent or not a number.
fn num(o: &Map<String, Value>, key: &str, default: f64) -> f64 {
    o.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn string<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    o.get(key).and_then(Value::as_str)
}

fn flag(o: &Map<String, Value>, key: &str) -> bool {
    o.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// A frame in dots: centre, size and angle.
#[derive(Debug, Clone, Copy)]
struct Placed {
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    angle: f64,
}

impl Importer {
    fn mm(&self, dots: f64) -> f64 {
        dots / self.dpmm
    }

    /// Font size in dots to points.
    fn pt(&self, dots: f64) -> f64 {
        self.mm(dots) * 72.0 / 25.4
    }

    fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if !self.warnings.contains(&msg) {
            self.warnings.push(msg);
        }
    }

    fn label(&mut self, o: &Map<String, Value>, fallback_name: &str) -> Result<Document, String> {
        let props = o
            .get("label")
            .and_then(Value::as_object)
            .ok_or("`label` must be an object")?;
        let size = props
            .get("size")
            .and_then(Value::as_object)
            .ok_or("`label.size` must be an object")?;
        let (w, h) = (num(size, "width", 0.0), num(size, "height", 0.0));
        if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
            return Err(format!("`label.size` must be positive, got {w}×{h}"));
        }
        let (width_mm, height_mm) = (self.mm(w), self.mm(h));
        let corner_radius_mm = match string(props, "shape") {
            Some("rounded_rect") => self.mm(ROUND_RADIUS_DOTS),
            Some("circle") => width_mm.min(height_mm) / 2.0,
            _ => 0.0,
        };
        if string(props, "split").is_some_and(|s| s != "none") {
            self.warn("the label split was dropped");
        }
        if num(props, "tailLength", 0.0) > 0.0 {
            self.warn("the cable label tail was dropped");
        }
        if string(props, "mirror").is_some_and(|s| s != "none") {
            self.warn("the label mirror setting was dropped");
        }
        if let Some(off) = props.get("offset").and_then(Value::as_object)
            && (num(off, "x", 0.0) != 0.0 || num(off, "y", 0.0) != 0.0)
        {
            self.warn("the print offset was dropped");
        }

        let name = string(o, "title")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback_name);
        let mut doc = Document::new(
            name,
            LabelSpec {
                width_mm,
                height_mm,
                corner_radius_mm,
                ..LabelSpec::default()
            },
        );

        let objects = o
            .get("canvas")
            .and_then(|c| c.get("objects"))
            .and_then(Value::as_array)
            .ok_or("`canvas.objects` must be an array")?;
        let mut elements = Vec::new();
        for obj in objects {
            let Some(obj) = obj.as_object() else {
                self.warn("a canvas object that is not a JSON object was skipped");
                continue;
            };
            elements.extend(self.object(obj, &mut doc));
        }
        for mut e in elements {
            e.id = doc.alloc_id();
            match validate_element(&e) {
                Ok(()) => {
                    if let Some(page) = doc.pages.first_mut() {
                        page.elements.push(e);
                    }
                }
                Err(err) => self.warn(format!(
                    "a {} was skipped: {}",
                    e.kind.type_name(),
                    err.message
                )),
            }
        }
        doc.prune_assets();

        if let Some(csv) = o
            .get("csv")
            .and_then(|c| c.get("data"))
            .and_then(Value::as_str)
        {
            doc.data = self.csv(csv, name)?;
        }

        // "left" prints the canvas turned 90° clockwise, so its height runs across the head.
        if string(props, "printDirection") != Some("top") {
            doc.rotate_label(true, true);
        }
        Ok(doc)
    }

    fn csv(&mut self, text: &str, name: &str) -> Result<Option<DataSource>, String> {
        let mut r = csv::ReaderBuilder::new()
            .flexible(true)
            .has_headers(true)
            .from_reader(text.as_bytes());
        let columns: Vec<String> = r
            .headers()
            .map_err(|e| format!("`csv.data`: {e}"))?
            .iter()
            .map(|s| s.trim().to_owned())
            .collect();
        if columns.iter().all(String::is_empty) {
            return Ok(None);
        }
        if columns.iter().any(|c| c == "$times") {
            self.warn("the `$times` column is kept as data; Labely does not repeat records by it");
        }
        let mut rows = Vec::new();
        for rec in r.records() {
            let rec = rec.map_err(|e| format!("`csv.data`: {e}"))?;
            rows.push(rec.iter().map(|c| c.replace("\\n", "\n")).collect());
        }
        Ok(Some(DataSource {
            origin: Some(format!("{name} (NiimBlue)")),
            columns,
            rows,
        }))
    }

    /// Centre, size and angle of a Fabric object, in dots. `stroke_in_box` adds the stroke
    /// width to the size the way Fabric does for its bounding box.
    fn place(o: &Map<String, Value>, w: f64, h: f64, stroke_in_box: bool) -> Placed {
        let (sx, sy) = (num(o, "scaleX", 1.0).abs(), num(o, "scaleY", 1.0).abs());
        let sw = if stroke_in_box {
            num(o, "strokeWidth", 0.0).max(0.0)
        } else {
            0.0
        };
        let (w, h) = if flag(o, "strokeUniform") {
            (w * sx + sw, h * sy + sw)
        } else {
            ((w + sw) * sx, (h + sw) * sy)
        };
        let angle = num(o, "angle", 0.0);
        let ox = origin(o.get("originX"), "left", "right");
        let oy = origin(o.get("originY"), "top", "bottom");
        // Fabric places the origin point at (left, top); the box turns about that point.
        let (dx, dy) = ((0.5 - ox) * w, (0.5 - oy) * h);
        let (s, c) = angle.to_radians().sin_cos();
        Placed {
            cx: num(o, "left", 0.0) + dx * c - dy * s,
            cy: num(o, "top", 0.0) + dx * s + dy * c,
            w,
            h,
            angle,
        }
    }

    fn element(&self, o: &Map<String, Value>, p: Placed, kind: ElementKind) -> Element {
        let (w, h) = (self.mm(p.w), self.mm(p.h));
        Element {
            id: ElementId(0),
            name: None,
            x_mm: self.mm(p.cx) - w / 2.0,
            y_mm: self.mm(p.cy) - h / 2.0,
            width_mm: w,
            height_mm: h,
            rotation_deg: p.angle.rem_euclid(360.0),
            locked: false,
            hidden: o.get("visible").and_then(Value::as_bool) == Some(false),
            flip_h: flag(o, "flipX"),
            flip_v: flag(o, "flipY"),
            color: Default::default(),
            group: None,
            mirror: None,
            kind,
        }
    }

    fn object(&mut self, o: &Map<String, Value>, doc: &mut Document) -> Vec<Element> {
        let ty = string(o, "type").unwrap_or_default().to_ascii_lowercase();
        let (w, h) = (num(o, "width", 0.0), num(o, "height", 0.0));
        match ty.as_str() {
            "textbox" | "text" | "fabrictext" | "itext" | "i-text" => {
                let kind = self.text(o, ty == "textbox");
                let p = Self::place(o, w, h, false);
                vec![self.element(o, p, kind)]
            }
            "qrcode" => {
                let kind = ElementKind::Code2d(Code2dProps {
                    data: self.bindings(string(o, "text").unwrap_or_default()),
                    symbology: Symbology2d::Qr,
                    ecc: match string(o, "ecl") {
                        Some("L") => EccLevel::L,
                        Some("Q") => EccLevel::Q,
                        Some("H") => EccLevel::H,
                        _ => EccLevel::M,
                    },
                });
                let p = Self::place(o, w, h, false);
                vec![self.element(o, p, kind)]
            }
            "barcode" => {
                let encoding = string(o, "encoding").unwrap_or("CODE128");
                let Some(symbology) = symbology(encoding) else {
                    self.warn(format!(
                        "a {encoding} barcode was skipped: Labely has no such symbology"
                    ));
                    return Vec::new();
                };
                let kind = ElementKind::Barcode(BarcodeProps {
                    data: self.bindings(string(o, "text").unwrap_or_default()),
                    symbology,
                    human_readable: if o.get("printText").and_then(Value::as_bool) == Some(false) {
                        HumanReadable::None
                    } else {
                        HumanReadable::Below
                    },
                    text_size_pt: self
                        .pt(num(o, "fontSize", 12.0) * num(o, "scaleY", 1.0).abs())
                        .max(1.0),
                });
                let p = Self::place(o, w, h, false);
                vec![self.element(o, p, kind)]
            }
            "rect" | "circle" | "ellipse" => {
                let (w, h) = match ty.as_str() {
                    "circle" if o.contains_key("radius") => {
                        let d = 2.0 * num(o, "radius", 0.0);
                        (d, d)
                    }
                    "ellipse" if o.contains_key("rx") => {
                        (2.0 * num(o, "rx", 0.0), 2.0 * num(o, "ry", 0.0))
                    }
                    _ => (w, h),
                };
                let p = Self::place(o, w, h, true);
                let mut shape = ShapeProps {
                    stroke_mm: self.stroke_mm(o),
                    fill: o.get("fill").is_some_and(is_ink),
                    corner_radius_mm: 0.0,
                };
                let kind = if ty == "rect" {
                    let rx = num(o, "rx", 0.0).max(num(o, "ry", 0.0));
                    shape.corner_radius_mm = self.mm(rx * num(o, "scaleX", 1.0).abs());
                    ElementKind::Rect(shape)
                } else {
                    ElementKind::Ellipse(shape)
                };
                vec![self.element(o, p, kind)]
            }
            "polyline" | "line" => self.lines(o, &ty),
            "image" | "fabricimage" => self.image(o, doc).into_iter().collect(),
            "arucomarker" => {
                self.warn("an ArUco marker was skipped");
                Vec::new()
            }
            "" => {
                self.warn("a canvas object without a type was skipped");
                Vec::new()
            }
            other => {
                self.warn(format!("a canvas object of type `{other}` was skipped"));
                Vec::new()
            }
        }
    }

    fn text(&mut self, o: &Map<String, Value>, wraps: bool) -> ElementKind {
        let size_dots = num(o, "fontSize", 40.0) * num(o, "scaleY", 1.0).abs();
        let family = string(o, "fontFamily")
            .map(|f| f.trim().trim_end_matches(" Variable").trim())
            .filter(|f| !f.is_empty())
            .unwrap_or(crate::document::DEFAULT_FONT);
        let bold = match o.get("fontWeight") {
            Some(Value::String(s)) => s == "bold" || s.parse::<u32>().is_ok_and(|n| n >= 600),
            Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n >= 600.0),
            _ => false,
        };
        let styled = match o.get("styles") {
            Some(Value::Array(a)) => !a.is_empty(),
            Some(Value::Object(m)) => !m.is_empty(),
            _ => false,
        };
        if styled {
            self.warn("per-character text styles were dropped");
        }
        ElementKind::Text(TextProps {
            text: self.bindings(string(o, "text").unwrap_or_default()),
            font_family: family.to_owned(),
            font_size_pt: self.pt(size_dots).max(1.0),
            bold,
            italic: matches!(string(o, "fontStyle"), Some("italic" | "oblique")),
            underline: flag(o, "underline"),
            align: match string(o, "textAlign") {
                Some("center") => HAlign::Center,
                Some("right" | "justify-right") => HAlign::Right,
                Some("justify" | "justify-left" | "justify-center") => HAlign::Justify,
                _ => HAlign::Left,
            },
            valign: VAlign::Top,
            line_spacing: num(o, "lineHeight", 1.16).max(0.1),
            // charSpacing is in thousandths of an em.
            letter_spacing_mm: self.mm(num(o, "charSpacing", 0.0) / 1000.0 * size_dots),
            wrap: wraps,
            fit: if flag(o, "fontAutoSize") {
                TextFit::Shrink
            } else {
                TextFit::None
            },
            ..TextProps::default()
        })
    }

    /// Outline width in mm; 0 when the stroke is absent or not dark.
    fn stroke_mm(&self, o: &Map<String, Value>) -> f64 {
        if !o.get("stroke").is_some_and(is_ink) {
            return 0.0;
        }
        let sw = num(o, "strokeWidth", 1.0).max(0.0);
        let scale = if flag(o, "strokeUniform") {
            1.0
        } else {
            num(o, "scaleX", 1.0).abs()
        };
        self.mm(sw * scale)
    }

    /// One Labely line per segment of a Fabric polyline (or legacy line).
    fn lines(&mut self, o: &Map<String, Value>, ty: &str) -> Vec<Element> {
        let points: Vec<(f64, f64)> = if ty == "line" {
            let (x1, y1) = (num(o, "x1", 0.0), num(o, "y1", 0.0));
            let (x2, y2) = (num(o, "x2", 0.0), num(o, "y2", 0.0));
            let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
            vec![(x1 - mx, y1 - my), (x2 - mx, y2 - my)]
        } else {
            let pts: Vec<(f64, f64)> = o
                .get("points")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|p| Some((p.get("x")?.as_f64()?, p.get("y")?.as_f64()?)))
                        .collect()
                })
                .unwrap_or_default();
            // Points are relative to pathOffset, the centre of their bounding box.
            let (pox, poy) = match o.get("pathOffset") {
                Some(po) => (
                    po.get("x").and_then(Value::as_f64).unwrap_or(0.0),
                    po.get("y").and_then(Value::as_f64).unwrap_or(0.0),
                ),
                None => {
                    let xs = pts.iter().map(|p| p.0);
                    let ys = pts.iter().map(|p| p.1);
                    let (x0, x1) = (
                        xs.clone().fold(f64::MAX, f64::min),
                        xs.fold(f64::MIN, f64::max),
                    );
                    let (y0, y1) = (
                        ys.clone().fold(f64::MAX, f64::min),
                        ys.fold(f64::MIN, f64::max),
                    );
                    ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
                }
            };
            pts.into_iter().map(|(x, y)| (x - pox, y - poy)).collect()
        };
        if points.len() < 2 {
            self.warn("a line with fewer than two points was skipped");
            return Vec::new();
        }
        let (w, h) = (num(o, "width", 0.0), num(o, "height", 0.0));
        let p = Self::place(o, w, h, true);
        let (sx, sy) = (num(o, "scaleX", 1.0).abs(), num(o, "scaleY", 1.0).abs());
        let (fx, fy) = (
            if flag(o, "flipX") { -1.0 } else { 1.0 },
            if flag(o, "flipY") { -1.0 } else { 1.0 },
        );
        let (s, c) = p.angle.to_radians().sin_cos();
        let world = |(x, y): (f64, f64)| {
            let (x, y) = (x * sx * fx, y * sy * fy);
            (p.cx + x * c - y * s, p.cy + x * s + y * c)
        };
        let stroke_mm = self.stroke_mm(o);
        let hidden = o.get("visible").and_then(Value::as_bool) == Some(false);
        let mut out = Vec::new();
        for seg in points.windows(2) {
            let (a, b) = (world(seg[0]), world(seg[1]));
            if a == b {
                continue;
            }
            out.push(Element {
                id: ElementId(0),
                name: None,
                x_mm: self.mm(a.0),
                y_mm: self.mm(a.1),
                width_mm: self.mm(b.0 - a.0),
                height_mm: self.mm(b.1 - a.1),
                rotation_deg: 0.0,
                locked: false,
                hidden,
                flip_h: false,
                flip_v: false,
                color: Default::default(),
                group: None,
                mirror: None,
                kind: ElementKind::Line(LineProps {
                    stroke_mm,
                    dash_mm: None,
                }),
            });
        }
        out
    }

    fn image(&mut self, o: &Map<String, Value>, doc: &mut Document) -> Option<Element> {
        let src = string(o, "src").unwrap_or_default();
        let Some((header, payload)) = src.strip_prefix("data:").and_then(|s| s.split_once(','))
        else {
            self.warn("an image that is not embedded in the file was skipped");
            return None;
        };
        if !header.ends_with(";base64") {
            self.warn("an image that is not base64-encoded was skipped");
            return None;
        }
        let bytes = match base64::engine::general_purpose::STANDARD.decode(payload.trim()) {
            Ok(b) if !b.is_empty() && b.len() <= MAX_IMAGE_BYTES => b,
            Ok(_) => {
                self.warn("an empty or too large image was skipped");
                return None;
            }
            Err(_) => {
                self.warn("an image with invalid base64 was skipped");
                return None;
            }
        };
        let Some(mime) = sniff_mime(&bytes) else {
            self.warn("an image in an unsupported format was skipped");
            return None;
        };
        let key = sha256_hex(&bytes);
        doc.assets.entry(key.clone()).or_insert_with(|| Asset {
            mime: mime.to_owned(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        });
        if o.get("filters")
            .and_then(Value::as_array)
            .is_some_and(|f| !f.is_empty())
        {
            self.warn("image filters were dropped");
        }
        let p = Self::place(o, num(o, "width", 0.0), num(o, "height", 0.0), false);
        Some(self.element(
            o,
            p,
            ElementKind::Image(ImageProps {
                asset: key,
                fit: ImageFit::Stretch,
                dither: Dither::default(),
                threshold: 128,
                brightness: 0.0,
                contrast: 0.0,
                invert: false,
                crop: None,
            }),
        ))
    }

    /// NiimBlue `{column}` → `{{column}}` and `{dt±Nu|FORMAT}` → `{{now±Nu:%…}}`. Anything
    /// else stays as written.
    fn bindings(&mut self, s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else {
                out.push_str(&rest[open..]);
                return out;
            };
            let inner = &after[..close];
            match self.binding(inner) {
                Some(b) => out.push_str(&b),
                None => {
                    out.push('{');
                    out.push_str(inner);
                    out.push('}');
                }
            }
            rest = &after[close + 1..];
        }
        out.push_str(rest);
        out
    }

    fn binding(&mut self, inner: &str) -> Option<String> {
        let (head, format) = match inner.split_once('|') {
            Some((h, f)) => (h.trim(), Some(f.trim())),
            None => (inner.trim(), None),
        };
        let is_word =
            |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if let Some(name) = head.strip_prefix('$') {
            if is_word(name) {
                self.warn(format!(
                    "`{{${name}}}` has no Labely equivalent and was left as text"
                ));
            }
            return None;
        }
        let word_end = head
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(head.len());
        let (name, offset) = head.split_at(word_end);
        if name == "dt" {
            let offset_ok = offset.is_empty()
                || (offset.len() >= 3
                    && matches!(offset.as_bytes()[0], b'+' | b'-')
                    && offset[1..offset.len() - 1]
                        .bytes()
                        .all(|b| b.is_ascii_digit())
                    && offset.ends_with(['y', 'M', 'w', 'd', 'h', 'm', 's']));
            if !offset_ok {
                return None;
            }
            let fmt = dayjs_to_strftime(format.unwrap_or("YYYY-MM-DD HH:mm:ss"));
            return Some(format!("{{{{now{offset}:{fmt}}}}}"));
        }
        if !is_word(name) || !offset.is_empty() || format.is_some() {
            return None;
        }
        Some(format!("{{{{{name}}}}}"))
    }
}

/// Fabric origin as a fraction of the box: `lo` 0, `center` 0.5, `hi` 1, or a number.
fn origin(v: Option<&Value>, lo: &str, hi: &str) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) if s == "center" => 0.5,
        Some(Value::String(s)) if s == hi => 1.0,
        Some(Value::String(s)) if s == lo => 0.0,
        _ => 0.0,
    }
}

fn symbology(encoding: &str) -> Option<Symbology1d> {
    Some(match encoding.to_ascii_uppercase().as_str() {
        "CODE128" => Symbology1d::Code128,
        "CODE128A" => Symbology1d::Code128A,
        "CODE128B" => Symbology1d::Code128B,
        "CODE128C" => Symbology1d::Code128C,
        "CODE39" => Symbology1d::Code39,
        "CODE93" => Symbology1d::Code93,
        "EAN13" => Symbology1d::Ean13,
        "EAN8" => Symbology1d::Ean8,
        "UPC" | "UPCA" => Symbology1d::UpcA,
        "UPCE" => Symbology1d::UpcE,
        "ITF" | "ITF14" => Symbology1d::Itf,
        "CODABAR" => Symbology1d::Codabar,
        _ => return None,
    })
}

/// Whether a Fabric colour would print: not absent, transparent or light.
fn is_ink(v: &Value) -> bool {
    let s = match v {
        Value::Null => return false,
        Value::String(s) => s.trim().to_ascii_lowercase(),
        // Gradients and patterns are objects; count them as ink.
        _ => return true,
    };
    let rgba = match s.as_str() {
        "" | "none" | "transparent" => return false,
        "white" => (255.0, 255.0, 255.0, 1.0),
        "black" => (0.0, 0.0, 0.0, 1.0),
        s if s.starts_with('#') => match hex_rgba(&s[1..]) {
            Some(c) => c,
            None => return true,
        },
        s if s.starts_with("rgb") => {
            let inner = s
                .trim_start_matches("rgba")
                .trim_start_matches("rgb")
                .trim_matches(|c| c == '(' || c == ')' || c == ' ');
            let parts: Vec<f64> = inner
                .split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            match parts.as_slice() {
                [r, g, b] => (*r, *g, *b, 1.0),
                [r, g, b, a] => (*r, *g, *b, *a),
                _ => return true,
            }
        }
        _ => return true,
    };
    let (r, g, b, a) = rgba;
    a > 0.0 && (0.299 * r + 0.587 * g + 0.114 * b) < 128.0
}

fn hex_rgba(h: &str) -> Option<(f64, f64, f64, f64)> {
    let digit = |i: usize, n: usize| -> Option<f64> {
        let v = u8::from_str_radix(h.get(i..i + n)?, 16).ok()?;
        Some(if n == 1 {
            f64::from(v * 17)
        } else {
            f64::from(v)
        })
    };
    match h.len() {
        3 | 4 => Some((
            digit(0, 1)?,
            digit(1, 1)?,
            digit(2, 1)?,
            if h.len() == 4 {
                digit(3, 1)? / 255.0
            } else {
                1.0
            },
        )),
        6 | 8 => Some((
            digit(0, 2)?,
            digit(2, 2)?,
            digit(4, 2)?,
            if h.len() == 8 {
                digit(6, 2)? / 255.0
            } else {
                1.0
            },
        )),
        _ => None,
    }
}

/// Day.js format tokens to chrono `strftime`. `[literal]` is copied; other letters too.
fn dayjs_to_strftime(f: &str) -> String {
    const TOKENS: &[(&str, &str)] = &[
        ("YYYY", "%Y"),
        ("YY", "%y"),
        ("MMMM", "%B"),
        ("MMM", "%b"),
        ("MM", "%m"),
        ("M", "%-m"),
        ("DD", "%d"),
        ("D", "%-d"),
        ("dddd", "%A"),
        ("ddd", "%a"),
        ("HH", "%H"),
        ("H", "%-H"),
        ("hh", "%I"),
        ("h", "%-I"),
        ("mm", "%M"),
        ("m", "%-M"),
        ("ss", "%S"),
        ("s", "%-S"),
        ("A", "%p"),
        ("a", "%P"),
    ];
    let mut out = String::new();
    let mut rest = f;
    'outer: while let Some(ch) = rest.chars().next() {
        if ch == '['
            && let Some(end) = rest.find(']')
        {
            out.push_str(&rest[1..end].replace('%', "%%"));
            rest = &rest[end + 1..];
            continue;
        }
        for (tok, rep) in TOKENS {
            if let Some(r) = rest.strip_prefix(tok) {
                out.push_str(rep);
                rest = r;
                continue 'outer;
            }
        }
        if ch == '%' {
            out.push_str("%%");
        } else if ch == '}' {
            // A `}` would end the Labely binding early.
            out.push(')');
        } else {
            out.push(ch);
        }
        rest = &rest[ch.len_utf8()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn importer() -> Importer {
        Importer {
            dpmm: 8.0,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn bindings_and_dates() {
        let mut im = importer();
        assert_eq!(im.bindings("Hi {name}!"), "Hi {{name}}!");
        assert_eq!(im.bindings("{ code }"), "{{code}}");
        assert_eq!(im.bindings("{dt}"), "{{now:%Y-%m-%d %H:%M:%S}}");
        assert_eq!(im.bindings("{dt+3d|DD.MM.YYYY}"), "{{now+3d:%d.%m.%Y}}");
        assert_eq!(im.bindings("{dt-1M|[Q]YY}"), "{{now-1M:Q%y}}");
        assert_eq!(im.bindings("{$times} {a b} {"), "{$times} {a b} {");
        assert_eq!(im.warnings.len(), 1, "{:?}", im.warnings);
        assert_eq!(im.bindings("{x+1d}"), "{x+1d}");
    }

    #[test]
    fn colours() {
        for ink in [
            json!("black"),
            json!("#000"),
            json!("rgb(0,0,0)"),
            json!("#333333ff"),
        ] {
            assert!(is_ink(&ink), "{ink}");
        }
        for none in [
            json!(null),
            json!("transparent"),
            json!("#fff"),
            json!("white"),
            json!("rgba(0,0,0,0)"),
            json!("#00000000"),
        ] {
            assert!(!is_ink(&none), "{none}");
        }
    }

    #[test]
    fn placement_follows_fabric_origin_and_angle() {
        let o = json!({"left": 100, "top": 50, "originX": "center", "originY": "center"});
        let p = Importer::place(o.as_object().unwrap_or(&Map::new()), 40.0, 20.0, false);
        assert_eq!((p.cx, p.cy, p.w, p.h), (100.0, 50.0, 40.0, 20.0));
        // Top-left origin turned 90°: the centre swings below the origin point.
        let o = json!({"left": 0, "top": 0, "angle": 90, "scaleX": 2});
        let p = Importer::place(o.as_object().unwrap_or(&Map::new()), 10.0, 10.0, false);
        assert!(
            (p.cx - -5.0).abs() < 1e-9 && (p.cy - 10.0).abs() < 1e-9,
            "{p:?}"
        );
        assert_eq!((p.w, p.h), (20.0, 10.0));
    }

    #[test]
    fn detection() {
        assert!(detect(&json!({"label": {}, "canvas": {}})));
        assert!(detect(&json!([{"label": {}, "canvas": {}}])));
        assert!(!detect(
            &json!({"format_version": 2, "label": {}, "canvas": {}})
        ));
        assert!(!detect(&json!([])));
    }
}
