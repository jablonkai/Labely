//! `export.png`, `export.pdf`: write the print raster to a file (ADR-0018).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::data::record_index;
use super::device::MAX_JOB_LABELS;
use super::render::{pick_dpi, render_error};
use super::render_options_for;
use crate::command::{Command, CommandError, CommandMeta, Ctx};

fn write_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), CommandError> {
    std::fs::write(path, bytes).map_err(|e| {
        CommandError::internal(format!("cannot write {}: {e}", path.display()))
            .with_param("path")
            .with_hint("check the directory exists and is writable")
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PngParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Target `.png` file.
    pub path: String,
    /// Resolution in dpi; defaults to the label's printer model dpi, else 203.
    #[serde(default)]
    pub dpi: Option<u32>,
    /// 0-based page index (default 0).
    #[serde(default)]
    pub page: Option<usize>,
    /// Data record, 1-based (default 1).
    #[serde(default)]
    pub record: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PngOutput {
    pub path: String,
    pub bytes: usize,
    pub width_dots: u32,
    pub height_dots: u32,
    pub dpi: u32,
}

/// `export.png`
#[derive(Debug)]
pub struct ExportPng;

impl Command for ExportPng {
    const ID: &'static str = "export.png";
    type Params = PngParams;
    type Output = PngOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Export PNG",
            "Write one label as a 1-bit PNG: the exact print raster at `dpi` (default: the \
             model's, else 203), for `page` and data `record` (1-based, default 1).",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: PngParams) -> Result<PngOutput, CommandError> {
        let doc = ctx.doc_arc(p.doc_id.as_deref())?;
        let path = ctx.check_path("path", &p.path, true)?;
        let dpi = pick_dpi(ctx, &doc, p.dpi)?;
        let record = p.record.unwrap_or(1);
        let row = record_index(&doc, record)?;
        let opts = render_options_for(&doc, dpi, p.page.unwrap_or(0), row, record as u64 - 1);
        let renderer = ctx.services().renderer.clone();
        let out = renderer.render(&doc, &opts).map_err(render_error)?;
        let png = renderer.to_png(&out.bitmap).map_err(render_error)?;
        write_file(&path, &png)?;
        Ok(PngOutput {
            path: path.display().to_string(),
            bytes: png.len(),
            width_dots: out.bitmap.width(),
            height_dots: out.bitmap.height(),
            dpi,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PdfParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Target `.pdf` file.
    pub path: String,
    /// Raster resolution in dpi; defaults to the label's printer model dpi, else 203.
    #[serde(default)]
    pub dpi: Option<u32>,
    /// Data records, 1-based: `all` (default), `3`, `1,4,6-9`. Needs a data source.
    #[serde(default)]
    pub records: Option<String>,
    /// Labels without a data source, 1–5000 (default 1); serials advance per label.
    #[serde(default)]
    pub count: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PdfOutput {
    pub path: String,
    pub bytes: usize,
    /// PDF pages written: labels × document pages.
    pub pages: usize,
    pub dpi: u32,
}

/// `export.pdf`
#[derive(Debug)]
pub struct ExportPdf;

impl Command for ExportPdf {
    const ID: &'static str = "export.pdf";
    type Params = PdfParams;
    type Output = PdfOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Export PDF",
            "Write labels to a PDF, one page per label sized to the label in mm, each the exact \
             1-bit print raster at `dpi`. With data: one label per record (`records` like \
             `1,3,5-7`, default all); without: `count` labels (default 1) with serials \
             advancing.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: PdfParams) -> Result<PdfOutput, CommandError> {
        let doc = ctx.doc_arc(p.doc_id.as_deref())?;
        let path = ctx.check_path("path", &p.path, true)?;
        let dpi = pick_dpi(ctx, &doc, p.dpi)?;
        let labels: Vec<Option<usize>> = match &doc.data {
            Some(data) => {
                if p.count.is_some() {
                    return Err(CommandError::bad_params(
                        "`count` is for documents without data; use `records`",
                    )
                    .with_param("count"));
                }
                super::parse_records(p.records.as_deref().unwrap_or("all"), data.rows.len())
                    .map_err(|e| CommandError::bad_params(e).with_param("records"))?
                    .into_iter()
                    .map(Some)
                    .collect()
            }
            None => {
                if p.records.is_some() {
                    return Err(CommandError::bad_params("`records` needs a data source")
                        .with_param("records"));
                }
                let n = p.count.unwrap_or(1);
                if !(1..=MAX_JOB_LABELS as u32).contains(&n) {
                    return Err(CommandError::bad_params(format!(
                        "count must be 1–{MAX_JOB_LABELS}"
                    ))
                    .with_param("count"));
                }
                vec![None; n as usize]
            }
        };
        if labels.is_empty() {
            return Err(CommandError::bad_params("no records to export").with_param("records"));
        }
        let total = labels.len() * doc.pages.len();
        if total > MAX_JOB_LABELS {
            return Err(CommandError::bad_params(format!(
                "{total} pages; at most {MAX_JOB_LABELS} are exported at once"
            ))
            .with_param("records"));
        }
        let renderer = ctx.services().renderer.clone();
        let mut pages = Vec::with_capacity(total);
        for (n, row) in labels.into_iter().enumerate() {
            for page in 0..doc.pages.len() {
                let opts = render_options_for(&doc, dpi, page, row, n as u64);
                pages.push(renderer.render(&doc, &opts).map_err(render_error)?.bitmap);
            }
        }
        let pdf = crate::pdf::write_pdf(&pages, doc.label.width_mm, doc.label.height_mm, &doc.name);
        write_file(&path, &pdf)?;
        Ok(PdfOutput {
            path: path.display().to_string(),
            bytes: pdf.len(),
            pages: pages.len(),
            dpi,
        })
    }
}
