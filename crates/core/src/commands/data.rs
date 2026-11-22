//! Data sources, column binding and serial numbers: `data.*` (ADR-0014).

use std::collections::BTreeMap;
use std::io::Read;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::binding_values;
use crate::command::{Command, CommandError, CommandMeta, Ctx, element_mut};
use crate::document::{DataSource, Document, ElementId, ElementKind, SerialSpec};

/// Most records a data source may hold.
pub const MAX_ROWS: usize = 10_000;
/// Most columns a data source may hold; extra columns are dropped with a warning.
pub const MAX_COLUMNS: usize = 256;
/// Largest data file read.
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// Rows returned in attach results.
const PREVIEW_ROWS: usize = 5;

/// Summary of the attached data source.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DataSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Column names; bind them as `{{column}}`.
    pub columns: Vec<String>,
    /// Number of records.
    pub row_count: usize,
    /// The first rows.
    pub preview: Vec<Vec<String>>,
    /// Sheet that was read (spreadsheets only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    /// All sheets in the workbook (spreadsheets only).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sheets: Vec<String>,
}

fn summary(d: &DataSource) -> DataSummary {
    DataSummary {
        origin: d.origin.clone(),
        columns: d.columns.clone(),
        row_count: d.rows.len(),
        preview: d.rows.iter().take(PREVIEW_ROWS).cloned().collect(),
        sheet: None,
        sheets: Vec::new(),
    }
}

/// Spreadsheet-style column name: 0 → A, 25 → Z, 26 → AA.
pub fn column_letter(mut i: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Turn raw cells into a [`DataSource`]. Blank rows are dropped; header names are trimmed,
/// made unique and stripped of braces. Returns warnings for the envelope.
pub fn build_source(
    raw: Vec<Vec<String>>,
    header: bool,
    origin: Option<String>,
) -> Result<(DataSource, Vec<String>), CommandError> {
    let mut warnings = Vec::new();
    let mut rows: Vec<Vec<String>> = raw
        .into_iter()
        .filter(|r| r.iter().any(|c| !c.trim().is_empty()))
        .collect();
    let head = if header && !rows.is_empty() {
        Some(rows.remove(0))
    } else {
        None
    };
    let width = rows
        .iter()
        .map(Vec::len)
        .chain(head.as_ref().map(Vec::len))
        .max()
        .unwrap_or(0);
    if width > MAX_COLUMNS {
        warnings.push(format!(
            "only the first {MAX_COLUMNS} of {width} columns were kept"
        ));
    }
    let width = width.min(MAX_COLUMNS);
    if rows.len() > MAX_ROWS {
        return Err(CommandError::bad_params(format!(
            "the data has {} records; at most {MAX_ROWS} are supported",
            rows.len()
        ))
        .with_hint("split the file, or attach only the rows you need"));
    }
    let mut columns: Vec<String> = Vec::with_capacity(width);
    for i in 0..width {
        let raw_name = head
            .as_ref()
            .and_then(|h| h.get(i))
            .map(|n| n.replace(['{', '}'], "").trim().to_owned())
            .unwrap_or_default();
        let base = if raw_name.is_empty() {
            column_letter(i)
        } else {
            raw_name
        };
        let mut name = base.clone();
        let mut k = 2;
        while columns.contains(&name) {
            name = format!("{base} {k}");
            k += 1;
        }
        columns.push(name);
    }
    for r in &mut rows {
        r.truncate(width);
        while r.last().is_some_and(String::is_empty) {
            r.pop();
        }
    }
    Ok((
        DataSource {
            origin,
            columns,
            rows,
        },
        warnings,
    ))
}

fn read_file(ctx: &Ctx<'_>, path: &str) -> Result<Vec<u8>, CommandError> {
    let p = ctx.check_path("path", path, false)?;
    let file = std::fs::File::open(&p).map_err(|e| {
        let err = if e.kind() == std::io::ErrorKind::NotFound {
            CommandError::not_found(format!("cannot read {path}: {e}"))
        } else {
            CommandError::bad_params(format!("cannot read {path}: {e}"))
        };
        err.with_param("path")
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| {
            CommandError::bad_params(format!("cannot read {path}: {e}")).with_param("path")
        })?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(CommandError::bad_params(format!(
            "{path} is larger than {} MiB",
            MAX_FILE_BYTES / 1024 / 1024
        ))
        .with_param("path"));
    }
    Ok(bytes)
}

fn file_name(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
}

/// Attach `source` to the document, warning about names that shadow variables or serials.
pub(crate) fn attach(
    ctx: &mut Ctx<'_>,
    doc_id: Option<&str>,
    source: DataSource,
    warnings: Vec<String>,
) -> Result<DataSummary, CommandError> {
    for w in warnings {
        ctx.warn(w);
    }
    let doc = ctx.doc_mut(doc_id)?;
    let shadowed: Vec<String> = source
        .columns
        .iter()
        .filter(|c| doc.variables.contains_key(*c) || doc.serials.contains_key(*c))
        .cloned()
        .collect();
    let replaced = doc.data.is_some();
    let out = summary(&source);
    doc.data = Some(source);
    doc.prune_assets();
    if !shadowed.is_empty() {
        ctx.warn(format!(
            "column(s) {} shadow a variable or serial of the same name",
            shadowed.join(", ")
        ));
    }
    if replaced {
        ctx.warn("the previous data source was replaced");
    }
    if out.row_count == 0 {
        ctx.warn("the data has no records");
    }
    Ok(out)
}

/// Pick a delimiter from the first line: the most frequent of `,` `;` tab `|`.
fn sniff_delimiter(text: &str) -> u8 {
    let line = text.lines().next().unwrap_or("");
    b",;\t|"
        .iter()
        .copied()
        .max_by_key(|d| line.bytes().filter(|b| b == d).count())
        .filter(|d| line.as_bytes().contains(d))
        .unwrap_or(b',')
}

/// Parse CSV text into raw rows.
pub fn parse_csv(text: &str, delimiter: Option<u8>) -> Result<Vec<Vec<String>>, CommandError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter.unwrap_or_else(|| sniff_delimiter(text)))
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| CommandError::bad_params(format!("CSV line {}: {e}", i + 1)))?;
        rows.push(rec.iter().map(str::to_owned).collect());
        if rows.len() > MAX_ROWS + 1 {
            break;
        }
    }
    Ok(rows)
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachCsvParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// CSV file to read (UTF-8). Pass exactly one of `path` or `text`.
    #[serde(default)]
    pub path: Option<String>,
    /// CSV content inline.
    #[serde(default)]
    pub text: Option<String>,
    /// One-character field delimiter; detected from the first line when absent.
    #[serde(default)]
    pub delimiter: Option<String>,
    /// First row holds the column names (default true). Otherwise columns are A, B, C….
    #[serde(default)]
    pub header: Option<bool>,
}

/// `data.attach_csv`
#[derive(Debug)]
pub struct AttachCsv;

impl Command for AttachCsv {
    const ID: &'static str = "data.attach_csv";
    type Params = AttachCsvParams;
    type Output = DataSummary;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Attach CSV",
            "Embed CSV data (path or text) as the document's data source, replacing any other. \
             Each row is one record printed as one label; bind columns as `{{column}}`. \
             Returns the columns, the record count and the first rows.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: AttachCsvParams) -> Result<DataSummary, CommandError> {
        let delimiter = match p.delimiter.as_deref() {
            None => None,
            Some("\\t") => Some(b'\t'),
            Some(d) if d.len() == 1 && d.is_ascii() => d.bytes().next(),
            Some(_) => {
                return Err(
                    CommandError::bad_params("delimiter must be one ASCII character")
                        .with_param("delimiter")
                        .with_accepted([",", ";", "\\t", "|"]),
                );
            }
        };
        let (text, origin) = match (&p.path, p.text) {
            (Some(path), None) => {
                let bytes = read_file(ctx, path)?;
                let text = String::from_utf8(bytes).map_err(|_| {
                    CommandError::bad_params(format!("{path} is not UTF-8 text"))
                        .with_param("path")
                        .with_hint("save the CSV as UTF-8")
                })?;
                (text, file_name(path))
            }
            (None, Some(text)) => (text, None),
            _ => {
                return Err(CommandError::bad_params(
                    "pass exactly one of `path` or `text`",
                ));
            }
        };
        let raw = parse_csv(&text, delimiter)?;
        let (source, warnings) = build_source(raw, p.header.unwrap_or(true), origin)?;
        attach(ctx, p.doc_id.as_deref(), source, warnings)
    }
}

/// Spreadsheet cell as display text. Whole floats print without a fraction; dates as ISO.
fn cell_text(c: &calamine::Data) -> String {
    use calamine::Data;
    match c {
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::DateTime(d) if d.is_datetime() => {
            let (y, mo, da, h, mi, s, _) = d.to_ymd_hms_milli();
            if (h, mi, s) == (0, 0, 0) {
                format!("{y:04}-{mo:02}-{da:02}")
            } else if s == 0 {
                format!("{y:04}-{mo:02}-{da:02} {h:02}:{mi:02}")
            } else {
                format!("{y:04}-{mo:02}-{da:02} {h:02}:{mi:02}:{s:02}")
            }
        }
        other => other.to_string(),
    }
}

/// Raw rows of one sheet, its name, and every sheet name.
pub type Workbook = (Vec<Vec<String>>, String, Vec<String>);

/// Read one sheet of a workbook (xlsx, xlsm, xls, ods) into raw rows.
pub fn read_workbook(bytes: Vec<u8>, sheet: Option<&str>) -> Result<Workbook, CommandError> {
    use calamine::Reader;
    let mut wb =
        calamine::open_workbook_auto_from_rs(std::io::Cursor::new(bytes)).map_err(|e| {
            CommandError::bad_params(format!("not a readable spreadsheet: {e}"))
                .with_param("path")
                .with_hint("use .xlsx, .xls or .ods, or export the sheet as CSV")
        })?;
    let sheets = wb.sheet_names();
    let name = match sheet {
        Some(s) if sheets.iter().any(|n| n == s) => s.to_owned(),
        Some(s) => {
            return Err(CommandError::not_found(format!("no sheet named `{s}`"))
                .with_param("sheet")
                .with_accepted(sheets.clone()));
        }
        None => sheets
            .first()
            .cloned()
            .ok_or_else(|| CommandError::bad_params("the workbook has no sheets"))?,
    };
    let range = wb.worksheet_range(&name).map_err(|e| {
        CommandError::bad_params(format!("cannot read sheet `{name}`: {e}")).with_param("sheet")
    })?;
    let rows = range
        .rows()
        .take(MAX_ROWS + 2)
        .map(|r| r.iter().map(cell_text).collect())
        .collect();
    Ok((rows, name, sheets))
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachXlsxParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Spreadsheet file: .xlsx, .xlsm, .xls or .ods.
    pub path: String,
    /// Sheet name; defaults to the first sheet.
    #[serde(default)]
    pub sheet: Option<String>,
    /// First row holds the column names (default true). Otherwise columns are A, B, C….
    #[serde(default)]
    pub header: Option<bool>,
}

/// `data.attach_xlsx`
#[derive(Debug)]
pub struct AttachXlsx;

impl Command for AttachXlsx {
    const ID: &'static str = "data.attach_xlsx";
    type Params = AttachXlsxParams;
    type Output = DataSummary;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Attach spreadsheet",
            "Embed one sheet of a spreadsheet (.xlsx, .xls, .ods) as the document's data \
             source, replacing any other. Cells become text (dates as YYYY-MM-DD). Each row \
             is one record; bind columns as `{{column}}`.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: AttachXlsxParams) -> Result<DataSummary, CommandError> {
        let bytes = read_file(ctx, &p.path)?;
        let (raw, sheet, sheets) = read_workbook(bytes, p.sheet.as_deref())?;
        let (source, warnings) = build_source(raw, p.header.unwrap_or(true), file_name(&p.path))?;
        let mut out = attach(ctx, p.doc_id.as_deref(), source, warnings)?;
        out.sheet = Some(sheet);
        out.sheets = sheets;
        Ok(out)
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocParams {
    #[serde(default)]
    pub doc_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DetachOutput {
    /// Whether a data source was removed.
    pub removed: bool,
}

/// `data.detach`
#[derive(Debug)]
pub struct Detach;

impl Command for Detach {
    const ID: &'static str = "data.detach";
    type Params = DocParams;
    type Output = DetachOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Remove data",
            "Remove the document's data source. Bindings stay in the elements and render as \
             literal `{{column}}` until data is attached again.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: DocParams) -> Result<DetachOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let removed = doc.data.take().is_some();
        doc.prune_assets();
        Ok(DetachOutput { removed })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// First record to return, 1-based (default 1).
    #[serde(default)]
    pub from: Option<usize>,
    /// Records to return, 0–200 (default 20).
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetOutput {
    /// False when no data source is attached; the other fields are then empty.
    pub attached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub columns: Vec<String>,
    pub row_count: usize,
    /// 1-based number of the first returned row.
    pub from: usize,
    pub rows: Vec<Vec<String>>,
    /// Serial counters by name.
    pub serials: BTreeMap<String, SerialSpec>,
}

/// `data.get`
#[derive(Debug)]
pub struct Get;

impl Command for Get {
    const ID: &'static str = "data.get";
    type Params = GetParams;
    type Output = GetOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Get data",
            "Columns, record count and a page of rows of the data source (from 1-based, \
             limit ≤ 200), plus the serial counters.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: GetParams) -> Result<GetOutput, CommandError> {
        let limit = p.limit.unwrap_or(20);
        if limit > 200 {
            return Err(CommandError::bad_params("limit must be 0–200").with_param("limit"));
        }
        let from = p.from.unwrap_or(1).max(1);
        let doc = ctx.doc(p.doc_id.as_deref())?;
        let serials = doc.serials.clone();
        Ok(match &doc.data {
            None => GetOutput {
                attached: false,
                origin: None,
                columns: Vec::new(),
                row_count: 0,
                from,
                rows: Vec::new(),
                serials,
            },
            Some(d) => GetOutput {
                attached: true,
                origin: d.origin.clone(),
                columns: d.columns.clone(),
                row_count: d.rows.len(),
                from,
                rows: d.rows.iter().skip(from - 1).take(limit).cloned().collect(),
                serials,
            },
        })
    }
}

/// Every name a `{{binding}}` can resolve to: columns, serials and variables.
pub fn binding_names(doc: &Document) -> Vec<String> {
    let mut names: Vec<String> = doc
        .data
        .iter()
        .flat_map(|d| d.columns.iter().cloned())
        .collect();
    for n in doc.serials.keys().chain(doc.variables.keys()) {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }
    names
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BindParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Text, barcode or 2D-code element.
    pub id: ElementId,
    /// Column, serial or variable name.
    pub column: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BindOutput {
    pub id: ElementId,
    /// Property that now holds the binding (`text` or `data`).
    pub property: String,
    pub value: String,
}

/// `data.bind`
#[derive(Debug)]
pub struct Bind;

impl Command for Bind {
    const ID: &'static str = "data.bind";
    type Params = BindParams;
    type Output = BindOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Bind to column",
            "Set a text element's `text`, or a barcode or 2D code's `data`, to `{{column}}`. \
             The name must be a data column, serial or variable. For mixed content, use \
             element.set_props with `{{name}}` inside the string.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: BindParams) -> Result<BindOutput, CommandError> {
        let column = p.column.trim().to_owned();
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let names = binding_names(doc);
        if !names.contains(&column) {
            return Err(CommandError::not_found(format!(
                "no column, serial or variable `{column}`"
            ))
            .with_param("column")
            .with_accepted(names.into_iter().take(50))
            .with_hint("attach data with data.attach_csv, or add data.set_serial"));
        }
        let value = format!("{{{{{column}}}}}");
        let e = element_mut(doc, p.id)?;
        if e.locked {
            return Err(
                CommandError::bad_params(format!("element {} is locked", p.id.0)).with_param("id"),
            );
        }
        let property = match &mut e.kind {
            ElementKind::Text(t) => {
                t.text.clone_from(&value);
                "text"
            }
            ElementKind::Barcode(b) => {
                b.data.clone_from(&value);
                "data"
            }
            ElementKind::Code2d(c) => {
                c.data.clone_from(&value);
                "data"
            }
            other => {
                return Err(CommandError::bad_params(format!(
                    "a {} element cannot be bound; use a text, barcode or code2d element",
                    other.type_name()
                ))
                .with_param("id"));
            }
        };
        Ok(BindOutput {
            id: p.id,
            property: property.to_owned(),
            value,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewRecordParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Record number, 1-based (default 1).
    #[serde(default)]
    pub record: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PreviewRecordOutput {
    pub record: usize,
    pub row_count: usize,
    /// Every binding name with its value for this record (columns, serials, variables).
    pub values: BTreeMap<String, String>,
}

/// `data.preview_record`
#[derive(Debug)]
pub struct PreviewRecord;

impl Command for PreviewRecord {
    const ID: &'static str = "data.preview_record";
    type Params = PreviewRecordParams;
    type Output = PreviewRecordOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Preview record",
            "Binding values for one record (1-based) as printing all records would produce \
             them: columns, serials at that position, and variables. Use render.preview \
             {record} for the image.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: PreviewRecordParams) -> Result<PreviewRecordOutput, CommandError> {
        let doc = ctx.doc(p.doc_id.as_deref())?;
        let record = p.record.unwrap_or(1);
        let row = record_index(doc, record)?;
        Ok(PreviewRecordOutput {
            record,
            row_count: doc.data.as_ref().map_or(0, |d| d.rows.len()),
            values: binding_values(doc, row, record.saturating_sub(1) as u64),
        })
    }
}

/// Most record numbers listed in a `data.find_record` result.
const MAX_MATCHES: usize = 50;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FindRecordParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Value to look for, such as a scanned barcode. Compared with each cell exactly,
    /// after trimming surrounding whitespace on both sides.
    pub value: String,
    /// Column to search; every column when absent.
    #[serde(default)]
    pub column: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FindRecordOutput {
    /// First matching record, 1-based; pass it to render.preview or print.submit `records`.
    pub record: usize,
    /// Column the first match was found in.
    pub column: String,
    /// Number of matching records.
    pub match_count: usize,
    /// Matching records, 1-based, in order (at most 50).
    pub matches: Vec<usize>,
}

/// `data.find_record`
#[derive(Debug)]
pub struct FindRecord;

impl Command for FindRecord {
    const ID: &'static str = "data.find_record";
    type Params = FindRecordParams;
    type Output = FindRecordOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "Find record",
            "Find the record whose cell equals `value` (trimmed, exact), in `column` or in \
             any column. Returns the first matching record (1-based) and every match, e.g. \
             to print the record of a scanned barcode with print.submit {records}.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: FindRecordParams) -> Result<FindRecordOutput, CommandError> {
        let doc = ctx.doc(p.doc_id.as_deref())?;
        let Some(d) = &doc.data else {
            return Err(CommandError::bad_params("the document has no data source")
                .with_hint("attach data with data.attach_csv or data.attach_xlsx"));
        };
        let value = p.value.trim();
        if value.is_empty() {
            return Err(CommandError::bad_params("value must not be empty").with_param("value"));
        }
        let cols: Vec<usize> = match p.column.as_deref().map(str::trim) {
            None | Some("") => (0..d.columns.len()).collect(),
            Some(c) => match d.columns.iter().position(|n| n == c) {
                Some(i) => vec![i],
                None => {
                    return Err(CommandError::not_found(format!("no column `{c}`"))
                        .with_param("column")
                        .with_accepted(d.columns.iter().take(50).cloned()));
                }
            },
        };
        let mut first: Option<(usize, usize)> = None;
        let mut matches = Vec::new();
        let mut match_count = 0;
        for (i, row) in d.rows.iter().enumerate() {
            let Some(&c) = cols
                .iter()
                .find(|&&c| row.get(c).is_some_and(|v| v.trim() == value))
            else {
                continue;
            };
            first.get_or_insert((i, c));
            match_count += 1;
            if matches.len() < MAX_MATCHES {
                matches.push(i + 1);
            }
        }
        let Some((row, col)) = first else {
            let place = match &p.column {
                Some(c) if !c.trim().is_empty() => format!("column `{}`", c.trim()),
                _ => "any column".to_owned(),
            };
            return Err(
                CommandError::not_found(format!("no record has `{value}` in {place}"))
                    .with_param("value")
                    .with_hint("values are compared exactly; check the column with data.get"),
            );
        };
        Ok(FindRecordOutput {
            record: row + 1,
            column: d.columns.get(col).cloned().unwrap_or_default(),
            match_count,
            matches,
        })
    }
}

/// 0-based row for a 1-based `record` param, or `None` without data (only record 1 is valid).
pub(crate) fn record_index(doc: &Document, record: usize) -> Result<Option<usize>, CommandError> {
    match &doc.data {
        Some(d) if (1..=d.rows.len()).contains(&record) => Ok(Some(record - 1)),
        Some(d) => Err(CommandError::bad_params(format!(
            "record {record} does not exist; the data has {} record(s)",
            d.rows.len()
        ))
        .with_param("record")
        .with_hint("records are 1-based")),
        None if record == 1 => Ok(None),
        None => Err(CommandError::bad_params("the document has no data source")
            .with_param("record")
            .with_hint("attach data with data.attach_csv or data.attach_xlsx")),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSerialParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Binding name, used as `{{name}}` (default `serial`).
    #[serde(default)]
    pub name: Option<String>,
    /// First value (default 1).
    #[serde(default)]
    pub start: Option<i64>,
    /// Increment per label, may be negative (default 1).
    #[serde(default)]
    pub step: Option<i64>,
    /// Zero-pad to this many digits, 0–18 (default 0: no padding).
    #[serde(default)]
    pub digits: Option<u8>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SetSerialOutput {
    pub name: String,
    pub serial: SerialSpec,
    /// Values of the first three labels.
    pub sample: Vec<String>,
}

/// `data.set_serial`
#[derive(Debug)]
pub struct SetSerial;

impl Command for SetSerial {
    const ID: &'static str = "data.set_serial";
    type Params = SetSerialParams;
    type Output = SetSerialOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Serial number",
            "Create or replace a serial counter `{{name}}` (default name `serial`): \
             prefix + zero-padded (start + step × n) + suffix, where n is the label's 0-based \
             position in the print job. Bind it with data.bind or `{{name}}` in any text.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: SetSerialParams) -> Result<SetSerialOutput, CommandError> {
        let name = p.name.as_deref().unwrap_or("serial").trim().to_owned();
        if name.is_empty() || name.contains(['{', '}']) {
            return Err(CommandError::bad_params(
                "serial name must be non-empty and contain no braces",
            )
            .with_param("name"));
        }
        let digits = p.digits.unwrap_or(0);
        if digits > 18 {
            return Err(CommandError::bad_params("digits must be 0–18").with_param("digits"));
        }
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let old = doc.serials.get(&name).cloned().unwrap_or_default();
        let serial = SerialSpec {
            start: p.start.unwrap_or(old.start),
            step: p.step.unwrap_or(old.step),
            digits: p.digits.unwrap_or(old.digits),
            prefix: p.prefix.unwrap_or(old.prefix),
            suffix: p.suffix.unwrap_or(old.suffix),
        };
        doc.serials.insert(name.clone(), serial.clone());
        let shadowed = doc.data.as_ref().is_some_and(|d| d.columns.contains(&name));
        if shadowed {
            ctx.warn(format!(
                "data column `{name}` has the same name and takes precedence"
            ));
        }
        Ok(SetSerialOutput {
            sample: (0..3).map(|n| serial.value(n)).collect(),
            name,
            serial,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveSerialParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub name: String,
}

/// `data.remove_serial`
#[derive(Debug)]
pub struct RemoveSerial;

impl Command for RemoveSerial {
    const ID: &'static str = "data.remove_serial";
    type Params = RemoveSerialParams;
    type Output = DetachOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit("Remove serial", "Delete a serial counter by name.")
    }

    fn run(ctx: &mut Ctx<'_>, p: RemoveSerialParams) -> Result<DetachOutput, CommandError> {
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        if doc.serials.remove(p.name.trim()).is_none() {
            let names: Vec<String> = doc.serials.keys().cloned().collect();
            return Err(
                CommandError::not_found(format!("no serial named `{}`", p.name))
                    .with_param("name")
                    .with_accepted(names),
            );
        }
        Ok(DetachOutput { removed: true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
        assert_eq!(column_letter(701), "ZZ");
        assert_eq!(column_letter(702), "AAA");
    }

    #[test]
    fn csv_sniff_and_headers() {
        let raw = parse_csv("\u{feff}name;{sku};name;\nAda;1;x\n;;;\nBob;2\n", None).unwrap();
        let (d, w) = build_source(raw, true, None).unwrap();
        assert!(w.is_empty());
        assert_eq!(d.columns, ["name", "sku", "name 2", "D"]);
        assert_eq!(d.rows, [vec!["Ada", "1", "x"], vec!["Bob", "2"]]);
        let raw = parse_csv("a\tb\n1\t2\n", None).unwrap();
        let (d, _) = build_source(raw, false, None).unwrap();
        assert_eq!(d.columns, ["A", "B"]);
        assert_eq!(d.rows.len(), 2);
    }
}
