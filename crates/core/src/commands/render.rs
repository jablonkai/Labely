//! `render.preview`, `doc.check`.

use base64::Engine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::data::record_index;
use super::{
    DEFAULT_DPI, check_dpi, check_mm, doc_model, interpolate, render_options, render_options_for,
};
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::document::{Document, Element, ElementId, ElementKind};
use crate::services::{RenderError, RenderIssue};

pub(crate) fn render_error(e: RenderError) -> CommandError {
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
pub(crate) fn pick_dpi(
    ctx: &Ctx<'_>,
    doc: &Document,
    dpi: Option<u32>,
) -> Result<u32, CommandError> {
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
    /// Data record to show, 1-based (default 1). Serials take that record's position.
    #[serde(default)]
    pub record: Option<usize>,
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
             size in dots. dpi defaults to the label's printer model (else 203). Bindings are \
             filled from `record` (1-based, default 1), serials and variables.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: PreviewParams) -> Result<PreviewOutput, CommandError> {
        let doc = ctx.doc_arc(p.doc_id.as_deref())?;
        let dpi = pick_dpi(ctx, &doc, p.dpi)?;
        let record = p.record.unwrap_or(1);
        let row = record_index(&doc, record)?;
        let opts = render_options_for(&doc, dpi, p.page.unwrap_or(0), row, record as u64 - 1);
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
    /// Safe margin from the label edge in mm; elements inside it are reported. Default 0.
    #[serde(default)]
    pub margin_mm: Option<f64>,
    /// Printer whose loaded paper (RFID) is compared with the label. Defaults to the only
    /// connected printer, if exactly one is connected.
    #[serde(default)]
    pub address: Option<String>,
}

/// Inputs of [`check_document`] besides the document.
#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    pub dpi: u32,
    pub margin_mm: f64,
    /// Paper the printer reported, if known.
    pub paper: Option<crate::services::PaperInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CheckOutput {
    /// True when there are no `error` issues.
    pub ok: bool,
    pub dpi: u32,
    pub issues: Vec<CheckIssue>,
}

fn rotated_bounds(e: &Element) -> (f64, f64, f64, f64) {
    e.bounds()
}

fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// Run all rules of architecture §3.4 on `doc`.
pub fn check_document(ctx: &Ctx<'_>, doc: &Document, o: &CheckOptions) -> Vec<CheckIssue> {
    const EPS: f64 = 0.01;
    let dpi = o.dpi;
    let mut issues = Vec::new();
    let label = &doc.label;
    let visible: Vec<&Element> = doc.elements().filter(|e| !e.hidden).collect();
    let model = doc_model(ctx, doc);

    if o.margin_mm > 0.0 {
        let m = o.margin_mm;
        for e in &visible {
            let (x0, y0, x1, y1) = rotated_bounds(e);
            let inside_label = x0 >= -EPS
                && y0 >= -EPS
                && x1 <= label.width_mm + EPS
                && y1 <= label.height_mm + EPS;
            if inside_label
                && (x0 < m - EPS
                    || y0 < m - EPS
                    || x1 > label.width_mm - m + EPS
                    || y1 > label.height_mm - m + EPS)
            {
                issues.push(CheckIssue::new(
                    "in_margin",
                    Severity::Warning,
                    Some(e.id),
                    format!("element {} is within the {m} mm margin", e.id.0),
                    "move it further from the label edge",
                ));
            }
        }
    }

    if let Some(m) = &model {
        if dpi != m.dpi {
            issues.push(CheckIssue::new(
                "dpi_mismatch",
                Severity::Warning,
                None,
                format!(
                    "checked at {dpi} dpi but the {} prints at {} dpi",
                    m.name, m.dpi
                ),
                "omit dpi to check at the model resolution",
            ));
        }
        let head_mm = f64::from(m.head_dots) * 25.4 / f64::from(m.dpi);
        if label.width_mm > head_mm + EPS {
            let left = (label.width_mm - head_mm) / 2.0;
            let right = left + head_mm;
            for e in &visible {
                let (x0, _, x1, _) = rotated_bounds(e);
                if x0 < left - EPS || x1 > right + EPS {
                    issues.push(CheckIssue::new(
                        "outside_printable",
                        Severity::Warning,
                        Some(e.id),
                        format!(
                            "element {} reaches outside the {head_mm:.1} mm printable width of the {}",
                            e.id.0, m.name
                        ),
                        "keep elements within the centred printable area",
                    ));
                }
            }
        }
    }

    if let Some(paper) = &o.paper {
        let known = super::paper::linked_paper(&*ctx.session, &paper.barcode);
        match known {
            Some(k) if !k.matches(label) => issues.push(CheckIssue::new(
                "paper_mismatch",
                Severity::Warning,
                None,
                format!(
                    "the printer has {} loaded ({} × {} mm, {:?}; RFID {}), the label is \
                     {} × {} mm, {:?}",
                    k.name,
                    k.width_mm,
                    k.height_mm,
                    k.media,
                    paper.barcode,
                    label.width_mm,
                    label.height_mm,
                    label.media
                ),
                &format!(
                    "use the loaded paper with paper.apply {{\"id\": \"{}\"}}",
                    k.id
                ),
            )),
            Some(_) => {}
            None => {
                if let Some(media) = paper.media
                    && media != label.media
                {
                    issues.push(CheckIssue::new(
                        "paper_mismatch",
                        Severity::Warning,
                        None,
                        format!(
                            "the printer reports {media:?} paper (RFID {}), the label is set \
                             to {:?}",
                            paper.barcode, label.media
                        ),
                        "set the label media with doc.set_label or load matching paper",
                    ));
                }
            }
        }
    }

    // Bindings: `{{name}}` must name a column, serial or variable, and should not be empty.
    if let Some(d) = &doc.data
        && d.rows.is_empty()
    {
        issues.push(CheckIssue::new(
            "data_empty",
            Severity::Warning,
            None,
            "the data source has no records".to_owned(),
            "attach data with rows, or remove it with data.detach",
        ));
    }
    for e in doc.elements() {
        let Ok(v) = serde_json::to_value(&e.kind) else {
            continue;
        };
        let mut names = Vec::new();
        collect_bindings(&v, &mut names);
        for name in names {
            if let Some(d) = doc.data.as_ref().filter(|d| d.columns.contains(&name)) {
                let empty: Vec<usize> = (0..d.rows.len())
                    .filter(|&r| d.cell(r, &name).is_some_and(|c| c.trim().is_empty()))
                    .collect();
                if let Some(first) = empty.first() {
                    issues.push(CheckIssue::new(
                        "binding_empty",
                        Severity::Info,
                        Some(e.id),
                        format!(
                            "column `{name}` used by element {} is empty in {} of {} records \
                             (first: record {})",
                            e.id.0,
                            empty.len(),
                            d.rows.len(),
                            first + 1
                        ),
                        "fill the cells, or skip those records with print.submit {records}",
                    ));
                }
                continue;
            }
            if doc.serials.contains_key(&name) {
                continue;
            }
            if crate::datetime::is_date_binding(&name) {
                if let Err(why) = crate::datetime::check(&name) {
                    issues.push(CheckIssue::new(
                        "binding_invalid",
                        Severity::Error,
                        Some(e.id),
                        format!("element {} uses {{{{{name}}}}}: {why}", e.id.0),
                        "use {{now}}, {{now:%d.%m.%Y}} or {{now+30d:%Y-%m-%d}}",
                    ));
                }
                continue;
            }
            match doc.variables.get(&name) {
                None => issues.push(CheckIssue::new(
                    "binding_missing",
                    Severity::Warning,
                    Some(e.id),
                    format!(
                        "element {} uses {{{{{name}}}}} but no column, serial or variable has \
                         that name",
                        e.id.0
                    ),
                    "attach data, set it with variable.set or data.set_serial, or fix the name",
                )),
                Some(val) if val.is_empty() => issues.push(CheckIssue::new(
                    "binding_empty",
                    Severity::Info,
                    Some(e.id),
                    format!("variable `{name}` used by element {} is empty", e.id.0),
                    "give it a value with variable.set",
                )),
                Some(_) => {}
            }
        }
    }

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

    if let Some(m) = &model {
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
            RenderIssue::LowContrast {
                element_id,
                black_percent,
            } => CheckIssue::new(
                "low_contrast",
                Severity::Warning,
                Some(*element_id),
                format!(
                    "image of element {} is {black_percent}% black after dithering",
                    element_id.0
                ),
                "adjust brightness, contrast or threshold (element.set_props)",
            ),
            RenderIssue::CodeLayout {
                element_id,
                module_dots,
                slack_dots,
                quiet_modules,
            } => {
                let Some(e) = doc.element(*element_id) else {
                    continue;
                };
                let horizontal_only = matches!(e.kind, ElementKind::Barcode(_));
                let need_mm = f64::from(quiet_modules * module_dots) * 25.4 / f64::from(dpi);
                let have_mm = f64::from(*slack_dots) * 25.4 / f64::from(dpi)
                    + clearance(e, &visible, label, horizontal_only);
                if have_mm + EPS < need_mm {
                    CheckIssue::new(
                        "quiet_zone",
                        Severity::Warning,
                        Some(*element_id),
                        format!(
                            "{} element {} has {have_mm:.2} mm of quiet zone, {need_mm:.2} mm ({quiet_modules} modules) is recommended",
                            e.kind.type_name(),
                            element_id.0
                        ),
                        "leave blank space around the code or make its box wider than the symbol",
                    )
                } else {
                    continue;
                }
            }
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

/// Collect `{{name}}` binding names from every string in `v`.
fn collect_bindings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => {
            let mut rest = s.as_str();
            while let Some(start) = rest.find("{{") {
                let after = &rest[start + 2..];
                let Some(end) = after.find("}}") else { break };
                let name = after[..end].trim().to_owned();
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
                rest = &after[end + 2..];
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_bindings(x, out)),
        serde_json::Value::Object(m) => m.values().for_each(|x| collect_bindings(x, out)),
        _ => {}
    }
}

/// Free space in mm around `e` on its quiet-zone sides, up to the label edge or the nearest
/// other visible element. `horizontal_only` for 1D codes (left and right of the bars).
fn clearance(
    e: &Element,
    others: &[&Element],
    label: &crate::LabelSpec,
    horizontal_only: bool,
) -> f64 {
    let b = rotated_bounds(e);
    let turned = {
        let r = e.rotation_deg.rem_euclid(180.0);
        (45.0..135.0).contains(&r)
    };
    // Sides to measure: (dx_sign, dy_sign).
    let sides: &[(i8, i8)] = match (horizontal_only, turned) {
        (true, false) => &[(-1, 0), (1, 0)],
        (true, true) => &[(0, -1), (0, 1)],
        (false, _) => &[(-1, 0), (1, 0), (0, -1), (0, 1)],
    };
    let mut min = f64::INFINITY;
    for &(dx, dy) in sides {
        let mut gap = match (dx, dy) {
            (-1, _) => b.0,
            (1, _) => label.width_mm - b.2,
            (_, -1) => b.1,
            _ => label.height_mm - b.3,
        };
        for o in others.iter().filter(|o| o.id != e.id) {
            let ob = rotated_bounds(o);
            if overlaps(b, ob) {
                continue; // overlapping elements are a decode problem, not a quiet-zone one
            }
            let in_lane = if dx != 0 {
                ob.1 < b.3 && b.1 < ob.3
            } else {
                ob.0 < b.2 && b.0 < ob.2
            };
            if !in_lane {
                continue;
            }
            let d = match (dx, dy) {
                (-1, _) if ob.2 <= b.0 => b.0 - ob.2,
                (1, _) if ob.0 >= b.2 => ob.0 - b.2,
                (_, -1) if ob.3 <= b.1 => b.1 - ob.3,
                (_, 1) if ob.1 >= b.3 => ob.1 - b.3,
                _ => continue,
            };
            gap = gap.min(d);
        }
        min = min.min(gap.max(0.0));
    }
    if min.is_finite() { min } else { 0.0 }
}

/// Paper loaded in `address` (or the only connected printer); `None` when unknown.
fn check_paper(
    ctx: &Ctx<'_>,
    address: Option<&str>,
) -> Result<Option<crate::services::PaperInfo>, CommandError> {
    let devices = ctx.services().devices.clone();
    match address {
        Some(a) => Ok(devices.status(a)?.paper),
        None => {
            let connected = devices.connected();
            match connected.as_slice() {
                [only] => Ok(devices.status(&only.address).ok().and_then(|s| s.paper)),
                _ => Ok(None),
            }
        }
    }
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
            "Find print problems. Rules: out_of_bounds, in_margin (with margin_mm), \
             outside_printable, label_too_wide, dpi_mismatch, density_out_of_range, \
             paper_mismatch (the printer's RFID paper: size when the barcode is linked in the paper library, else media), binding_missing, binding_empty, \
             render_failed, text_overflow, text_shrunk (< 5 pt is a warning), encode_failed, \
             module_too_narrow (< 2 dots), quiet_zone, code_unreadable (render, decode, \
             compare), bad_asset, font_fallback, low_contrast. Returns {ok, dpi, \
             issues:[{rule, severity, element_id?, message, hint}]}; ok is false when any issue \
             has severity error.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: CheckParams) -> Result<CheckOutput, CommandError> {
        let doc = ctx.doc_arc(p.doc_id.as_deref())?;
        let dpi = pick_dpi(ctx, &doc, p.dpi)?;
        let margin_mm = check_mm("margin_mm", p.margin_mm.unwrap_or(0.0), false)?;
        let paper = check_paper(ctx, p.address.as_deref())?;
        let opts = CheckOptions {
            dpi,
            margin_mm,
            paper,
        };
        let issues = check_document(ctx, &doc, &opts);
        let ok = !issues.iter().any(|i| i.severity == Severity::Error);
        Ok(CheckOutput { ok, dpi, issues })
    }
}
