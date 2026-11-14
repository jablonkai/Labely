//! `data.attach_images`: a folder of images as a data source, one record per image (ADR-0018).

use base64::Engine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::data::{DataSummary, attach};
use super::element::{
    AddOutput, Frame, MAX_IMAGE_BYTES, add, read_image_file, sha256_hex, sniff_mime,
};
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::document::{Asset, DataSource, Dither, ElementId, ElementKind, ImageFit, ImageProps};

/// Most images attached at once.
pub const MAX_IMAGES: usize = 500;
/// Column with each record's file name.
const FILE_COLUMN: &str = "file";

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachImagesParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Image files (PNG, JPEG, BMP, WebP), one record each, in this order.
    pub paths: Vec<String>,
    /// Column holding each record's image (default `image`).
    #[serde(default)]
    pub column: Option<String>,
    /// Add an image element filling the label and bound to the column, unless one already
    /// uses it (default true).
    #[serde(default)]
    pub place: Option<bool>,
    /// Fit of the placed element (default `contain`).
    #[serde(default)]
    pub fit: Option<ImageFit>,
    /// 1-bit conversion of the placed element (default `floyd_steinberg`).
    #[serde(default)]
    pub dither: Option<Dither>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AttachImagesOutput {
    #[serde(flatten)]
    pub data: DataSummary,
    /// The image element that was placed, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<ElementId>,
}

/// `data.attach_images`
#[derive(Debug)]
pub struct AttachImages;

impl Command for AttachImages {
    const ID: &'static str = "data.attach_images";
    type Params = AttachImagesParams;
    type Output = AttachImagesOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Attach images",
            "Embed image files as the data source, replacing any other: one record per image \
             with columns `image` (the asset) and `file` (the file name). An image element \
             whose asset is `{{image}}` shows each record's picture; one filling the label is \
             placed unless it exists (place: false skips it). Printing prints one label per \
             image. 1–500 paths.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: AttachImagesParams) -> Result<AttachImagesOutput, CommandError> {
        let column = p.column.unwrap_or_else(|| "image".to_owned());
        if column.trim().is_empty() || column.contains(['{', '}']) || column == FILE_COLUMN {
            return Err(CommandError::bad_params(format!(
                "column must be a plain name other than `{FILE_COLUMN}`"
            ))
            .with_param("column"));
        }
        if p.paths.is_empty() || p.paths.len() > MAX_IMAGES {
            return Err(CommandError::bad_params(format!(
                "pass 1–{MAX_IMAGES} image paths, got {}",
                p.paths.len()
            ))
            .with_param("paths"));
        }
        let mut assets = Vec::with_capacity(p.paths.len());
        let mut rows = Vec::with_capacity(p.paths.len());
        for (i, path) in p.paths.iter().enumerate() {
            let param = format!("paths.{i}");
            let bytes = read_image_file(ctx, &param, path)?;
            if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
                return Err(CommandError::bad_params(format!(
                    "{path} is {} bytes; images must be 1 … {MAX_IMAGE_BYTES}",
                    bytes.len()
                ))
                .with_param(param));
            }
            let Some(mime) = sniff_mime(&bytes) else {
                return Err(CommandError::bad_params(format!(
                    "{path} is not a PNG, JPEG, BMP or WebP"
                ))
                .with_param(param));
            };
            let key = sha256_hex(&bytes);
            let file = std::path::Path::new(path)
                .file_name()
                .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
            rows.push(vec![key.clone(), file]);
            assets.push((
                key,
                Asset {
                    mime: mime.to_owned(),
                    data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
                },
            ));
        }
        let origin = match p.paths.len() {
            1 => rows[0][1].clone(),
            n => format!("{n} images"),
        };
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        for (key, asset) in assets {
            doc.assets.entry(key).or_insert(asset);
        }
        let binding = format!("{{{{{column}}}}}");
        let placed = doc
            .elements()
            .any(|e| matches!(&e.kind, ElementKind::Image(i) if i.asset == binding));
        let (w, h) = (doc.label.width_mm, doc.label.height_mm);
        let source = DataSource {
            origin: Some(origin),
            columns: vec![column, FILE_COLUMN.to_owned()],
            rows,
        };
        let data = attach(ctx, p.doc_id.as_deref(), source, Vec::new())?;
        let element_id = if p.place.unwrap_or(true) && !placed {
            let props = ImageProps {
                asset: binding,
                fit: p.fit.unwrap_or_default(),
                dither: p.dither.unwrap_or_default(),
                threshold: 128,
                brightness: 0.0,
                contrast: 0.0,
                invert: false,
            };
            let frame = Frame {
                name: None,
                x_mm: 0.0,
                y_mm: 0.0,
                width_mm: w,
                height_mm: h,
                rotation_deg: 0.0,
            };
            let AddOutput { id } = add(ctx, p.doc_id.as_deref(), frame, ElementKind::Image(props))?;
            Some(id)
        } else {
            None
        };
        Ok(AttachImagesOutput { data, element_id })
    }
}
