//! The `.labely` document model (ADR-0008). Units are millimetres; ids are stable.
//!
//! JSON shape is part of the public format: every change needs a `FORMAT_VERSION` bump
//! and a migration.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Current `.labely` format version.
pub const FORMAT_VERSION: u32 = 2;

/// Stable element id, unique within a document.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ElementId(pub u64);

/// A label document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub format_version: u32,
    pub name: String,
    pub label: LabelSpec,
    /// Pages; Phase 1 documents have exactly one.
    pub pages: Vec<Page>,
    /// Embedded assets keyed by lowercase hex sha256 of the bytes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<String, Asset>,
    /// Free variables usable as `{{name}}` in string properties.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
    /// Attached data source; each row is one record, printed as one label (v2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<DataSource>,
    /// Serial counters usable as `{{name}}`; they advance by one step per label (v2).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub serials: BTreeMap<String, SerialSpec>,
    /// Next id to hand out.
    pub next_id: u64,
}

impl Document {
    /// New empty document with one page.
    pub fn new(name: impl Into<String>, label: LabelSpec) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: name.into(),
            label,
            pages: vec![Page::default()],
            assets: BTreeMap::new(),
            variables: BTreeMap::new(),
            data: None,
            serials: BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Allocate a fresh element id.
    pub fn alloc_id(&mut self) -> ElementId {
        let id = ElementId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Iterate all elements on all pages.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.pages.iter().flat_map(|p| p.elements.iter())
    }

    pub fn element(&self, id: ElementId) -> Option<&Element> {
        self.elements().find(|e| e.id == id)
    }

    /// Drop assets no image element and no data cell refers to; returns how many went.
    /// Data cells hold asset keys for images bound with `{{column}}` (ADR-0018).
    pub fn prune_assets(&mut self) -> usize {
        let used: std::collections::BTreeSet<&str> = self
            .elements()
            .filter_map(|e| match &e.kind {
                ElementKind::Image(i) => Some(i.asset.as_str()),
                _ => None,
            })
            .chain(
                self.data
                    .iter()
                    .flat_map(|d| d.rows.iter().flatten().map(String::as_str)),
            )
            .collect();
        let unused: Vec<String> = self
            .assets
            .keys()
            .filter(|k| !used.contains(k.as_str()))
            .cloned()
            .collect();
        for k in &unused {
            self.assets.remove(k);
        }
        unused.len()
    }

    pub fn element_mut(&mut self, id: ElementId) -> Option<&mut Element> {
        self.pages
            .iter_mut()
            .flat_map(|p| p.elements.iter_mut())
            .find(|e| e.id == id)
    }
}

/// Tabular data embedded in the document. Cells are text; column names are the binding
/// names (`{{column}}`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DataSource {
    /// File name the data came from, for display only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub columns: Vec<String>,
    /// Rows of cells; a short row reads as empty cells.
    pub rows: Vec<Vec<String>>,
}

impl DataSource {
    /// Cell value of `row` (0-based) in `column`, empty when the row is short.
    pub fn cell(&self, row: usize, column: &str) -> Option<&str> {
        let c = self.columns.iter().position(|n| n == column)?;
        let r = self.rows.get(row)?;
        Some(r.get(c).map_or("", String::as_str))
    }

    /// `column -> value` for one row (0-based).
    pub fn record(&self, row: usize) -> BTreeMap<String, String> {
        let cells = self.rows.get(row).map_or(&[][..], Vec::as_slice);
        self.columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.clone(), cells.get(i).cloned().unwrap_or_default()))
            .collect()
    }
}

/// A serial number: `prefix + pad(start + step × n, digits) + suffix`, where `n` is the
/// 0-based position of the label in the print job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SerialSpec {
    #[serde(default = "one_i64")]
    pub start: i64,
    #[serde(default = "one_i64")]
    pub step: i64,
    /// Minimum digits; shorter numbers are zero-padded. 0 means no padding.
    #[serde(default)]
    pub digits: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub suffix: String,
}

fn one_i64() -> i64 {
    1
}

impl Default for SerialSpec {
    fn default() -> Self {
        Self {
            start: 1,
            step: 1,
            digits: 0,
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}

impl SerialSpec {
    /// Value for the label at position `n` (0-based).
    pub fn value(&self, n: u64) -> String {
        let n = i64::try_from(n).unwrap_or(i64::MAX);
        let v = self.start.saturating_add(self.step.saturating_mul(n));
        let digits = usize::from(self.digits);
        let num = if v < 0 {
            format!("-{:0digits$}", v.unsigned_abs())
        } else {
            format!("{v:0digits$}")
        };
        format!("{}{num}{}", self.prefix, self.suffix)
    }
}

/// Physical label stock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LabelSpec {
    /// Width across the printhead, mm.
    pub width_mm: f64,
    /// Length along the feed, mm.
    pub height_mm: f64,
    #[serde(default)]
    pub corner_radius_mm: f64,
    #[serde(default)]
    pub media: MediaType,
    /// Registry id of the intended printer model, e.g. `niimbot.b21_pro`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer_model: Option<String>,
    /// Preferred density; the model default is used when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<u8>,
}

impl Default for LabelSpec {
    fn default() -> Self {
        Self {
            width_mm: 50.0,
            height_mm: 30.0,
            corner_radius_mm: 0.0,
            media: MediaType::Gap,
            printer_model: None,
            density: None,
        }
    }
}

/// Media type of the label stock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    #[default]
    Gap,
    BlackMark,
    Continuous,
    Transparent,
}

/// One printed page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Page {
    /// Z-order: later elements draw on top.
    pub elements: Vec<Element>,
}

/// Embedded binary asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub mime: String,
    /// Standard base64 of the bytes.
    pub data_base64: String,
}

/// A positioned element. The frame is the unrotated box; rotation is about its centre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Element {
    pub id: ElementId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    #[serde(default)]
    pub rotation_deg: f64,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub hidden: bool,
    /// Mirror left–right inside the frame (before rotation).
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_h: bool,
    /// Mirror top–bottom inside the frame (before rotation).
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_v: bool,
    /// Group id shared by grouped elements (`element.group`); groups are flat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    /// Linked mirror copy (`element.mirror`): content, size and position follow the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror: Option<MirrorLink>,
    #[serde(flatten)]
    pub kind: ElementKind,
}

/// Element payload, tagged by `"type"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElementKind {
    Text(TextProps),
    Barcode(BarcodeProps),
    Code2d(Code2dProps),
    Image(ImageProps),
    Rect(ShapeProps),
    Ellipse(ShapeProps),
    Line(LineProps),
    Table(TableProps),
    Icon(IconProps),
    Border(BorderProps),
}

impl Element {
    /// Axis-aligned bounds of the rotated frame: (min_x, min_y, max_x, max_y) in mm.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let (s, c) = self.rotation_deg.to_radians().sin_cos();
        let (hw, hh) = (self.width_mm / 2.0, self.height_mm / 2.0);
        let ex = (hw * c).abs() + (hh * s).abs();
        let ey = (hw * s).abs() + (hh * c).abs();
        let (cx, cy) = (self.x_mm + hw, self.y_mm + hh);
        (cx - ex, cy - ey, cx + ex, cy + ey)
    }
}

/// A mirror copy's link to its source element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MirrorLink {
    /// Source element on the same page.
    pub of: ElementId,
    pub axis: MirrorAxis,
}

/// Line a mirror copy is reflected across.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MirrorAxis {
    /// Left–right, across the label's vertical centre line.
    #[default]
    Horizontal,
    /// Top–bottom, across the label's horizontal centre line.
    Vertical,
}

impl Document {
    /// Re-derive every linked mirror copy from its source. A link whose source is gone, is on
    /// another page or is itself a mirror is dropped and the copy stays as it is.
    pub fn sync_mirrors(&mut self) {
        let (lw, lh) = (self.label.width_mm, self.label.height_mm);
        for page in &mut self.pages {
            if !page.elements.iter().any(|e| e.mirror.is_some()) {
                continue;
            }
            for i in 0..page.elements.len() {
                let Some(link) = page.elements[i].mirror else {
                    continue;
                };
                let src = page
                    .elements
                    .iter()
                    .find(|e| e.id == link.of && e.mirror.is_none() && e.id != page.elements[i].id)
                    .cloned();
                let m = &mut page.elements[i];
                let Some(src) = src else {
                    m.mirror = None;
                    continue;
                };
                m.kind = src.kind;
                m.width_mm = src.width_mm;
                m.height_mm = src.height_mm;
                m.rotation_deg = (-src.rotation_deg).rem_euclid(360.0);
                m.hidden = src.hidden;
                match link.axis {
                    MirrorAxis::Horizontal => {
                        m.x_mm = lw - src.x_mm - src.width_mm;
                        m.y_mm = src.y_mm;
                        m.flip_h = !src.flip_h;
                        m.flip_v = src.flip_v;
                    }
                    MirrorAxis::Vertical => {
                        m.x_mm = src.x_mm;
                        m.y_mm = lh - src.y_mm - src.height_mm;
                        m.flip_h = src.flip_h;
                        m.flip_v = !src.flip_v;
                    }
                }
            }
        }
    }
}

impl Document {
    /// Turn the label a quarter turn: width and height swap. With `rotate_content` every
    /// element turns with it about the label, so the design keeps its look.
    pub fn rotate_label(&mut self, cw: bool, rotate_content: bool) {
        let (w, h) = (self.label.width_mm, self.label.height_mm);
        if rotate_content {
            for page in &mut self.pages {
                for e in &mut page.elements {
                    let (cx, cy) = (e.x_mm + e.width_mm / 2.0, e.y_mm + e.height_mm / 2.0);
                    // y points down, so a clockwise turn maps (x, y) to (h − y, x).
                    let (nx, ny) = if cw { (h - cy, cx) } else { (cy, w - cx) };
                    e.x_mm = nx - e.width_mm / 2.0;
                    e.y_mm = ny - e.height_mm / 2.0;
                    let turn = if cw { 90.0 } else { 270.0 };
                    e.rotation_deg = (e.rotation_deg + turn).rem_euclid(360.0);
                }
            }
        }
        self.label.width_mm = h;
        self.label.height_mm = w;
    }
}

impl ElementKind {
    /// Stable type name as used in JSON.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Barcode(_) => "barcode",
            Self::Code2d(_) => "code2d",
            Self::Image(_) => "image",
            Self::Rect(_) => "rect",
            Self::Ellipse(_) => "ellipse",
            Self::Line(_) => "line",
            Self::Table(_) => "table",
            Self::Icon(_) => "icon",
            Self::Border(_) => "border",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
    /// Stretch the spaces so wrapped lines fill the box; the last line stays left.
    Justify,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// How text reacts when it does not fit its box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextFit {
    /// Keep the size; overflow is reported by `doc.check`.
    #[default]
    None,
    /// Shrink the font until the text fits.
    Shrink,
    /// Grow or shrink the font to the largest size that fits the box.
    Fill,
}

/// Style for a range of a text element's `text`. Unset fields keep the element's style;
/// later spans win where they overlap.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextSpan {
    /// First character (Unicode scalar index into `text`).
    pub start: usize,
    /// One past the last character.
    pub end: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<f64>,
}

/// Most spans on one text element.
pub const MAX_SPANS: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextProps {
    /// Text; may contain `{{variable}}` bindings and newlines.
    pub text: String,
    #[serde(default = "default_font")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size_pt: f64,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub align: HAlign,
    #[serde(default)]
    pub valign: VAlign,
    /// Line height multiplier.
    #[serde(default = "one")]
    pub line_spacing: f64,
    #[serde(default)]
    pub letter_spacing_mm: f64,
    #[serde(default = "yes")]
    pub wrap: bool,
    #[serde(default)]
    pub fit: TextFit,
    /// White text on black.
    #[serde(default)]
    pub invert: bool,
    /// Horizontal, or vertical: one character per row, top to bottom.
    #[serde(default, skip_serializing_if = "is_default")]
    pub direction: TextDirection,
    /// Warn in `doc.check` when the longest printed line is wider than this, mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length_mm: Option<f64>,
    /// Rich-text style ranges over `text` (horizontal text only). A `{{binding}}` that
    /// crosses a span edge is not substituted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<TextSpan>,
}

/// Text flow. Text turned 90° is `rotation_deg: 90`; curved text is planned for Phase 6.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextDirection {
    #[default]
    Horizontal,
    /// Characters stacked top to bottom; each source line is separated by a blank row.
    Vertical,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// Default bundled font family.
pub const DEFAULT_FONT: &str = "Noto Sans";

fn default_font() -> String {
    DEFAULT_FONT.to_owned()
}
fn default_font_size() -> f64 {
    12.0
}
fn one() -> f64 {
    1.0
}
fn is_false(b: &bool) -> bool {
    !*b
}
fn yes() -> bool {
    true
}

impl Default for TextProps {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_family: default_font(),
            font_size_pt: default_font_size(),
            bold: false,
            italic: false,
            underline: false,
            align: HAlign::Left,
            valign: VAlign::Top,
            line_spacing: 1.0,
            letter_spacing_mm: 0.0,
            wrap: true,
            fit: TextFit::None,
            invert: false,
            direction: TextDirection::Horizontal,
            max_length_mm: None,
            spans: Vec::new(),
        }
    }
}

/// 1D symbologies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Symbology1d {
    /// Code 128, code set chosen automatically.
    #[default]
    Code128,
    /// Code 128 forced to code set A (upper case, digits, control characters).
    #[serde(rename = "code128_a")]
    Code128A,
    /// Code 128 forced to code set B (printable ASCII).
    #[serde(rename = "code128_b")]
    Code128B,
    /// Code 128 forced to code set C (an even number of digits).
    #[serde(rename = "code128_c")]
    Code128C,
    /// GS1-128: application identifiers in brackets, e.g. `(01)09501101530003(17)250101`.
    #[serde(rename = "gs1_128")]
    Gs1128,
    Code39,
    Code93,
    Ean13,
    Ean8,
    UpcA,
    UpcE,
    Itf,
    Codabar,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HumanReadable {
    None,
    #[default]
    Below,
    Above,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BarcodeProps {
    pub data: String,
    #[serde(default)]
    pub symbology: Symbology1d,
    #[serde(default)]
    pub human_readable: HumanReadable,
    #[serde(default = "default_hr_size")]
    pub text_size_pt: f64,
}

fn default_hr_size() -> f64 {
    8.0
}

/// 2D symbologies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Symbology2d {
    #[default]
    Qr,
    DataMatrix,
    Pdf417,
    Aztec,
}

/// QR error-correction level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EccLevel {
    L,
    #[default]
    M,
    Q,
    H,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Code2dProps {
    pub data: String,
    #[serde(default)]
    pub symbology: Symbology2d,
    #[serde(default)]
    pub ecc: EccLevel,
}

/// 1-bit conversion for images.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dither {
    Threshold,
    #[default]
    FloydSteinberg,
    Atkinson,
    Bayer,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    #[default]
    Contain,
    Cover,
    Stretch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageProps {
    /// Key into [`Document::assets`].
    pub asset: String,
    #[serde(default)]
    pub fit: ImageFit,
    #[serde(default)]
    pub dither: Dither,
    /// 0–255 cut-off for `threshold` and the gray point for dithering.
    #[serde(default = "default_threshold")]
    pub threshold: u8,
    /// -1.0 … 1.0
    #[serde(default)]
    pub brightness: f64,
    /// -1.0 … 1.0
    #[serde(default)]
    pub contrast: f64,
    #[serde(default)]
    pub invert: bool,
    /// Part of the source image to use, as fractions of its size; the whole image if absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
}

/// A crop rectangle in fractions (0–1) of the image's width and height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Crop {
    /// `Err` names the problem when the edges are not 0 ≤ left < right ≤ 1 (and the same
    /// for top and bottom).
    pub fn validate(&self) -> Result<(), String> {
        let ok = |a: f64, b: f64| a.is_finite() && b.is_finite() && 0.0 <= a && a < b && b <= 1.0;
        if ok(self.left, self.right) && ok(self.top, self.bottom) {
            Ok(())
        } else {
            Err("crop needs 0 ≤ left < right ≤ 1 and 0 ≤ top < bottom ≤ 1".to_owned())
        }
    }
}

fn default_threshold() -> u8 {
    128
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShapeProps {
    #[serde(default = "default_stroke")]
    pub stroke_mm: f64,
    #[serde(default)]
    pub fill: bool,
    /// Rect only.
    #[serde(default)]
    pub corner_radius_mm: f64,
}

impl Default for ShapeProps {
    fn default() -> Self {
        Self {
            stroke_mm: default_stroke(),
            fill: false,
            corner_radius_mm: 0.0,
        }
    }
}

/// A line from the frame's top-left `(x, y)` to `(x + width, y + height)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LineProps {
    #[serde(default = "default_stroke")]
    pub stroke_mm: f64,
    /// Dash and gap length in mm; solid when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash_mm: Option<[f64; 2]>,
}

fn default_stroke() -> f64 {
    0.3
}

/// Most rows or columns in a table.
pub const MAX_TABLE_DIM: usize = 64;

/// A grid that fills its frame. Column widths and row heights are relative weights, so resizing
/// the element scales the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TableProps {
    /// Relative column widths (weights > 0).
    pub column_widths: Vec<f64>,
    /// Relative row heights (weights > 0).
    pub row_heights: Vec<f64>,
    /// Row-major cells: `row_heights.len()` rows of `column_widths.len()` cells.
    pub cells: Vec<Vec<TableCell>>,
    /// Merged cell ranges; the top-left cell's content fills the range.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merges: Vec<CellRange>,
    /// Grid line width in mm; 0 hides the lines.
    #[serde(default = "default_stroke")]
    pub stroke_mm: f64,
    #[serde(default = "default_font")]
    pub font_family: String,
    /// Default cell font size; text shrinks to fit its cell.
    #[serde(default = "default_table_font")]
    pub font_size_pt: f64,
    /// Default cell alignment.
    #[serde(default = "center")]
    pub align: HAlign,
}

fn default_table_font() -> f64 {
    8.0
}
fn center() -> HAlign {
    HAlign::Center
}

impl TableProps {
    /// `rows × cols` empty cells with equal sizes.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            column_widths: vec![1.0; cols],
            row_heights: vec![1.0; rows],
            cells: vec![vec![TableCell::default(); cols]; rows],
            merges: Vec::new(),
            stroke_mm: default_stroke(),
            font_family: default_font(),
            font_size_pt: default_table_font(),
            align: HAlign::Center,
        }
    }

    /// The merge covering `(row, col)`, if any.
    pub fn merge_at(&self, row: usize, col: usize) -> Option<&CellRange> {
        self.merges.iter().find(|m| m.contains(row, col))
    }

    /// Check sizes, weights and merges; `Err` names the problem.
    pub fn validate(&self) -> Result<(), String> {
        let (rows, cols) = (self.row_heights.len(), self.column_widths.len());
        if rows == 0 || cols == 0 || rows > MAX_TABLE_DIM || cols > MAX_TABLE_DIM {
            return Err(format!(
                "a table needs 1–{MAX_TABLE_DIM} rows and columns, got {rows}×{cols}"
            ));
        }
        if let Some(w) = self
            .column_widths
            .iter()
            .chain(&self.row_heights)
            .find(|w| !(w.is_finite() && **w > 0.0))
        {
            return Err(format!(
                "row heights and column widths must be > 0, got {w}"
            ));
        }
        if self.cells.len() != rows || self.cells.iter().any(|r| r.len() != cols) {
            return Err(format!("`cells` must be {rows} rows of {cols} cells"));
        }
        for (i, m) in self.merges.iter().enumerate() {
            if m.rows == 0 || m.cols == 0 || m.rows * m.cols < 2 {
                return Err(format!("merge {i} must span at least two cells"));
            }
            if m.row + m.rows > rows || m.col + m.cols > cols {
                return Err(format!("merge {i} extends outside the {rows}×{cols} table"));
            }
            if self.merges[..i].iter().any(|o| o.overlaps(m)) {
                return Err(format!("merge {i} overlaps another merge"));
            }
        }
        Ok(())
    }
}

/// One table cell. Style fields override the table defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TableCell {
    /// Cell text; may contain `{{name}}` bindings and newlines.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<HAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<f64>,
    /// White text on a black cell.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
}

/// A rectangular block of cells, 0-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CellRange {
    pub row: usize,
    pub col: usize,
    pub rows: usize,
    pub cols: usize,
}

impl CellRange {
    pub fn contains(&self, row: usize, col: usize) -> bool {
        (self.row..self.row + self.rows).contains(&row)
            && (self.col..self.col + self.cols).contains(&col)
    }

    pub fn overlaps(&self, o: &CellRange) -> bool {
        self.row < o.row + o.rows
            && o.row < self.row + self.rows
            && self.col < o.col + o.cols
            && o.col < self.col + self.cols
    }
}

/// A built-in icon from the bundled set (`labely-render` lists the names).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IconProps {
    /// Icon name, e.g. `fragile`, `keep_dry`, `this_way_up`, `recycle`.
    pub icon: String,
    /// Keep the icon's aspect ratio inside the frame (default true).
    #[serde(default = "yes")]
    pub keep_aspect: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
}

/// Decorative frame styles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    #[default]
    Single,
    Double,
    /// A thick outer line and a thin inner line.
    ThickThin,
    Dashed,
    Dotted,
    /// Corner brackets only.
    Corners,
    /// Concave (ticket-style) corners.
    Ticket,
    /// Scalloped edge.
    Scallop,
}

/// A frame drawn just inside the element's box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BorderProps {
    #[serde(default)]
    pub style: BorderStyle,
    /// Line width in mm.
    #[serde(default = "default_border_stroke")]
    pub stroke_mm: f64,
    /// Corner radius in mm (single, double, thick_thin, dashed, dotted).
    #[serde(default)]
    pub corner_radius_mm: f64,
}

fn default_border_stroke() -> f64 {
    0.5
}

impl Default for BorderProps {
    fn default() -> Self {
        Self {
            style: BorderStyle::Single,
            stroke_mm: default_border_stroke(),
            corner_radius_mm: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_json_is_flat_and_tagged() {
        let mut d = Document::new("t", LabelSpec::default());
        let id = d.alloc_id();
        d.pages[0].elements.push(Element {
            id,
            name: None,
            x_mm: 1.0,
            y_mm: 2.0,
            width_mm: 10.0,
            height_mm: 5.0,
            rotation_deg: 0.0,
            locked: false,
            hidden: false,
            flip_h: false,
            flip_v: false,
            group: None,
            mirror: None,
            kind: ElementKind::Text(TextProps {
                text: "Hi".into(),
                ..Default::default()
            }),
        });
        let v = serde_json::to_value(&d).unwrap();
        let e = &v["pages"][0]["elements"][0];
        assert_eq!(e["type"], "text");
        assert_eq!(e["text"], "Hi");
        let back: Document = serde_json::from_value(v).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn serial_values() {
        let s = SerialSpec {
            start: 7,
            step: 3,
            digits: 4,
            prefix: "A-".into(),
            suffix: "/x".into(),
        };
        assert_eq!(s.value(0), "A-0007/x");
        assert_eq!(s.value(2), "A-0013/x");
        let neg = SerialSpec {
            start: 0,
            step: -1,
            digits: 2,
            ..Default::default()
        };
        assert_eq!(neg.value(3), "-03");
        assert_eq!(SerialSpec::default().value(9), "10");
    }

    #[test]
    fn data_records() {
        let d = DataSource {
            origin: None,
            columns: vec!["a".into(), "b".into()],
            rows: vec![vec!["1".into()], vec!["2".into(), "x".into()]],
        };
        assert_eq!(d.cell(0, "b"), Some(""));
        assert_eq!(d.cell(1, "b"), Some("x"));
        assert_eq!(d.cell(5, "a"), None);
        assert_eq!(d.record(1)["a"], "2");
    }
}
