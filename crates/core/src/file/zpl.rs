//! Read-only, offline import of a ZPL II subset (ADR-0044).
//!
//! Only the first `^XA … ^XZ` format is read. Coordinates are printer dots and the file does
//! not say at which dpi. Every command that does not come across is named in a warning.

use base64::Engine as _;

use super::Import;
use crate::commands::element::{MAX_IMAGE_BYTES, sha256_hex, validate_element};
use crate::document::{
    Asset, BarcodeProps, Code2dProps, DEFAULT_FONT, Dither, Document, EccLevel, Element, ElementId,
    ElementKind, HAlign, HumanReadable, ImageFit, ImageProps, LabelSpec, LineProps, MediaType,
    ShapeProps, Symbology1d, Symbology2d, TextFit, TextProps,
};

/// Line height of Labely text as a multiple of its font size (see `labely-render`).
const LINE_HEIGHT: f64 = 1.2;

/// Settings that do not change the layout, ignored without a warning.
const IGNORED: &[&str] = &[
    "FX", "MD", "MM", "MT", "MF", "MW", "PR", "PQ", "PM", "JM", "JU", "JZ", "SZ", "XB", "TA", "SD",
    "JS",
];

/// Whether `bytes` look like ZPL: not JSON, and holding a `^XA`.
pub fn detect(bytes: &[u8]) -> bool {
    let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match b.iter().find(|c| !c.is_ascii_whitespace()) {
        None | Some(b'{' | b'[') => false,
        Some(_) => b.windows(3).any(|w| {
            w[0] == b'^' && w[1].eq_ignore_ascii_case(&b'X') && w[2].eq_ignore_ascii_case(&b'A')
        }),
    }
}

/// Convert the first label format of a ZPL file. `name` names the document; `dpi` converts
/// dots to mm.
pub fn import(bytes: &[u8], name: &str, dpi: f64) -> Result<Import, String> {
    let mut z = Zpl::new(dpi);
    for t in tokenize(bytes) {
        z.command(&t);
    }
    if z.formats == 0 {
        return Err("no `^XA … ^XZ` label format found".into());
    }
    if z.formats > 1 {
        let n = z.formats;
        z.warn(format!(
            "the file holds {n} labels; only the first was opened"
        ));
    }
    z.finish(name)
}

/// One command: `^` or `~`, its name, and the raw bytes up to the next command.
#[derive(Debug)]
struct Token<'a> {
    prefix: u8,
    name: String,
    params: &'a [u8],
}

fn tokenize(b: &[u8]) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let prefix = b[i];
        if prefix != b'^' && prefix != b'~' {
            i += 1;
            continue;
        }
        let Some(n1) = b.get(i + 1).map(u8::to_ascii_uppercase) else {
            break;
        };
        // `^A` is the only command named by one letter; the font name follows it.
        let (name, start) = if prefix == b'^' && n1 == b'A' {
            ("A".to_owned(), i + 2)
        } else {
            let Some(n2) = b.get(i + 2).map(u8::to_ascii_uppercase) else {
                break;
            };
            (String::from_utf8_lossy(&[n1, n2]).into_owned(), i + 3)
        };
        // Field data may hold a `~`; only a `^` ends it.
        let caret_only = matches!(name.as_str(), "FD" | "FV" | "FX");
        let binary_end = (name == "GF").then(|| gf_binary_end(b, start)).flatten();
        let end = binary_end.unwrap_or_else(|| {
            b[start..]
                .iter()
                .position(|&c| c == b'^' || (!caret_only && c == b'~'))
                .map_or(b.len(), |p| start + p)
        });
        out.push(Token {
            prefix,
            name,
            params: &b[start..end],
        });
        i = end;
    }
    out
}

/// End of a binary (`B` or `C`) `^GF`, whose data may hold any byte.
fn gf_binary_end(b: &[u8], start: usize) -> Option<usize> {
    let format = b.get(start)?.to_ascii_uppercase();
    if format != b'B' && format != b'C' {
        return None;
    }
    let mut commas = Vec::new();
    for (k, &c) in b.get(start..)?.iter().enumerate().take(64) {
        if c == b',' {
            commas.push(start + k);
            if commas.len() == 4 {
                break;
            }
        }
    }
    let (&c1, &c2, &c4) = (commas.first()?, commas.get(1)?, commas.get(3)?);
    let total: usize = std::str::from_utf8(b.get(c1 + 1..c2)?)
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some((c4 + 1).saturating_add(total).min(b.len()))
}

/// Params as text, with the CR and LF that ZPL ignores removed.
fn clean(p: &[u8]) -> Vec<u8> {
    p.iter()
        .copied()
        .filter(|&c| c != b'\r' && c != b'\n')
        .collect()
}

fn text_params(p: &[u8]) -> String {
    String::from_utf8_lossy(&clean(p)).into_owned()
}

/// Comma-separated params; an empty or unparsable one is `None`.
fn nums(p: &str) -> Vec<Option<f64>> {
    p.split(',')
        .map(|s| s.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect()
}

fn nth(v: &[Option<f64>], i: usize) -> Option<f64> {
    v.get(i).copied().flatten()
}

/// A positive param, else `None` (ZPL treats 0 as "use the default").
fn positive(v: &[Option<f64>], i: usize) -> Option<f64> {
    nth(v, i).filter(|&x| x > 0.0)
}

fn part(p: &str, i: usize) -> Option<&str> {
    p.split(',').nth(i).map(str::trim).filter(|s| !s.is_empty())
}

fn letter(s: Option<&str>) -> Option<u8> {
    s.and_then(|s| s.bytes().next())
        .map(|c| c.to_ascii_uppercase())
}

/// `N`, `R`, `I`, `B` to a clockwise angle.
fn orientation(s: Option<&str>, default: f64) -> f64 {
    match letter(s) {
        Some(b'N') => 0.0,
        Some(b'R') => 90.0,
        Some(b'I') => 180.0,
        Some(b'B') => 270.0,
        _ => default,
    }
}

fn yes_no(s: Option<&str>, default: bool) -> bool {
    match letter(s) {
        Some(b'Y') => true,
        Some(b'N') => false,
        _ => default,
    }
}

/// A resolved font: name and cell size in dots.
#[derive(Debug, Clone, Copy)]
struct Font {
    name: u8,
    height: f64,
    width: f64,
}

impl Font {
    /// Bitmap fonts `A`–`H` have fixed cells; the others scale.
    fn bitmap(name: u8) -> bool {
        matches!(name, b'A'..=b'H')
    }

    fn base(name: u8) -> (f64, f64) {
        match name {
            b'A' => (9.0, 5.0),
            b'B' => (11.0, 7.0),
            b'C' | b'D' => (18.0, 10.0),
            b'E' => (28.0, 15.0),
            b'F' => (26.0, 13.0),
            b'G' => (60.0, 40.0),
            b'H' => (21.0, 13.0),
            _ => (15.0, 12.0),
        }
    }

    fn resolve(name: u8, h: Option<f64>, w: Option<f64>) -> Self {
        let (bh, bw) = Self::base(name);
        let scalable = !Self::bitmap(name);
        let (height, width) = match (h, w) {
            (Some(h), Some(w)) => (h, w),
            (Some(h), None) => (h, if scalable { h } else { bw * h / bh }),
            (None, Some(w)) => (if scalable { w } else { bh * w / bw }, w),
            (None, None) => (bh, bw),
        };
        Self {
            name,
            height,
            width,
        }
    }
}

/// `^FB` text block.
#[derive(Debug, Clone, Copy)]
struct Block {
    width: f64,
    lines: f64,
    spacing: f64,
    align: HAlign,
}

/// A barcode command waiting for its `^FD`.
#[derive(Debug, Clone, Copy)]
enum Code {
    Code128 {
        rot: f64,
        height: f64,
        hr: HumanReadable,
        gs1: bool,
    },
    Code39 {
        rot: f64,
        height: f64,
        hr: HumanReadable,
    },
    /// EAN-13 (`^BE`) or UPC-A (`^BU`): 95 modules.
    Ean {
        symbology: Symbology1d,
        rot: f64,
        height: f64,
        hr: HumanReadable,
    },
    Qr {
        magnification: f64,
        ecc: EccLevel,
    },
    DataMatrix {
        rot: f64,
        module: f64,
        cols: Option<f64>,
    },
}

/// What the current field (`^FO … ^FS`) has collected.
#[derive(Debug, Default)]
struct Field {
    /// x, y and whether it is a `^FT` (bottom-left) origin.
    origin: Option<(f64, f64, bool)>,
    font: Option<(Font, f64)>,
    block: Option<Block>,
    hex: Option<u8>,
    reverse: bool,
    number: bool,
    code: Option<Code>,
    data: Option<Vec<u8>>,
}

#[derive(Debug)]
struct Zpl {
    dpi: f64,
    dpmm: f64,
    warnings: Vec<String>,
    formats: usize,
    active: bool,
    home: (f64, f64),
    width: Option<f64>,
    length: Option<f64>,
    media: Option<MediaType>,
    font: Font,
    rot: f64,
    module: f64,
    ratio: f64,
    bar_height: f64,
    utf8: bool,
    field: Field,
    elements: Vec<Element>,
    assets: Vec<(String, Asset)>,
}

impl Zpl {
    fn new(dpi: f64) -> Self {
        Self {
            dpi,
            dpmm: dpi / 25.4,
            warnings: Vec::new(),
            formats: 0,
            active: false,
            home: (0.0, 0.0),
            width: None,
            length: None,
            media: None,
            font: Font::resolve(b'A', None, None),
            rot: 0.0,
            module: 2.0,
            ratio: 3.0,
            bar_height: 10.0,
            utf8: false,
            field: Field::default(),
            elements: Vec::new(),
            assets: Vec::new(),
        }
    }

    fn mm(&self, dots: f64) -> f64 {
        dots / self.dpmm
    }

    /// A height in dots to the point size whose Labely line is that high.
    fn pt(&self, dots: f64) -> f64 {
        (self.mm(dots / LINE_HEIGHT) * 72.0 / 25.4).max(1.0)
    }

    fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if !self.warnings.contains(&msg) {
            self.warnings.push(msg);
        }
    }

    fn unsupported(&mut self, t: &Token<'_>) {
        let prefix = char::from(t.prefix);
        self.warn(format!(
            "`{prefix}{}` is not supported and was skipped",
            t.name
        ));
    }

    fn command(&mut self, t: &Token<'_>) {
        match t.name.as_str() {
            "XA" => {
                self.formats += 1;
                self.active = self.formats == 1;
                return;
            }
            "XZ" => {
                self.active = false;
                self.field = Field::default();
                return;
            }
            _ if !self.active => return,
            _ => {}
        }
        let p = text_params(t.params);
        let v = nums(&p);
        match t.name.as_str() {
            n if IGNORED.contains(&n) => {}
            "LH" => self.home = (nth(&v, 0).unwrap_or(0.0), nth(&v, 1).unwrap_or(0.0)),
            "PW" => self.width = positive(&v, 0),
            "LL" => self.length = positive(&v, 0),
            "MN" => {
                self.media = match letter(part(&p, 0)) {
                    Some(b'Y' | b'W') => Some(MediaType::Gap),
                    Some(b'M') => Some(MediaType::BlackMark),
                    Some(b'N') => Some(MediaType::Continuous),
                    _ => self.media,
                }
            }
            "MU" => {
                if letter(part(&p, 0)).is_some_and(|u| u != b'D') {
                    self.warn(
                        "`^MU` units other than dots are not supported; values were read as dots",
                    );
                }
            }
            "CI" => self.utf8 = part(&p, 0) == Some("28"),
            "CF" => {
                let name = p
                    .bytes()
                    .next()
                    .filter(|c| *c != b',')
                    .map_or(self.font.name, |c| c.to_ascii_uppercase());
                self.font = Font::resolve(name, positive(&v, 1), positive(&v, 2));
            }
            "FW" => self.rot = orientation(part(&p, 0), self.rot),
            "BY" => {
                self.module = positive(&v, 0).unwrap_or(self.module);
                self.ratio = positive(&v, 1).unwrap_or(self.ratio);
                self.bar_height = positive(&v, 2).unwrap_or(self.bar_height);
            }
            "FO" | "FT" => {
                self.field.origin = Some((
                    nth(&v, 0).unwrap_or(0.0),
                    nth(&v, 1).unwrap_or(0.0),
                    t.name == "FT",
                ));
            }
            "A" => self.field_font(&p),
            "FB" => {
                self.field.block = Some(Block {
                    width: positive(&v, 0).unwrap_or(0.0),
                    lines: positive(&v, 1).unwrap_or(1.0).round().max(1.0),
                    spacing: nth(&v, 2).unwrap_or(0.0),
                    align: match letter(part(&p, 3)) {
                        Some(b'C') => HAlign::Center,
                        Some(b'R') => HAlign::Right,
                        Some(b'J') => HAlign::Justify,
                        _ => HAlign::Left,
                    },
                });
            }
            "FH" => self.field.hex = Some(clean(t.params).first().copied().unwrap_or(b'_')),
            "FR" => self.field.reverse = true,
            "FN" => self.field.number = true,
            "FD" | "FV" => self.field.data = Some(clean(t.params)),
            "SN" => {
                self.warn("serial numbers (`^SN`) were imported as their start value");
                self.field.data = Some(part(&p, 0).unwrap_or_default().as_bytes().to_vec());
            }
            "BC" | "B3" | "BE" | "BU" | "BQ" | "BX" => {
                self.field.code = Some(self.code(&t.name, &p, &v))
            }
            "GB" => self.graphic_box(&p, &v),
            "GC" => {
                let d = positive(&v, 0).unwrap_or(3.0);
                self.ellipse(d, d, positive(&v, 1).unwrap_or(1.0), part(&p, 2));
            }
            "GE" => {
                let t = positive(&v, 2).unwrap_or(1.0);
                let (w, h) = (positive(&v, 0).unwrap_or(t), positive(&v, 1).unwrap_or(t));
                self.ellipse(w, h, t, part(&p, 3));
            }
            "GD" => self.diagonal(&p, &v),
            "GF" => self.graphic_field(t.params),
            "FS" => self.end_field(),
            "PO" => {
                if letter(part(&p, 0)) == Some(b'I') {
                    self.warn("the inverted print orientation (`^POI`) was dropped");
                }
            }
            "LR" => {
                if letter(part(&p, 0)) == Some(b'Y') {
                    self.warn("reverse printing of the whole label (`^LRY`) was dropped");
                }
            }
            "LT" | "LS" => {
                if nth(&v, 0).is_some_and(|x| x != 0.0) {
                    let name = &t.name;
                    self.warn(format!("the label shift (`^{name}`) was dropped"));
                }
            }
            _ => self.unsupported(t),
        }
    }

    /// `^Afo,h,w`: font name, then orientation, height and width.
    fn field_font(&mut self, p: &str) {
        let mut chars = p.chars();
        let name = chars.next().map_or(b'0', |c| {
            u8::try_from(c.to_ascii_uppercase()).unwrap_or(b'0')
        });
        let rest = chars.as_str();
        let v = nums(rest);
        let font = Font::resolve(name, positive(&v, 1), positive(&v, 2));
        let rot = orientation(part(rest, 0), self.rot);
        self.field.font = Some((font, rot));
    }

    fn code(&mut self, name: &str, p: &str, v: &[Option<f64>]) -> Code {
        let rot = orientation(part(p, 0), self.rot);
        let hr = |f: usize, g: usize| {
            if !yes_no(part(p, f), true) {
                HumanReadable::None
            } else if yes_no(part(p, g), false) {
                HumanReadable::Above
            } else {
                HumanReadable::Below
            }
        };
        match name {
            "BC" => Code::Code128 {
                rot,
                height: positive(v, 1).unwrap_or(self.bar_height),
                hr: hr(2, 3),
                gs1: letter(part(p, 5)) == Some(b'D'),
            },
            "B3" => Code::Code39 {
                rot,
                height: positive(v, 2).unwrap_or(self.bar_height),
                hr: hr(3, 4),
            },
            "BE" | "BU" => Code::Ean {
                symbology: if name == "BE" {
                    Symbology1d::Ean13
                } else {
                    Symbology1d::UpcA
                },
                rot,
                height: positive(v, 1).unwrap_or(self.bar_height),
                hr: hr(2, 3),
            },
            "BQ" => Code::Qr {
                magnification: positive(v, 2).unwrap_or(match self.dpi {
                    d if d < 175.0 => 1.0,
                    d if d < 250.0 => 2.0,
                    d if d < 450.0 => 3.0,
                    _ => 6.0,
                }),
                ecc: ecc(letter(part(p, 3))).unwrap_or(EccLevel::Q),
            },
            _ => {
                if nth(v, 2).is_some_and(|q| q != 200.0) {
                    self.warn("Data Matrix ECC 000–140 was imported as ECC 200");
                }
                Code::DataMatrix {
                    rot,
                    module: positive(v, 1).unwrap_or(self.module),
                    cols: positive(v, 3),
                }
            }
        }
    }

    /// Top-left of the unturned frame (`w`×`h` dots, turned `rot` about its centre) placed at
    /// the field origin: the top-left of its bounding box for `^FO`, the turned bottom-left
    /// corner for `^FT`.
    fn place(&self, w: f64, h: f64, rot: f64) -> (f64, f64) {
        let (ox, oy, typeset) = self.field.origin.unwrap_or((0.0, 0.0, false));
        let (ox, oy) = (ox + self.home.0, oy + self.home.1);
        let (s, c) = rot.to_radians().sin_cos();
        let (cx, cy) = if typeset {
            let (dx, dy) = (-w / 2.0, h / 2.0);
            (ox - (dx * c - dy * s), oy - (dx * s + dy * c))
        } else {
            let bw = (w * c).abs() + (h * s).abs();
            let bh = (w * s).abs() + (h * c).abs();
            (ox + bw / 2.0, oy + bh / 2.0)
        };
        (cx - w / 2.0, cy - h / 2.0)
    }

    fn frame(&self, x: f64, y: f64, w: f64, h: f64, rot: f64, kind: ElementKind) -> Element {
        Element {
            id: ElementId(0),
            name: None,
            x_mm: self.mm(x),
            y_mm: self.mm(y),
            width_mm: self.mm(w),
            height_mm: self.mm(h),
            rotation_deg: rot.rem_euclid(360.0),
            locked: false,
            hidden: false,
            flip_h: false,
            flip_v: false,
            color: Default::default(),
            group: None,
            mirror: None,
            kind,
        }
    }

    fn push(&mut self, w: f64, h: f64, rot: f64, kind: ElementKind) {
        let (x, y) = self.place(w, h, rot);
        let e = self.frame(x, y, w, h, rot, kind);
        self.elements.push(e);
    }

    fn graphic_reverse(&mut self) {
        if self.field.reverse {
            self.warn("reverse printing (`^FR`) of a graphic was dropped");
        }
    }

    fn white(&mut self, color: Option<&str>, what: &str) -> bool {
        let white = letter(color) == Some(b'W');
        if white {
            self.warn(format!("a white {what} was skipped"));
        }
        white
    }

    /// `^GBw,h,t,c,r`
    fn graphic_box(&mut self, p: &str, v: &[Option<f64>]) {
        self.graphic_reverse();
        if self.white(part(p, 3), "box (`^GB`)") {
            return;
        }
        let t = positive(v, 2).unwrap_or(1.0);
        let (w, h) = (
            positive(v, 0).unwrap_or(t).max(t),
            positive(v, 1).unwrap_or(t).max(t),
        );
        let fill = 2.0 * t >= w.min(h);
        let r = nth(v, 4).unwrap_or(0.0).clamp(0.0, 8.0);
        let shape = ShapeProps {
            stroke_mm: if fill { 0.0 } else { self.mm(t) },
            fill,
            corner_radius_mm: self.mm(r / 8.0 * w.min(h) / 2.0),
        };
        self.push(w, h, 0.0, ElementKind::Rect(shape));
    }

    fn ellipse(&mut self, w: f64, h: f64, t: f64, color: Option<&str>) {
        self.graphic_reverse();
        if self.white(color, "circle or ellipse") {
            return;
        }
        let fill = 2.0 * t >= w.min(h);
        let shape = ShapeProps {
            stroke_mm: if fill { 0.0 } else { self.mm(t) },
            fill,
            corner_radius_mm: 0.0,
        };
        self.push(w, h, 0.0, ElementKind::Ellipse(shape));
    }

    /// `^GDw,h,t,c,o`: a diagonal from corner to corner of the box.
    fn diagonal(&mut self, p: &str, v: &[Option<f64>]) {
        self.graphic_reverse();
        if self.white(part(p, 3), "diagonal line (`^GD`)") {
            return;
        }
        let t = positive(v, 2).unwrap_or(1.0);
        let (w, h) = (positive(v, 0).unwrap_or(t), positive(v, 1).unwrap_or(t));
        let (ox, oy, _) = self.field.origin.unwrap_or((0.0, 0.0, false));
        let (ox, oy) = (ox + self.home.0, oy + self.home.1);
        // `R` (the default) leans right: bottom-left to top-right.
        let (y, dy) = if letter(part(p, 4)) == Some(b'L') || part(p, 4) == Some("\\") {
            (oy, h)
        } else {
            (oy + h, -h)
        };
        let kind = ElementKind::Line(LineProps {
            stroke_mm: self.mm(t),
            dash_mm: None,
        });
        let e = self.frame(ox, y, w, dy, 0.0, kind);
        self.elements.push(e);
    }

    /// `^GFa,b,c,d,data`
    fn graphic_field(&mut self, raw: &[u8]) {
        self.graphic_reverse();
        let mut parts = raw.splitn(5, |&c| c == b',');
        let format = parts
            .next()
            .and_then(|a| clean(a).first().copied())
            .map_or(b'A', |c| c.to_ascii_uppercase());
        let num = |b: Option<&[u8]>| -> usize {
            b.and_then(|b| String::from_utf8_lossy(&clean(b)).trim().parse().ok())
                .unwrap_or(0)
        };
        let total = num(parts.next());
        let _field = parts.next();
        let row = num(parts.next());
        let data = parts.next().unwrap_or_default();
        if total == 0 || row == 0 {
            self.warn("a `^GF` graphic without a size was skipped");
            return;
        }
        if total > MAX_IMAGE_BYTES {
            self.warn("a too large `^GF` graphic was skipped");
            return;
        }
        let bits = match format {
            b'A' => match decode_ascii(data, row, total) {
                Ok(b) => b,
                Err(e) => {
                    self.warn(format!("a `^GF` graphic was skipped: {e}"));
                    return;
                }
            },
            b'B' => data.to_vec(),
            _ => {
                self.warn("a compressed binary `^GFC` graphic was skipped");
                return;
            }
        };
        let rows = total.div_ceil(row);
        let mut bits = bits;
        bits.resize(rows * row, 0);
        let bmp = bmp_1bit(&bits, row, rows);
        let key = sha256_hex(&bmp);
        if !self.assets.iter().any(|(k, _)| *k == key) {
            self.assets.push((
                key.clone(),
                Asset {
                    mime: "image/bmp".to_owned(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(&bmp),
                },
            ));
        }
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = ((row * 8) as f64, rows as f64);
        self.push(
            w,
            h,
            0.0,
            ElementKind::Image(ImageProps {
                asset: key,
                fit: ImageFit::Stretch,
                dither: Dither::Threshold,
                threshold: 128,
                brightness: 0.0,
                contrast: 0.0,
                invert: false,
                crop: None,
            }),
        );
    }

    fn decode(&self, b: &[u8]) -> String {
        match std::str::from_utf8(b) {
            Ok(s) => s.to_owned(),
            Err(_) if self.utf8 => String::from_utf8_lossy(b).into_owned(),
            // Code page 850 and 1252 agree with Latin-1 on the common letters.
            Err(_) => b.iter().map(|&c| char::from(c)).collect(),
        }
    }

    /// `^FS`: turn the collected field into an element.
    fn end_field(&mut self) {
        let Some(raw) = self.field.data.take() else {
            if self.field.code.is_some() {
                self.warn("a barcode without `^FD` data was skipped");
            }
            self.field = Field::default();
            return;
        };
        if self.field.number {
            self.warn("field variables (`^FN`) were imported as their default data");
        }
        let bytes = match self.field.hex {
            Some(esc) => unhex(&raw, esc),
            None => raw,
        };
        let data = self.decode(&bytes);
        match self.field.code {
            None => self.text(data),
            Some(code) => {
                if self.field.reverse {
                    self.warn("reverse printing (`^FR`) of a barcode was dropped");
                }
                self.barcode(code, data);
            }
        }
        self.field = Field::default();
    }

    fn text(&mut self, data: String) {
        if data.trim().is_empty() {
            return;
        }
        let (font, rot) = self.field.font.unwrap_or((self.font, self.rot));
        let h = font.height;
        let mut props = TextProps {
            font_family: DEFAULT_FONT.to_owned(),
            font_size_pt: self.pt(h),
            // Font 0 is CG Triumvirate Bold Condensed.
            bold: font.name == b'0',
            invert: self.field.reverse,
            ..TextProps::default()
        };
        let (w, bh) = match self.field.block {
            Some(b) => {
                props.text = data.replace("\\&", "\n");
                props.wrap = true;
                props.align = b.align;
                props.line_spacing = ((h + b.spacing) / h).max(0.1);
                let w = if b.width > 0.0 {
                    b.width
                } else {
                    estimate_width(&props.text, font)
                };
                (w, b.lines * (h + b.spacing.max(0.0)))
            }
            None => {
                props.wrap = false;
                // The width is an estimate; shrinking keeps the text inside it.
                props.fit = TextFit::Shrink;
                let w = estimate_width(&data, font);
                props.text = data;
                (w, h)
            }
        };
        self.push(w, bh, rot, ElementKind::Text(props));
    }

    /// Height of the interpretation line in dots.
    fn hr_dots(&self) -> f64 {
        (self.module * 9.0).max(9.0)
    }

    fn barcode(&mut self, code: Code, data: String) {
        if data.is_empty() {
            return;
        }
        let module = self.module;
        let linear = |s: &mut Self, symbology, data: String, rot, height: f64, hr, modules: f64| {
            let text_h = s.hr_dots();
            let h = height
                + if hr == HumanReadable::None {
                    0.0
                } else {
                    text_h
                };
            let kind = ElementKind::Barcode(BarcodeProps {
                data,
                symbology,
                human_readable: hr,
                text_size_pt: s.pt(text_h),
            });
            s.push(modules * module, h, rot, kind);
        };
        match code {
            Code::Code128 {
                rot,
                height,
                hr,
                gs1,
            } => {
                let (data, symbology) = self.code128(&data, gs1);
                let modules = code128_modules(&data);
                linear(self, symbology, data, rot, height, hr, modules);
            }
            Code::Code39 { rot, height, hr } => {
                #[allow(clippy::cast_precision_loss)]
                let n = data.chars().count() as f64 + 2.0;
                let modules = n * (7.0 + 3.0 * self.ratio) - 1.0;
                linear(self, Symbology1d::Code39, data, rot, height, hr, modules);
            }
            Code::Ean {
                symbology,
                rot,
                height,
                hr,
            } => linear(self, symbology, data, rot, height, hr, 95.0),
            Code::Qr { magnification, ecc } => {
                let (ecc, data) = qr_data(&data, ecc);
                let size = qr_modules(data.len(), ecc) * magnification;
                let kind = ElementKind::Code2d(Code2dProps {
                    data,
                    symbology: Symbology2d::Qr,
                    ecc,
                });
                self.push(size, size, 0.0, kind);
            }
            Code::DataMatrix { rot, module, cols } => {
                let size = cols.unwrap_or_else(|| datamatrix_modules(&data)) * module;
                let kind = ElementKind::Code2d(Code2dProps {
                    data,
                    symbology: Symbology2d::DataMatrix,
                    ecc: EccLevel::default(),
                });
                self.push(size, size, rot, kind);
            }
        }
    }

    /// Code 128 data with its `>` invocation codes: a leading `>;`, `>:` or `>9` forces the
    /// code set; any other code is dropped with a warning.
    fn code128(&mut self, data: &str, gs1: bool) -> (String, Symbology1d) {
        if gs1 {
            return (data.to_owned(), Symbology1d::Gs1128);
        }
        let (mut symbology, rest) = match data.get(..2) {
            Some(">;") => (Symbology1d::Code128C, &data[2..]),
            Some(">:") => (Symbology1d::Code128B, &data[2..]),
            Some(">9") => (Symbology1d::Code128A, &data[2..]),
            _ => (Symbology1d::Code128, data),
        };
        let mut out = String::with_capacity(rest.len());
        let mut chars = rest.chars().peekable();
        let mut dropped = false;
        while let Some(c) = chars.next() {
            if c == '>' {
                match chars.next() {
                    Some('<') => out.push('>'),
                    Some(_) => dropped = true,
                    None => out.push('>'),
                }
            } else {
                out.push(c);
            }
        }
        if dropped {
            self.warn("Code 128 code set and function codes (`>…`) were dropped");
            symbology = Symbology1d::Code128;
        }
        (out, symbology)
    }

    fn finish(mut self, name: &str) -> Result<Import, String> {
        let (mut right, mut bottom) = (0.0_f64, 0.0_f64);
        for e in &self.elements {
            let (x1, y1) = if matches!(e.kind, ElementKind::Line(_)) {
                (
                    e.x_mm.max(e.x_mm + e.width_mm),
                    e.y_mm.max(e.y_mm + e.height_mm),
                )
            } else {
                let (s, c) = e.rotation_deg.to_radians().sin_cos();
                let hw = ((e.width_mm * c).abs() + (e.height_mm * s).abs()) / 2.0;
                let hh = ((e.width_mm * s).abs() + (e.height_mm * c).abs()) / 2.0;
                (
                    e.x_mm + e.width_mm / 2.0 + hw,
                    e.y_mm + e.height_mm / 2.0 + hh,
                )
            };
            right = right.max(x1);
            bottom = bottom.max(y1);
        }
        let width_mm = match self.width {
            Some(w) => self.mm(w),
            None => {
                self.warn("the label has no `^PW`; its width was taken from the content");
                right
            }
        };
        let height_mm = match self.length {
            Some(l) => self.mm(l),
            None => {
                self.warn("the label has no `^LL`; its length was taken from the content");
                bottom
            }
        };
        if !(width_mm > 0.0 && height_mm > 0.0) {
            return Err("the label has no size: no `^PW`/`^LL` and no fields".into());
        }
        let mut doc = Document::new(
            name,
            LabelSpec {
                width_mm,
                height_mm,
                media: self.media.unwrap_or_default(),
                ..LabelSpec::default()
            },
        );
        for (key, asset) in std::mem::take(&mut self.assets) {
            doc.assets.entry(key).or_insert(asset);
        }
        for mut e in std::mem::take(&mut self.elements) {
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
        Ok(Import {
            doc,
            warnings: self.warnings,
        })
    }
}

fn ecc(c: Option<u8>) -> Option<EccLevel> {
    Some(match c? {
        b'L' => EccLevel::L,
        b'M' => EccLevel::M,
        b'Q' => EccLevel::Q,
        b'H' => EccLevel::H,
        _ => return None,
    })
}

/// Width of a single text line in dots: the cell width for bitmap fonts, a generous average
/// advance for scalable ones.
fn estimate_width(text: &str, font: Font) -> f64 {
    let longest = text
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0)
        .max(1);
    let advance = if Font::bitmap(font.name) {
        font.width
    } else {
        0.6 * font.width
    };
    #[allow(clippy::cast_precision_loss)]
    let n = longest as f64;
    n * advance
}

/// `^FH`: `<esc>XX` hex escapes to bytes.
fn unhex(raw: &[u8], esc: u8) -> Vec<u8> {
    let hex = |c: u8| char::from(c).to_digit(16);
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let c = raw[i];
        if c == esc
            && let (Some(h), Some(l)) = (
                raw.get(i + 1).copied().and_then(hex),
                raw.get(i + 2).copied().and_then(hex),
            )
        {
            #[allow(clippy::cast_possible_truncation)]
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Modules of a Code 128 symbol: set C for an even run of digits, start, check and stop.
fn code128_modules(data: &str) -> f64 {
    let n = data.chars().count();
    let symbols = if n >= 4 && data.bytes().all(|b| b.is_ascii_digit()) {
        n.div_ceil(2)
    } else {
        n
    };
    #[allow(clippy::cast_precision_loss)]
    let m = (11 * (symbols + 2) + 13) as f64;
    m
}

/// `^BQ` field data: `<ecc><mode>,data`. Manual mode adds a character mode (`N`, `A`, `K`)
/// or `B` and a 4-digit byte count before the data.
fn qr_data(s: &str, default: EccLevel) -> (EccLevel, String) {
    let Some((head, rest)) = s.split_once(',') else {
        return (default, s.to_owned());
    };
    let head = head.as_bytes();
    let (Some(e), Some(&mode), 2) = (
        ecc(head.first().map(u8::to_ascii_uppercase)),
        head.get(1),
        head.len(),
    ) else {
        return (default, s.to_owned());
    };
    let data = match mode.to_ascii_uppercase() {
        b'A' => rest,
        b'M' => match rest.as_bytes().first().map(u8::to_ascii_uppercase) {
            Some(b'N' | b'A' | b'K') => &rest[1..],
            Some(b'B')
                if rest.len() >= 5 && rest.as_bytes()[1..5].iter().all(u8::is_ascii_digit) =>
            {
                &rest[5..]
            }
            _ => rest,
        },
        _ => return (default, s.to_owned()),
    };
    (e, data.to_owned())
}

/// Byte-mode capacity of QR versions 1–40 at L, M, Q and H.
const QR_BYTES: [[u16; 4]; 40] = [
    [17, 14, 11, 7],
    [32, 26, 20, 14],
    [53, 42, 32, 24],
    [78, 62, 46, 34],
    [106, 84, 60, 44],
    [134, 106, 74, 58],
    [154, 122, 86, 64],
    [192, 152, 108, 84],
    [230, 180, 130, 98],
    [271, 213, 151, 119],
    [321, 251, 177, 137],
    [367, 287, 203, 155],
    [425, 331, 241, 177],
    [458, 362, 258, 194],
    [520, 412, 292, 220],
    [586, 450, 322, 250],
    [644, 504, 364, 280],
    [718, 560, 394, 310],
    [792, 624, 442, 338],
    [858, 666, 482, 382],
    [929, 711, 509, 403],
    [1003, 779, 565, 439],
    [1091, 857, 611, 461],
    [1171, 911, 661, 511],
    [1273, 997, 715, 535],
    [1367, 1059, 751, 593],
    [1465, 1125, 805, 625],
    [1528, 1190, 868, 658],
    [1628, 1264, 908, 698],
    [1732, 1370, 982, 742],
    [1840, 1452, 1030, 790],
    [1952, 1538, 1112, 842],
    [2068, 1628, 1168, 898],
    [2188, 1722, 1228, 958],
    [2303, 1809, 1283, 983],
    [2431, 1911, 1351, 1051],
    [2563, 1989, 1423, 1093],
    [2699, 2099, 1499, 1139],
    [2809, 2213, 1579, 1219],
    [2953, 2331, 1663, 1273],
];

/// Modules across the smallest QR symbol that holds `bytes` in byte mode.
fn qr_modules(bytes: usize, ecc: EccLevel) -> f64 {
    let col = match ecc {
        EccLevel::L => 0,
        EccLevel::M => 1,
        EccLevel::Q => 2,
        EccLevel::H => 3,
    };
    let version = QR_BYTES
        .iter()
        .position(|caps| usize::from(caps[col]) >= bytes)
        .map_or(40, |i| i + 1);
    #[allow(clippy::cast_precision_loss)]
    let m = (17 + 4 * version) as f64;
    m
}

/// Square ECC 200 sizes and their data codewords.
const DATAMATRIX: [(u16, u16); 24] = [
    (10, 3),
    (12, 5),
    (14, 8),
    (16, 12),
    (18, 18),
    (20, 22),
    (22, 30),
    (24, 36),
    (26, 44),
    (32, 62),
    (36, 86),
    (40, 114),
    (44, 144),
    (48, 174),
    (52, 204),
    (64, 280),
    (72, 368),
    (80, 456),
    (88, 576),
    (96, 696),
    (104, 816),
    (120, 1050),
    (132, 1304),
    (144, 1558),
];

/// Modules across the smallest square Data Matrix for `data` in ASCII encodation.
fn datamatrix_modules(data: &str) -> f64 {
    let b = data.as_bytes();
    let (mut words, mut i) = (0usize, 0usize);
    while i < b.len() {
        if b[i].is_ascii_digit() && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            i += 2;
            words += 1;
        } else {
            words += if b[i] < 128 { 1 } else { 2 };
            i += 1;
        }
    }
    let size = DATAMATRIX
        .iter()
        .find(|(_, cap)| usize::from(*cap) >= words)
        .map_or(144, |(s, _)| *s);
    f64::from(size)
}

/// ASCII `^GF` data to `total` bytes of bitmap: plain or run-length compressed hex, or a
/// `:B64:` or `:Z64:` payload.
fn decode_ascii(data: &[u8], row: usize, total: usize) -> Result<Vec<u8>, String> {
    let data: Vec<u8> = data
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let mut bytes = if let Some(rest) = data
        .strip_prefix(b":Z64:")
        .map(|r| (r, true))
        .or_else(|| data.strip_prefix(b":B64:").map(|r| (r, false)))
    {
        let (payload, zlib) = rest;
        let b64 = payload.split(|&c| c == b':').next().unwrap_or_default();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| format!("invalid base64: {e}"))?;
        if zlib {
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&raw, total)
                .map_err(|e| format!("invalid Z64 data: {e}"))?
        } else {
            raw
        }
    } else {
        hex_rle(&data, row, total)?
    };
    bytes.truncate(total);
    Ok(bytes)
}

/// ZPL compressed hex: `G`–`Y` repeat the next digit 1–19 times, `g`–`z` 20–400 times, `,`
/// fills the row with 0, `!` with 1 and `:` repeats the previous row.
fn hex_rle(data: &[u8], row: usize, total: usize) -> Result<Vec<u8>, String> {
    let row_n = row * 2;
    let mut nibbles: Vec<u8> = Vec::with_capacity(total * 2);
    let mut prev = vec![0u8; row_n];
    let mut cur: Vec<u8> = Vec::with_capacity(row_n);
    let mut count = 0usize;
    let flush = |cur: &mut Vec<u8>, prev: &mut Vec<u8>, nibbles: &mut Vec<u8>| {
        cur.resize(row_n, 0);
        nibbles.extend_from_slice(cur);
        std::mem::swap(prev, cur);
        cur.clear();
    };
    for &c in data {
        if nibbles.len() >= total * 2 {
            break;
        }
        match c {
            b'G'..=b'Y' => count += usize::from(c - b'G' + 1),
            b'g'..=b'z' => count += usize::from(c - b'g' + 1) * 20,
            b',' | b'!' => {
                cur.resize(row_n, if c == b'!' { 0xF } else { 0 });
                flush(&mut cur, &mut prev, &mut nibbles);
                count = 0;
            }
            b':' => {
                let k = cur.len();
                cur.extend_from_slice(prev.get(k..).unwrap_or_default());
                flush(&mut cur, &mut prev, &mut nibbles);
                count = 0;
            }
            _ => {
                let Some(v) = char::from(c).to_digit(16) else {
                    return Err(format!("unexpected character `{}`", char::from(c)));
                };
                #[allow(clippy::cast_possible_truncation)]
                let v = v as u8;
                for _ in 0..count.max(1) {
                    cur.push(v);
                    if cur.len() == row_n {
                        flush(&mut cur, &mut prev, &mut nibbles);
                    }
                }
                count = 0;
            }
        }
    }
    if !cur.is_empty() {
        flush(&mut cur, &mut prev, &mut nibbles);
    }
    Ok(nibbles
        .chunks(2)
        .map(|p| (p[0] << 4) | p.get(1).copied().unwrap_or(0))
        .collect())
}

/// A 1-bit BMP of `rows` rows of `row` bytes; a set bit is black, as in ZPL.
fn bmp_1bit(bits: &[u8], row: usize, rows: usize) -> Vec<u8> {
    let stride = row.div_ceil(4) * 4;
    let offset = 14 + 40 + 8;
    let size = offset + stride * rows;
    let u32le = |v: usize| u32::try_from(v).unwrap_or(u32::MAX).to_le_bytes();
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&u32le(size));
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&u32le(offset));
    out.extend_from_slice(&u32le(40));
    out.extend_from_slice(&u32le(row * 8));
    // A positive height stores the rows bottom-up.
    out.extend_from_slice(&u32le(rows));
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&u32le(stride * rows));
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&u32le(2));
    out.extend_from_slice(&[0; 4]);
    // Palette: 0 white, 1 black.
    out.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0]);
    for r in (0..rows).rev() {
        let line = bits.get(r * row..(r + 1) * row).unwrap_or_default();
        out.extend_from_slice(line);
        out.resize(out.len() + stride - line.len(), 0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection() {
        assert!(detect(b"\xEF\xBB\xBF  ^XA^FO10,10^FDHi^FS^XZ"));
        assert!(detect(b"~SD15\n^xa^xz"));
        assert!(!detect(b"{\"format_version\": 2, \"x\": \"^XA\"}"));
        assert!(!detect(b"plain text"));
    }

    #[test]
    fn tokens_split_on_prefixes() {
        let t = tokenize(b"^XA^A0N,30,30^FDa~b^FS~SD15^XZ");
        let names: Vec<&str> = t.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["XA", "A", "FD", "FS", "SD", "XZ"]);
        assert_eq!(t[1].params, b"0N,30,30");
        assert_eq!(t[2].params, b"a~b");
    }

    #[test]
    fn binary_graphic_field_may_hold_carets() {
        let t = tokenize(b"^GFB,2,2,1,^~^FS");
        assert_eq!(t[0].params, b"B,2,2,1,^~");
        assert_eq!(t[1].name, "FS");
    }

    #[test]
    fn hex_escapes() {
        assert_eq!(unhex(b"A_5FB_C3_A9", b'_'), b"A_B\xC3\xA9");
        assert_eq!(unhex(b"x_zz", b'_'), b"x_zz");
    }

    #[test]
    fn compressed_hex() {
        // 2 bytes per row: "FFFF", then a row filled by `,`, then `:` repeats it, then
        // `!` fills a row with ones after "0".
        let b = hex_rle(b"JF,:0!", 2, 8).unwrap_or_default();
        assert_eq!(b, [0xFF, 0xFF, 0, 0, 0, 0, 0x0F, 0xFF]);
        // `gG` = 21 repeats spans rows.
        let b = hex_rle(b"gGF", 3, 12).unwrap_or_default();
        let mut want = vec![0xFF; 10];
        want.extend([0xF0, 0]);
        assert_eq!(b, want);
    }

    #[test]
    fn base64_payloads() {
        let raw = [0xAAu8, 0x55];
        let b64 = base64::engine::general_purpose::STANDARD.encode(raw);
        let got = decode_ascii(format!(":B64:{b64}:1234").as_bytes(), 1, 2);
        assert_eq!(got, Ok(raw.to_vec()));
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let z64 = base64::engine::general_purpose::STANDARD.encode(z);
        let got = decode_ascii(format!(":Z64:{z64}:abcd").as_bytes(), 1, 2);
        assert_eq!(got, Ok(raw.to_vec()));
    }

    #[test]
    fn qr_prefixes() {
        assert_eq!(
            qr_data("QA,hello", EccLevel::M),
            (EccLevel::Q, "hello".into())
        );
        assert_eq!(qr_data("HM,N123", EccLevel::M), (EccLevel::H, "123".into()));
        assert_eq!(
            qr_data("LM,B0003abc", EccLevel::M),
            (EccLevel::L, "abc".into())
        );
        assert_eq!(
            qr_data("no prefix", EccLevel::M),
            (EccLevel::M, "no prefix".into())
        );
        assert_eq!(qr_modules(14, EccLevel::M), 21.0);
        assert_eq!(qr_modules(15, EccLevel::M), 25.0);
    }

    #[test]
    fn code_sizes() {
        // 12 digits in set C: 6 symbols + start + check, 11 modules each, + 13 stop.
        assert_eq!(code128_modules("123456789012"), 101.0);
        assert_eq!(code128_modules("AB"), 57.0);
        assert_eq!(datamatrix_modules("123456"), 10.0);
        assert_eq!(datamatrix_modules("ABCD"), 12.0);
    }

    #[test]
    fn bmp_layout() {
        let bmp = bmp_1bit(&[0x80, 0x01], 1, 2);
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(bmp.len(), 62 + 8);
        // Bottom-up: the second row comes first.
        assert_eq!(&bmp[62..], &[0x01, 0, 0, 0, 0x80, 0, 0, 0]);
    }
}
