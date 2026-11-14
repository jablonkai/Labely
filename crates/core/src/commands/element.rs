//! `element.*` commands.

use base64::Engine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::check_mm;
use crate::command::{Command, CommandError, CommandMeta, Ctx, element, element_mut};
use crate::document::*;
use crate::registry::serde_error;
use crate::schema::property_names;

/// Frame fields every element has (settable through `element.set_props`).
pub const FRAME_KEYS: &[&str] = &[
    "name",
    "x_mm",
    "y_mm",
    "width_mm",
    "height_mm",
    "rotation_deg",
    "locked",
    "hidden",
];

/// Largest embeddable image, bytes.
pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;

macro_rules! add_params {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Deserialize, JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            /// Document id; defaults to the active document.
            #[serde(default)]
            pub doc_id: Option<String>,
            /// Left edge of the unrotated frame, mm from the label's left edge.
            pub x_mm: f64,
            /// Top edge of the unrotated frame, mm from the label's top edge.
            pub y_mm: f64,
            /// Frame width, mm.
            pub width_mm: f64,
            /// Frame height, mm.
            pub height_mm: f64,
            /// Clockwise rotation about the frame centre, degrees (default 0).
            #[serde(default)]
            pub rotation_deg: Option<f64>,
            /// Optional element name.
            #[serde(default)]
            pub name: Option<String>,
            $($(#[$fm])* pub $field: $ty,)*
        }

        impl $name {
            fn frame(&self) -> Frame {
                Frame {
                    x_mm: self.x_mm,
                    y_mm: self.y_mm,
                    width_mm: self.width_mm,
                    height_mm: self.height_mm,
                    rotation_deg: self.rotation_deg.unwrap_or(0.0),
                    name: self.name.clone(),
                }
            }
        }
    };
}

struct Frame {
    x_mm: f64,
    y_mm: f64,
    width_mm: f64,
    height_mm: f64,
    rotation_deg: f64,
    name: Option<String>,
}

/// Validate frame and props of an element.
pub(crate) fn validate_element(e: &Element) -> Result<(), CommandError> {
    let is_line = matches!(e.kind, ElementKind::Line(_));
    check_mm("x_mm", e.x_mm, false)?;
    check_mm("y_mm", e.y_mm, false)?;
    check_mm("width_mm", e.width_mm, !is_line)?;
    check_mm("height_mm", e.height_mm, !is_line)?;
    if !e.rotation_deg.is_finite() {
        return Err(
            CommandError::bad_params("`rotation_deg` must be finite").with_param("rotation_deg")
        );
    }
    if is_line && e.width_mm == 0.0 && e.height_mm == 0.0 {
        return Err(
            CommandError::bad_params("a line needs a non-zero width_mm or height_mm")
                .with_param("width_mm"),
        );
    }
    let positive = |name: &str, v: f64| -> Result<(), CommandError> {
        if v.is_finite() && v > 0.0 {
            Ok(())
        } else {
            Err(CommandError::bad_params(format!("`{name}` must be > 0, got {v}")).with_param(name))
        }
    };
    let non_negative = |name: &str, v: f64| -> Result<(), CommandError> {
        if v.is_finite() && v >= 0.0 {
            Ok(())
        } else {
            Err(
                CommandError::bad_params(format!("`{name}` must be >= 0, got {v}"))
                    .with_param(name),
            )
        }
    };
    match &e.kind {
        ElementKind::Text(t) => {
            positive("font_size_pt", t.font_size_pt)?;
            positive("line_spacing", t.line_spacing)?;
        }
        ElementKind::Barcode(b) => positive("text_size_pt", b.text_size_pt)?,
        ElementKind::Code2d(_) => {}
        ElementKind::Image(i) => {
            for (n, v) in [("brightness", i.brightness), ("contrast", i.contrast)] {
                if !(-1.0..=1.0).contains(&v) {
                    return Err(
                        CommandError::bad_params(format!("`{n}` must be in -1.0…1.0"))
                            .with_param(n),
                    );
                }
            }
        }
        ElementKind::Rect(s) | ElementKind::Ellipse(s) => {
            non_negative("stroke_mm", s.stroke_mm)?;
            non_negative("corner_radius_mm", s.corner_radius_mm)?;
        }
        ElementKind::Line(l) => {
            positive("stroke_mm", l.stroke_mm)?;
            if let Some([d, g]) = l.dash_mm {
                positive("dash_mm", d)?;
                positive("dash_mm", g)?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddOutput {
    /// Id of the new element.
    pub id: ElementId,
}

fn add(
    ctx: &mut Ctx<'_>,
    doc_id: Option<&str>,
    frame: Frame,
    kind: ElementKind,
) -> Result<AddOutput, CommandError> {
    let doc = ctx.doc_mut(doc_id)?;
    let id = doc.alloc_id();
    let e = Element {
        id,
        name: frame.name,
        x_mm: frame.x_mm,
        y_mm: frame.y_mm,
        width_mm: frame.width_mm,
        height_mm: frame.height_mm,
        rotation_deg: frame.rotation_deg.rem_euclid(360.0),
        locked: false,
        hidden: false,
        kind,
    };
    validate_element(&e)?;
    if doc.pages.is_empty() {
        doc.pages.push(Page::default());
    }
    doc.pages[0].elements.push(e);
    Ok(AddOutput { id })
}

macro_rules! set_opt {
    ($target:expr, $p:ident: $($f:ident),*) => {
        $(if let Some(v) = $p.$f { $target.$f = v; })*
    };
}

add_params!(TextParams {
    /// Text; may contain `{{variable}}` bindings and newlines.
    text: String,
    /// Font family (default `Noto Sans`).
    #[serde(default)]
    font_family: Option<String>,
    /// Font size in points (default 12).
    #[serde(default)]
    font_size_pt: Option<f64>,
    #[serde(default)]
    bold: Option<bool>,
    #[serde(default)]
    italic: Option<bool>,
    #[serde(default)]
    underline: Option<bool>,
    #[serde(default)]
    align: Option<HAlign>,
    #[serde(default)]
    valign: Option<VAlign>,
    /// Line height multiplier (default 1.0).
    #[serde(default)]
    line_spacing: Option<f64>,
    #[serde(default)]
    letter_spacing_mm: Option<f64>,
    /// Wrap at the frame width (default true).
    #[serde(default)]
    wrap: Option<bool>,
    /// `none` (default; overflow reported by doc.check) or `shrink`.
    #[serde(default)]
    fit: Option<TextFit>,
    /// White text on black.
    #[serde(default)]
    invert: Option<bool>,
});

/// `element.add_text`
#[derive(Debug)]
pub struct AddText;

impl Command for AddText {
    const ID: &'static str = "element.add_text";
    type Params = TextParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Add text",
            "Add a text box. Frame in mm (x_mm, y_mm = top-left; width_mm, height_mm), \
             rotation_deg clockwise; font_size_pt in points. `{{name}}` binds a variable. \
             Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: TextParams) -> Result<AddOutput, CommandError> {
        let frame = p.frame();
        let mut t = TextProps {
            text: p.text,
            ..Default::default()
        };
        set_opt!(t, p: font_family, font_size_pt, bold, italic, underline, align, valign,
            line_spacing, letter_spacing_mm, wrap, fit, invert);
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Text(t))
    }
}

add_params!(BarcodeParams {
    /// Data to encode; may contain `{{variable}}` bindings.
    data: String,
    /// 1D symbology (default `code128`).
    #[serde(default)]
    symbology: Option<Symbology1d>,
    /// Human-readable text position (default `below`).
    #[serde(default)]
    human_readable: Option<HumanReadable>,
    /// Human-readable text size in points (default 8).
    #[serde(default)]
    text_size_pt: Option<f64>,
});

/// `element.add_barcode`
#[derive(Debug)]
pub struct AddBarcode;

impl Command for AddBarcode {
    const ID: &'static str = "element.add_barcode";
    type Params = BarcodeParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Add barcode",
            "Add a 1D barcode. Frame in mm; text_size_pt in points. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: BarcodeParams) -> Result<AddOutput, CommandError> {
        let frame = p.frame();
        let b = BarcodeProps {
            data: p.data,
            symbology: p.symbology.unwrap_or_default(),
            human_readable: p.human_readable.unwrap_or_default(),
            text_size_pt: p.text_size_pt.unwrap_or(8.0),
        };
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Barcode(b))
    }
}

add_params!(Code2dParams {
    /// Data to encode; may contain `{{variable}}` bindings.
    data: String,
    /// 2D symbology (default `qr`).
    #[serde(default)]
    symbology: Option<Symbology2d>,
    /// QR error correction L, M (default), Q or H.
    #[serde(default)]
    ecc: Option<EccLevel>,
});

/// `element.add_code2d`
#[derive(Debug)]
pub struct AddCode2d;

impl Command for AddCode2d {
    const ID: &'static str = "element.add_code2d";
    type Params = Code2dParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Add 2D code",
            "Add a QR / DataMatrix / PDF417 / Aztec code. Frame in mm. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: Code2dParams) -> Result<AddOutput, CommandError> {
        let frame = p.frame();
        let c = Code2dProps {
            data: p.data,
            symbology: p.symbology.unwrap_or_default(),
            ecc: p.ecc.unwrap_or_default(),
        };
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Code2d(c))
    }
}

add_params!(ImageParams {
    /// Image file to embed (PNG, JPEG, BMP, WebP). Exactly one of `path` / `data_base64`.
    #[serde(default)]
    path: Option<String>,
    /// Standard base64 of the image bytes.
    #[serde(default)]
    data_base64: Option<String>,
    /// MIME type; sniffed from the bytes when absent.
    #[serde(default)]
    mime: Option<String>,
    /// `contain` (default), `cover` or `stretch`.
    #[serde(default)]
    fit: Option<ImageFit>,
    /// 1-bit conversion (default `floyd_steinberg`).
    #[serde(default)]
    dither: Option<Dither>,
    /// 0–255 cut-off (default 128).
    #[serde(default)]
    threshold: Option<u8>,
    /// -1.0 … 1.0
    #[serde(default)]
    brightness: Option<f64>,
    /// -1.0 … 1.0
    #[serde(default)]
    contrast: Option<f64>,
    #[serde(default)]
    invert: Option<bool>,
});

fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Lowercase hex sha256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `element.add_image`
#[derive(Debug)]
pub struct AddImage;

impl Command for AddImage {
    const ID: &'static str = "element.add_image";
    type Params = ImageParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Add image",
            "Embed an image (path or data_base64) as a content-addressed asset (sha256 hex) and \
             place it. Frame in mm. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ImageParams) -> Result<AddOutput, CommandError> {
        let bytes = match (&p.path, &p.data_base64) {
            (Some(path), None) => std::fs::read(path).map_err(|e| {
                let err = if e.kind() == std::io::ErrorKind::NotFound {
                    CommandError::not_found(format!("cannot read image {path}: {e}"))
                } else {
                    CommandError::bad_params(format!("cannot read image {path}: {e}"))
                };
                err.with_param("path")
            })?,
            (None, Some(b64)) => base64::engine::general_purpose::STANDARD
                .decode(b64.trim())
                .map_err(|e| {
                    CommandError::bad_params(format!("`data_base64` is not valid base64: {e}"))
                        .with_param("data_base64")
                })?,
            _ => {
                return Err(CommandError::bad_params(
                    "pass exactly one of `path` or `data_base64`",
                )
                .with_param("path"));
            }
        };
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err(CommandError::bad_params(format!(
                "image is {} bytes; must be 1 … {MAX_IMAGE_BYTES}",
                bytes.len()
            )));
        }
        let mime = match p
            .mime
            .clone()
            .or_else(|| sniff_mime(&bytes).map(str::to_owned))
        {
            Some(m) => m,
            None => {
                return Err(CommandError::bad_params("unrecognised image format")
                    .with_param("mime")
                    .with_accepted(["image/png", "image/jpeg", "image/bmp", "image/webp"]));
            }
        };
        let key = sha256_hex(&bytes);
        let frame = p.frame();
        let mut props = ImageProps {
            asset: key.clone(),
            fit: ImageFit::default(),
            dither: Dither::default(),
            threshold: 128,
            brightness: 0.0,
            contrast: 0.0,
            invert: false,
        };
        set_opt!(props, p: fit, dither, threshold, brightness, contrast, invert);
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        doc.assets.entry(key).or_insert_with(|| Asset {
            mime,
            data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        });
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Image(props))
    }
}

add_params!(RectParams {
    /// Outline width, mm (default 0.3; 0 = no outline).
    #[serde(default)]
    stroke_mm: Option<f64>,
    /// Fill black.
    #[serde(default)]
    fill: Option<bool>,
    /// Corner radius, mm.
    #[serde(default)]
    corner_radius_mm: Option<f64>,
});

/// `element.add_rect`
#[derive(Debug)]
pub struct AddRect;

impl Command for AddRect {
    const ID: &'static str = "element.add_rect";
    type Params = RectParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Add rectangle",
            "Add a rectangle. Frame and stroke in mm. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: RectParams) -> Result<AddOutput, CommandError> {
        let frame = p.frame();
        let mut s = ShapeProps::default();
        set_opt!(s, p: stroke_mm, fill, corner_radius_mm);
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Rect(s))
    }
}

add_params!(EllipseParams {
    /// Outline width, mm (default 0.3; 0 = no outline).
    #[serde(default)]
    stroke_mm: Option<f64>,
    /// Fill black.
    #[serde(default)]
    fill: Option<bool>,
});

/// `element.add_ellipse`
#[derive(Debug)]
pub struct AddEllipse;

impl Command for AddEllipse {
    const ID: &'static str = "element.add_ellipse";
    type Params = EllipseParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Add ellipse",
            "Add an ellipse inscribed in the frame. Frame and stroke in mm. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: EllipseParams) -> Result<AddOutput, CommandError> {
        let frame = p.frame();
        let mut s = ShapeProps::default();
        set_opt!(s, p: stroke_mm, fill);
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Ellipse(s))
    }
}

add_params!(LineParams {
    /// Line width, mm (default 0.3).
    #[serde(default)]
    stroke_mm: Option<f64>,
    /// Dash and gap length in mm; solid when absent.
    #[serde(default)]
    dash_mm: Option<[f64; 2]>,
});

/// `element.add_line`
#[derive(Debug)]
pub struct AddLine;

impl Command for AddLine {
    const ID: &'static str = "element.add_line";
    type Params = LineParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Add line",
            "Add a line from (x_mm, y_mm) to (x_mm + width_mm, y_mm + height_mm); one of \
             width_mm/height_mm may be 0. Stroke in mm. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: LineParams) -> Result<AddOutput, CommandError> {
        let frame = p.frame();
        let l = LineProps {
            stroke_mm: p.stroke_mm.unwrap_or(0.3),
            dash_mm: p.dash_mm,
        };
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Line(l))
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ElementOutput {
    /// The element after the change.
    pub element: Element,
}

fn ensure_unlocked(e: &Element) -> Result<(), CommandError> {
    if e.locked {
        Err(
            CommandError::disabled(format!("element {} is locked", e.id.0))
                .with_param("id")
                .with_hint("unlock it with element.set_props {props: {locked: false}}"),
        )
    } else {
        Ok(())
    }
}

fn kind_keys(kind: &ElementKind) -> Vec<String> {
    match kind {
        ElementKind::Text(_) => property_names::<TextProps>(),
        ElementKind::Barcode(_) => property_names::<BarcodeProps>(),
        ElementKind::Code2d(_) => property_names::<Code2dProps>(),
        ElementKind::Image(_) => property_names::<ImageProps>(),
        ElementKind::Rect(_) | ElementKind::Ellipse(_) => property_names::<ShapeProps>(),
        ElementKind::Line(_) => property_names::<LineProps>(),
    }
}

fn check_kind_props(kind: &ElementKind, v: Value) -> Option<CommandError> {
    fn try_as<T: serde::de::DeserializeOwned>(v: Value) -> Option<CommandError> {
        serde_path_to_error::deserialize::<_, T>(v).err().map(|e| {
            let path = e.path().to_string();
            let inner = e.into_inner().to_string();
            let path = if path == "." {
                "props".to_owned()
            } else {
                format!("props.{path}")
            };
            serde_error("bad props", &path, &inner)
        })
    }
    match kind {
        ElementKind::Text(_) => try_as::<TextProps>(v),
        ElementKind::Barcode(_) => try_as::<BarcodeProps>(v),
        ElementKind::Code2d(_) => try_as::<Code2dProps>(v),
        ElementKind::Image(_) => try_as::<ImageProps>(v),
        ElementKind::Rect(_) | ElementKind::Ellipse(_) => try_as::<ShapeProps>(v),
        ElementKind::Line(_) => try_as::<LineProps>(v),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetPropsParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub id: ElementId,
    /// Partial element JSON: frame fields (name, x_mm, y_mm, width_mm, height_mm,
    /// rotation_deg, locked, hidden) and the element type's props (e.g. text, font_size_pt,
    /// data, symbology, stroke_mm). `null` resets a field to its default.
    pub props: Map<String, Value>,
}

/// `element.set_props`
#[derive(Debug)]
pub struct SetProps;

impl Command for SetProps {
    const ID: &'static str = "element.set_props";
    type Params = SetPropsParams;
    type Output = ElementOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Set properties",
            "Merge partial props into an element: frame fields in mm/degrees and the type's own \
             props. Unknown keys and type mismatches are rejected; `type` and `id` cannot change. \
             Returns {element}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: SetPropsParams) -> Result<ElementOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let el = element_mut(doc, p.id)?;
        if p.props.keys().any(|k| k != "locked") {
            ensure_unlocked(el)?;
        }
        let keys = kind_keys(&el.kind);
        let mut v = serde_json::to_value(&*el)
            .map_err(|e| CommandError::internal(format!("cannot serialise element: {e}")))?;
        let Value::Object(obj) = &mut v else {
            return Err(CommandError::internal("element is not a JSON object"));
        };
        for (k, val) in p.props {
            if k == "id" || k == "type" {
                return Err(CommandError::bad_params(format!("`{k}` cannot be changed"))
                    .with_param(format!("props.{k}"))
                    .with_hint("delete the element and add a new one instead"));
            }
            if !FRAME_KEYS.contains(&k.as_str()) && !keys.contains(&k) {
                let mut accepted: Vec<String> =
                    FRAME_KEYS.iter().map(|s| (*s).to_owned()).collect();
                accepted.extend(keys.iter().cloned());
                return Err(CommandError::bad_params(format!(
                    "unknown property `{k}` for a {} element",
                    el.kind.type_name()
                ))
                .with_param(format!("props.{k}"))
                .with_accepted(accepted));
            }
            if val.is_null() {
                obj.remove(&k);
            } else {
                obj.insert(k, val);
            }
        }
        let kind_part: Map<String, Value> = obj
            .iter()
            .filter(|(k, _)| keys.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let new: Element = serde_path_to_error::deserialize(v.clone()).map_err(|e| {
            let path = e.path().to_string();
            if path == "." {
                // Flattened kind props lose their path; re-check them on their own.
                if let Some(err) = check_kind_props(&el.kind, Value::Object(kind_part)) {
                    return err;
                }
            }
            let path = if path == "." {
                "props".to_owned()
            } else {
                format!("props.{path}")
            };
            serde_error("bad props", &path, &e.into_inner().to_string())
        })?;
        validate_element(&new).map_err(|mut e| {
            e.param = e.param.map(|p| format!("props.{p}"));
            e
        })?;
        let mut new = new;
        new.rotation_deg = new.rotation_deg.rem_euclid(360.0);
        *el = new.clone();
        Ok(ElementOutput { element: new })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub id: ElementId,
    /// New left edge, mm.
    pub x_mm: f64,
    /// New top edge, mm.
    pub y_mm: f64,
}

/// `element.move`
#[derive(Debug)]
pub struct Move;

impl Command for Move {
    const ID: &'static str = "element.move";
    type Params = MoveParams;
    type Output = ElementOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Move",
            "Move an element so its unrotated frame's top-left is at (x_mm, y_mm), absolute mm.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: MoveParams) -> Result<ElementOutput, CommandError> {
        check_mm("x_mm", p.x_mm, false)?;
        check_mm("y_mm", p.y_mm, false)?;
        let el = element_mut(ctx.doc_mut(p.doc_id.as_deref())?, p.id)?;
        ensure_unlocked(el)?;
        el.x_mm = p.x_mm;
        el.y_mm = p.y_mm;
        Ok(ElementOutput {
            element: el.clone(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResizeParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub id: ElementId,
    /// New frame width, mm.
    pub width_mm: f64,
    /// New frame height, mm.
    pub height_mm: f64,
}

/// `element.resize`
#[derive(Debug)]
pub struct Resize;

impl Command for Resize {
    const ID: &'static str = "element.resize";
    type Params = ResizeParams;
    type Output = ElementOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Resize",
            "Set an element's frame size in mm, keeping its top-left corner.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ResizeParams) -> Result<ElementOutput, CommandError> {
        let el = element_mut(ctx.doc_mut(p.doc_id.as_deref())?, p.id)?;
        ensure_unlocked(el)?;
        el.width_mm = p.width_mm;
        el.height_mm = p.height_mm;
        validate_element(el)?;
        Ok(ElementOutput {
            element: el.clone(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RotateParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub id: ElementId,
    /// Absolute clockwise rotation about the frame centre, degrees; normalised to 0–360.
    pub rotation_deg: f64,
}

/// `element.rotate`
#[derive(Debug)]
pub struct Rotate;

impl Command for Rotate {
    const ID: &'static str = "element.rotate";
    type Params = RotateParams;
    type Output = ElementOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Rotate",
            "Set an element's absolute clockwise rotation in degrees about its frame centre.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: RotateParams) -> Result<ElementOutput, CommandError> {
        if !p.rotation_deg.is_finite() {
            return Err(CommandError::bad_params("`rotation_deg` must be finite")
                .with_param("rotation_deg"));
        }
        let el = element_mut(ctx.doc_mut(p.doc_id.as_deref())?, p.id)?;
        ensure_unlocked(el)?;
        el.rotation_deg = p.rotation_deg.rem_euclid(360.0);
        Ok(ElementOutput {
            element: el.clone(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Ids of the elements to delete (all must exist).
    pub ids: Vec<ElementId>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteOutput {
    pub deleted: Vec<ElementId>,
}

/// `element.delete`
#[derive(Debug)]
pub struct Delete;

impl Command for Delete {
    const ID: &'static str = "element.delete";
    type Params = DeleteParams;
    type Output = DeleteOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Delete",
            "Delete elements by id. Fails without changes if any id is unknown or locked.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: DeleteParams) -> Result<DeleteOutput, CommandError> {
        if p.ids.is_empty() {
            return Err(CommandError::bad_params("`ids` is empty").with_param("ids"));
        }
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        for id in &p.ids {
            ensure_unlocked(element(doc, *id)?)?;
        }
        for page in &mut doc.pages {
            page.elements.retain(|e| !p.ids.contains(&e.id));
        }
        let mut deleted = p.ids;
        deleted.sort();
        deleted.dedup();
        Ok(DeleteOutput { deleted })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DuplicateParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub id: ElementId,
    /// Horizontal offset of the copy, mm (default 2).
    #[serde(default)]
    pub dx_mm: Option<f64>,
    /// Vertical offset of the copy, mm (default 2).
    #[serde(default)]
    pub dy_mm: Option<f64>,
}

/// `element.duplicate`
#[derive(Debug)]
pub struct Duplicate;

impl Command for Duplicate {
    const ID: &'static str = "element.duplicate";
    type Params = DuplicateParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Duplicate",
            "Copy an element on top of the z-order, offset by dx_mm/dy_mm (default 2 mm each). \
             Returns {id} of the copy.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: DuplicateParams) -> Result<AddOutput, CommandError> {
        let dx = check_mm("dx_mm", p.dx_mm.unwrap_or(2.0), false)?;
        let dy = check_mm("dy_mm", p.dy_mm.unwrap_or(2.0), false)?;
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let mut copy = element(doc, p.id)?.clone();
        let page = doc
            .pages
            .iter()
            .position(|pg| pg.elements.iter().any(|e| e.id == p.id));
        let id = doc.alloc_id();
        copy.id = id;
        copy.x_mm += dx;
        copy.y_mm += dy;
        copy.locked = false;
        let page = page.unwrap_or(0);
        if let Some(pg) = doc.pages.get_mut(page) {
            pg.elements.push(copy);
        }
        Ok(AddOutput { id })
    }
}

/// Z-order target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReorderTo {
    /// On top of everything.
    Front,
    /// Below everything.
    Back,
    /// One step up.
    Forward,
    /// One step down.
    Backward,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReorderParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub id: ElementId,
    pub to: ReorderTo,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReorderOutput {
    /// New z-index on the page (0 = bottom).
    pub index: usize,
}

/// `element.reorder`
#[derive(Debug)]
pub struct Reorder;

impl Command for Reorder {
    const ID: &'static str = "element.reorder";
    type Params = ReorderParams;
    type Output = ReorderOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Reorder",
            "Change z-order: front, back, forward (one up) or backward (one down).",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ReorderParams) -> Result<ReorderOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        element(doc, p.id)?;
        for page in &mut doc.pages {
            let Some(i) = page.elements.iter().position(|e| e.id == p.id) else {
                continue;
            };
            let last = page.elements.len() - 1;
            let j = match p.to {
                ReorderTo::Front => last,
                ReorderTo::Back => 0,
                ReorderTo::Forward => (i + 1).min(last),
                ReorderTo::Backward => i.saturating_sub(1),
            };
            let e = page.elements.remove(i);
            page.elements.insert(j, e);
            return Ok(ReorderOutput { index: j });
        }
        Err(CommandError::internal("element vanished"))
    }
}
