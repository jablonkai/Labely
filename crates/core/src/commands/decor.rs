//! Icons, borders and reading codes from images: `element.add_icon`, `element.add_border`,
//! `icon.list`, `font.list`, `font.add`, `code.decode`.

use base64::Engine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::element::{AddOutput, Frame, add, add_params};
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::document::{BorderProps, BorderStyle, DEFAULT_FONT, ElementKind, IconProps};
use crate::services::IconInfo;

/// Largest font file `font.add` reads.
const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;

/// Largest image `code.decode` reads.
const MAX_DECODE_BYTES: usize = 5 * 1024 * 1024;

add_params!(IconParams {
    /// Icon name from icon.list, e.g. `fragile`.
    icon: String,
    /// Keep the icon's aspect ratio (default true).
    #[serde(default)]
    keep_aspect: Option<bool>,
    #[serde(default)]
    invert: Option<bool>,
});

/// `element.add_icon`
#[derive(Debug)]
pub struct AddIcon;

impl Command for AddIcon {
    const ID: &'static str = "element.add_icon";
    type Params = IconParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Icon",
            "Add a built-in icon (names from icon.list), scaled into the frame. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: IconParams) -> Result<AddOutput, CommandError> {
        let icons = ctx.services().renderer.icons();
        let name = p.icon.trim().to_owned();
        if !icons.is_empty() && !icons.iter().any(|i| i.name == name) {
            return Err(CommandError::not_found(format!("no icon named `{name}`"))
                .with_param("icon")
                .with_accepted(icons.into_iter().map(|i| i.name))
                .with_hint("run icon.list"));
        }
        let frame = p.frame();
        let props = IconProps {
            icon: name,
            keep_aspect: p.keep_aspect.unwrap_or(true),
            invert: p.invert.unwrap_or(false),
        };
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Icon(props))
    }
}

add_params!(BorderParams {
    /// Frame style (default `single`).
    #[serde(default)]
    style: Option<BorderStyle>,
    /// Line width in mm (default 0.5).
    #[serde(default)]
    stroke_mm: Option<f64>,
    /// Corner radius in mm (default 0).
    #[serde(default)]
    corner_radius_mm: Option<f64>,
});

/// `element.add_border`
#[derive(Debug)]
pub struct AddBorder;

impl Command for AddBorder {
    const ID: &'static str = "element.add_border";
    type Params = BorderParams;
    type Output = AddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Border",
            "Add a decorative frame drawn inside its box: single, double, thick_thin, dashed, \
             dotted, corners, ticket or scallop. Usually as large as the label. Returns {id}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: BorderParams) -> Result<AddOutput, CommandError> {
        let frame = p.frame();
        let mut b = BorderProps::default();
        if let Some(v) = p.style {
            b.style = v;
        }
        if let Some(v) = p.stroke_mm {
            b.stroke_mm = v;
        }
        if let Some(v) = p.corner_radius_mm {
            b.corner_radius_mm = v;
        }
        add(ctx, p.doc_id.as_deref(), frame, ElementKind::Border(b))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IconListParams {
    /// Only this category.
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct IconListOutput {
    pub icons: Vec<IconInfo>,
}

/// `icon.list`
#[derive(Debug)]
pub struct IconList;

impl Command for IconList {
    const ID: &'static str = "icon.list";
    type Params = IconListParams;
    type Output = IconListOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "List icons",
            "Names and categories of the built-in icons for element.add_icon.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: IconListParams) -> Result<IconListOutput, CommandError> {
        let mut icons = ctx.services().renderer.icons();
        if let Some(c) = &p.category {
            icons.retain(|i| &i.category == c);
        }
        Ok(IconListOutput { icons })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FontListParams {}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FontListOutput {
    /// Family names usable as `font_family`, sorted.
    pub fonts: Vec<String>,
}

/// `font.list`
#[derive(Debug)]
pub struct FontList;

impl Command for FontList {
    const ID: &'static str = "font.list";
    type Params = FontListParams;
    type Output = FontListOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "List fonts",
            "Font family names the renderer can draw, for a text or table `font_family`. Other \
             names fall back to the default font and doc.check reports it.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, _p: FontListParams) -> Result<FontListOutput, CommandError> {
        let mut fonts = ctx.services().renderer.fonts();
        if fonts.is_empty() {
            fonts.push(DEFAULT_FONT.to_owned());
        }
        Ok(FontListOutput { fonts })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FontAddParams {
    /// Font file: TrueType (.ttf), OpenType (.otf) or a collection (.ttc), at most 64 MiB.
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FontAddOutput {
    /// Family names the file added, usable as `font_family`.
    pub added: Vec<String>,
    /// Every family now available, sorted.
    pub fonts: Vec<String>,
}

/// `font.add`
#[derive(Debug)]
pub struct FontAdd;

impl Command for FontAdd {
    const ID: &'static str = "font.add";
    type Params = FontAddParams;
    type Output = FontAddOutput;

    fn meta() -> CommandMeta {
        CommandMeta::ui(
            "Add font file",
            "Load a TTF, OTF or TTC font file for this session, so its families can be used as \
             a text or table `font_family`. Documents store only the family name: a label \
             that uses the font renders with it only where the font is loaded or installed.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: FontAddParams) -> Result<FontAddOutput, CommandError> {
        use std::io::Read as _;
        let path = ctx.check_path("path", &p.path, false)?;
        let read_err = |e: std::io::Error| {
            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    CommandError::not_found(format!("cannot read {}: {e}", p.path))
                }
                _ => CommandError::bad_params(format!("cannot read {}: {e}", p.path)),
            }
            .with_param("path")
        };
        let file = std::fs::File::open(&path).map_err(read_err)?;
        let mut bytes = Vec::new();
        file.take(MAX_FONT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(read_err)?;
        if bytes.len() as u64 > MAX_FONT_BYTES {
            return Err(CommandError::bad_params(format!(
                "{} is larger than {} MiB",
                p.path,
                MAX_FONT_BYTES / 1024 / 1024
            ))
            .with_param("path"));
        }
        let renderer = &ctx.services().renderer;
        let added = renderer
            .add_font(bytes)
            .map_err(|e| CommandError::bad_params(format!("{}: {e}", p.path)).with_param("path"))?;
        Ok(FontAddOutput {
            added,
            fonts: renderer.fonts(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecodeParams {
    /// Image file (PNG, JPEG, BMP, WebP; at most 5 MiB). Pass exactly one of `path` or
    /// `data_base64`.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub data_base64: Option<String>,
}

/// A code found in an image.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FoundCode {
    /// `barcode` or `code2d`: the element type that recreates it.
    pub kind: String,
    /// Symbology name as used by element.add_barcode / add_code2d (e.g. `qr`, `ean13`).
    pub symbology: String,
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DecodeOutput {
    pub codes: Vec<FoundCode>,
}

/// `code.decode`
#[derive(Debug)]
pub struct CodeDecode;

impl Command for CodeDecode {
    const ID: &'static str = "code.decode";
    type Params = DecodeParams;
    type Output = DecodeOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Read code from image",
            "Decode the barcodes and 2D codes in an image (path or data_base64). Each result \
             gives kind, symbology and text, ready for element.add_barcode or add_code2d.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: DecodeParams) -> Result<DecodeOutput, CommandError> {
        let bytes = match (&p.path, &p.data_base64) {
            (Some(path), None) => {
                std::fs::read(ctx.check_path("path", path, false)?).map_err(|e| {
                    let err = if e.kind() == std::io::ErrorKind::NotFound {
                        CommandError::not_found(format!("cannot read {path}: {e}"))
                    } else {
                        CommandError::bad_params(format!("cannot read {path}: {e}"))
                    };
                    err.with_param("path")
                })?
            }
            (None, Some(b64)) => base64::engine::general_purpose::STANDARD
                .decode(b64.trim())
                .map_err(|e| {
                    CommandError::bad_params(format!("`data_base64` is not valid base64: {e}"))
                        .with_param("data_base64")
                })?,
            _ => {
                return Err(CommandError::bad_params(
                    "pass exactly one of `path` or `data_base64`",
                ));
            }
        };
        if bytes.len() > MAX_DECODE_BYTES {
            return Err(CommandError::bad_params("the image is larger than 5 MiB"));
        }
        let found = ctx
            .services()
            .renderer
            .decode_image(&bytes)
            .map_err(|e| CommandError::bad_params(format!("cannot read the image: {e}")))?;
        if found.is_empty() {
            ctx.warn("no barcode or 2D code was found in the image");
        }
        let codes = found
            .into_iter()
            .map(|c| FoundCode {
                kind: if matches!(c.format.as_str(), "qr" | "data_matrix" | "pdf417" | "aztec") {
                    "code2d"
                } else {
                    "barcode"
                }
                .to_owned(),
                symbology: c.format,
                text: c.text,
            })
            .collect();
        Ok(DecodeOutput { codes })
    }
}
