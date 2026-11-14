//! `render.preview`, `doc.check`.

use base64::Engine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{DEFAULT_DPI, check_dpi, doc_model, interpolate, render_options};
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::document::{Document, Element, ElementId, ElementKind};
use crate::services::{RenderError, RenderIssue};

fn render_error(e: RenderError) -> CommandError {
    match e {
        RenderError::NoSuchPage(p) => CommandError::bad_params(e.to_string())
            .with_param("page")
            .with_hint(format!("pages are 0-based; page {p} does not exist")),
        RenderError::BadSize { .. } => CommandError::bad_params(e.to_string())
            .with_hint("fix the label size with doc.set_label"),
        RenderError::Other(_) => CommandError::internal(format!("render failed: {e}")),
    }
}

/// The dpi to use: explicit, else the document's model, else 203.
fn pick_dpi(ctx: &Ctx<'_>, doc: &Document, dpi: Option<u32>) -> Result<u32, CommandError> {
    match dpi {
        Some(d) => check_dpi(d),
        None => Ok(doc_model(ctx, doc).map_or(DEFAULT_DPI, |m| m.dpi)),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Resolution in dots per inch; defaults to the label's printer model dpi, else 203.
    #[serde(default)]
    pub dpi: Option<u32>,
    /// 0-based page index (default 0).
    #[serde(default)]
    pub page: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PreviewOutput {
    /// PNG of the 1-bit print raster, standard base64.
    pub png_base64: String,
    pub width_dots: u32,
    pub height_dots: u32,
    pub dpi: u32,
}

/// `render.preview`
#[derive(Debug)]
pub struct RenderPreview;

impl Command for RenderPreview {
    const ID: &'static str = "render.preview";
    type Params = PreviewParams;
    type Output = PreviewOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Preview",
            "Render a page to the exact 1-bit print raster and return it as base64 PNG with its \
             size in dots. dpi defaults to the label's printer model (else 203). Variables are \
             substituted.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: PreviewParams) -> Result<PreviewOutput, CommandError> {
        let doc = ctx.doc_arc(p.doc_id.as_deref())?;
        let dpi = pick_dpi(ctx, &doc, p.dpi)?;
        let opts = render_options(&doc, dpi, p.page.unwrap_or(0));
        let renderer = ctx.services().renderer.clone();
        let out = renderer.render(&doc, &opts).map_err(render_error)?;
        let png = renderer.to_png(&out.bitmap).map_err(render_error)?;
        if !out.issues.is_empty() {
            ctx.warn(format!(
                "{} render issue(s); run doc.check for details",
                out.issues.len()
            ));
        }
        Ok(PreviewOutput {
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            width_dots: out.bitmap.width(),
            height_dots: out.bitmap.height(),
            dpi,
        })
    }
}

/// How serious a check finding is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// One `doc.check` finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckIssue {
    /// Rule id, e.g. `out_of_bounds`, `text_overflow`, `code_unreadable`.
    pub rule: String,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_id: Option<ElementId>,
    pub message: String,
    pub hint: String,
}

impl CheckIssue {
    fn new(
        rule: &str,
        severity: Severity,
        element_id: Option<ElementId>,
        message: String,
        hint: &str,
    ) -> Self {
        Self {
            rule: rule.to_owned(),
            severity,
            element_id,
            message,
            hint: hint.to_owned(),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Check resolution in dpi; defaults to the label's printer model dpi, else 203.
    #[serde(default)]
    pub dpi: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CheckOutput {
    /// True when there are no `error` issues.
    pub ok: bool,
    pub dpi: u32,
    pub issues: Vec<CheckIssue>,
}

/// Axis-aligned bounds of the rotated frame: (min_x, min_y, max_x, max_y) in mm.
fn rotated_bounds(e: &Element) -> (f64, f64, f64, f64) {
    let (s, c) = e.rotation_deg.to_radians().sin_cos();
    let (hw, hh) = (e.width_mm / 2.0, e.height_mm / 2.0);
    let ex = (hw * c).abs() + (hh * s).abs();
    let ey = (hw * s).abs() + (hh * c).abs();
    let (cx, cy) = (e.x_mm + hw, e.y_mm + hh);
    (cx - ex, cy - ey, cx + ex, cy + ey)
}

/// Run all Phase-1 rules on `doc` at `dpi`.
pub fn check_document(ctx: &Ctx<'_>, doc: &Document, dpi: u32) -> Vec<CheckIssue> {
    const EPS: f64 = 0.01;
    let mut issues = Vec::new();
    let label = &doc.label;

    for e in doc.elements().filter(|e| !e.hidden) {
        let (x0, y0, x1, y1) = rotated_bounds(e);
        if x0 < -EPS || y0 < -EPS || x1 > label.width_mm + EPS || y1 > label.height_mm + EPS {
            issues.push(CheckIssue::new(
                "out_of_bounds",
                Severity::Warning,
                Some(e.id),
                format!(
                    "{} element {} extends outside the {}×{} mm label",
                    e.kind.type_name(),
                    e.id.0,
                    label.width_mm,
                    label.height_mm
                ),
                "move or resize it inside the label (element.move / element.resize)",
            ));
        }
    }

    if let Some(m) = doc_model(ctx, doc) {
        if label.width_mm > m.max_width_mm + EPS {
            issues.push(CheckIssue::new(
                "label_too_wide",
                Severity::Error,
                None,
                format!(
                    "label width {} mm exceeds the {} print width of {} mm",
                    label.width_mm, m.name, m.max_width_mm
                ),
                "use narrower stock (doc.set_label width_mm) or another printer model",
            ));
        }
        if let Some(d) = label.density
            && !(m.density_min..=m.density_max).contains(&d)
        {
            issues.push(CheckIssue::new(
                "density_out_of_range",
                Severity::Error,
                None,
                format!(
                    "density {d} is outside the {} range {}–{}",
                    m.name, m.density_min, m.density_max
                ),
                "set a density in range with doc.set_label, or 0 for the model default",
            ));
        }
    }

    let renderer = ctx.services().renderer.clone();
    let opts = render_options(doc, dpi, 0);
    let mut pages = Vec::new();
    for page in 0..doc.pages.len() {
        let opts = super::render_options(doc, dpi, page);
        match renderer.render(doc, &opts) {
            Ok(out) => pages.push(out),
            Err(e) => issues.push(CheckIssue::new(
                "render_failed",
                Severity::Error,
                None,
                format!("page {page} could not be rendered: {e}"),
                "fix the label size or the reported element",
            )),
        }
    }

    let mut encode_failed = Vec::new();
    for issue in pages.iter().flat_map(|p| p.issues.iter()) {
        issues.push(match issue {
            RenderIssue::TextOverflow { element_id } => CheckIssue::new(
                "text_overflow",
                Severity::Error,
                Some(*element_id),
                format!("text of element {} does not fit its box", element_id.0),
                "enlarge the box, reduce font_size_pt, or set fit: shrink",
            ),
            RenderIssue::TextShrunk {
                element_id,
                font_size_pt,
            } => CheckIssue::new(
                "text_shrunk",
                if *font_size_pt < 5.0 {
                    Severity::Warning
                } else {
                    Severity::Info
                },
                Some(*element_id),
                format!(
                    "text of element {} was shrunk to {font_size_pt:.1} pt",
                    element_id.0
                ),
                "below 5 pt is hard to read on thermal labels; enlarge the box or shorten the text",
            ),
            RenderIssue::EncodeFailed {
                element_id,
                message,
            } => {
                encode_failed.push(*element_id);
                CheckIssue::new(
                    "encode_failed",
                    Severity::Error,
                    Some(*element_id),
                    format!("element {} cannot be encoded: {message}", element_id.0),
                    "fix the data for the symbology, or pick another symbology",
                )
            }
            RenderIssue::ModuleWidth { element_id, dots } if *dots < 2.0 => CheckIssue::new(
                "module_too_narrow",
                Severity::Warning,
                Some(*element_id),
                format!(
                    "narrowest module of element {} is {dots:.2} dots (< 2) at {dpi} dpi",
                    element_id.0
                ),
                "make the code wider or the data shorter",
            ),
            RenderIssue::ModuleWidth { .. } => continue,
            RenderIssue::BadAsset {
                element_id,
                message,
            } => CheckIssue::new(
                "bad_asset",
                Severity::Error,
                Some(*element_id),
                format!("image of element {} is unusable: {message}", element_id.0),
                "re-add the image with element.add_image",
            ),
            RenderIssue::FontFallback {
                element_id,
                requested,
            } => CheckIssue::new(
                "font_fallback",
                Severity::Info,
                Some(*element_id),
                format!(
                    "font `{requested}` of element {} is unavailable; a fallback was used",
                    element_id.0
                ),
                "pick a bundled font family",
            ),
        });
    }

    // Barcode round-trip: every visible code must decode from the rendered raster.
    for (page_idx, out) in pages.iter().enumerate() {
        let decoded = renderer.decode_codes(&out.bitmap);
        let Some(page) = doc.pages.get(page_idx) else {
            continue;
        };
        for e in page.elements.iter().filter(|e| !e.hidden) {
            let data = match &e.kind {
                ElementKind::Barcode(b) => &b.data,
                ElementKind::Code2d(c) => &c.data,
                _ => continue,
            };
            if encode_failed.contains(&e.id) {
                continue;
            }
            let expected = interpolate(data, &opts.values);
            if !decoded.iter().any(|d| d.text == expected) {
                issues.push(CheckIssue::new(
                    "code_unreadable",
                    Severity::Warning,
                    Some(e.id),
                    format!(
                        "{} element {} (data {expected:?}) was not decoded from the {dpi} dpi raster",
                        e.kind.type_name(),
                        e.id.0
                    ),
                    "make the code larger, keep it inside the label, and avoid overlaps",
                ));
            }
        }
    }
    issues
}

/// `doc.check`
#[derive(Debug)]
pub struct DocCheck;

impl Command for DocCheck {
    const ID: &'static str = "doc.check";
    type Params = CheckParams;
    type Output = CheckOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Check document",
            "Find print problems: elements outside the label, text overflow or shrinking, \
             barcode encode failures, modules < 2 dots, codes that do not decode from the \
             rendered raster, label wider than the model's head, density out of model range. \
             Returns {ok, dpi, issues:[{rule, severity, element_id?, message, hint}]}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: CheckParams) -> Result<CheckOutput, CommandError> {
        let doc = ctx.doc_arc(p.doc_id.as_deref())?;
        let dpi = pick_dpi(ctx, &doc, p.dpi)?;
        let issues = check_document(ctx, &doc, dpi);
        let ok = !issues.iter().any(|i| i.severity == Severity::Error);
        Ok(CheckOutput { ok, dpi, issues })
    }
}
