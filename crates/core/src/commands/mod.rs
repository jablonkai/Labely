//! Built-in commands. Registered in [`crate::registry`].

pub mod arrange;
pub mod data;
pub mod decor;
pub mod device;
pub mod doc;
pub mod element;
pub mod export;
pub mod history;
pub mod images;
pub mod jobs;
pub mod meta;
pub mod paper;
pub mod render;
pub mod selection;
pub mod table;
pub mod template;
pub mod variable;

use std::collections::BTreeMap;

use crate::command::{CommandError, Ctx};
use crate::document::Document;
use crate::services::{ModelInfo, RenderOptions};

/// Resolution used when no model is known.
pub const DEFAULT_DPI: u32 = 203;

/// Registry entry for the document's `label.printer_model`.
pub(crate) fn doc_model(ctx: &Ctx<'_>, doc: &Document) -> Option<ModelInfo> {
    let id = doc.label.printer_model.as_deref()?;
    ctx.services()
        .devices
        .models()
        .into_iter()
        .find(|m| m.id == id)
}

/// Binding values for the label at job position `n` (0-based) showing data row `row`
/// (0-based). Data columns win over serials, serials over variables.
pub fn binding_values(doc: &Document, row: Option<usize>, n: u64) -> BTreeMap<String, String> {
    let mut values = doc.variables.clone();
    for (name, spec) in &doc.serials {
        values.insert(name.clone(), spec.value(n));
    }
    if let (Some(d), Some(r)) = (&doc.data, row) {
        values.extend(d.record(r));
    }
    let names = bound_names(doc);
    if names.iter().any(|n| crate::datetime::is_date_binding(n)) {
        let now = crate::datetime::clock(doc);
        for n in names {
            if crate::datetime::is_date_binding(&n)
                && let Ok(v) = crate::datetime::eval(&n, now)
            {
                values.insert(n, v);
            }
        }
    }
    values
}

/// Every `{{name}}` used in the bindable strings (text, barcode and 2D-code data).
pub fn bound_names(doc: &Document) -> Vec<String> {
    use crate::document::ElementKind;
    let mut out: Vec<String> = Vec::new();
    for e in doc.elements() {
        let s = match &e.kind {
            ElementKind::Text(t) => &t.text,
            ElementKind::Barcode(b) => &b.data,
            ElementKind::Code2d(c) => &c.data,
            ElementKind::Table(t) => {
                for c in t.cells.iter().flatten() {
                    collect_names(&c.text, &mut out);
                }
                continue;
            }
            _ => continue,
        };
        collect_names(s, &mut out);
    }
    out
}

fn collect_names(s: &str, out: &mut Vec<String>) {
    {
        let mut rest = s;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else { break };
            let name = after[..end].trim();
            if !name.is_empty() && !out.iter().any(|n| n == name) {
                out.push(name.to_owned());
            }
            rest = &after[end + 2..];
        }
    }
}

/// Render options for the first label: record 1 when data is attached, serials at start.
pub(crate) fn render_options(doc: &Document, dpi: u32, page: usize) -> RenderOptions {
    render_options_for(doc, dpi, page, doc.data.as_ref().map(|_| 0), 0)
}

/// Render options for data row `row` at job position `n`.
pub(crate) fn render_options_for(
    doc: &Document,
    dpi: u32,
    page: usize,
    row: Option<usize>,
    n: u64,
) -> RenderOptions {
    RenderOptions {
        dpi,
        page,
        values: binding_values(doc, row, n),
    }
}

/// Parse a 1-based record selection such as `1,3,5-7` or `all` into 0-based row indices,
/// in the given order. `count` is the number of records.
pub fn parse_records(spec: &str, count: usize) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("all") {
        return Ok((0..count).collect());
    }
    let num = |s: &str| -> Result<usize, String> {
        let n: usize = s
            .trim()
            .parse()
            .map_err(|_| format!("`{}` is not a record number", s.trim()))?;
        if n == 0 || n > count {
            return Err(format!("record {n} is out of range 1–{count}"));
        }
        Ok(n - 1)
    };
    let mut out = Vec::new();
    for part in spec.split(',') {
        match part.split_once(['-', '–']) {
            Some((a, b)) => {
                let a = num(a)?;
                let b = if b.trim().is_empty() {
                    count - 1
                } else {
                    num(b)?
                };
                if a <= b {
                    out.extend(a..=b);
                } else {
                    out.extend((b..=a).rev());
                }
            }
            None => out.push(num(part)?),
        }
    }
    Ok(out)
}

/// Substitute `{{name}}` from `values`; unknown names are left as is.
pub fn interpolate(text: &str, values: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let name = after[..end].trim();
                match values.get(name) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + 2 + end + 2]),
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn check_dpi(dpi: u32) -> Result<u32, CommandError> {
    if (72..=1200).contains(&dpi) {
        Ok(dpi)
    } else {
        Err(
            CommandError::bad_params(format!("dpi {dpi} is out of range 72–1200"))
                .with_param("dpi"),
        )
    }
}

/// A finite number, optionally strictly positive.
pub(crate) fn check_mm(name: &str, v: f64, positive: bool) -> Result<f64, CommandError> {
    if !v.is_finite() {
        return Err(
            CommandError::bad_params(format!("`{name}` must be a finite number")).with_param(name),
        );
    }
    if positive && v <= 0.0 {
        return Err(
            CommandError::bad_params(format!("`{name}` must be > 0 mm, got {v}")).with_param(name),
        );
    }
    if v.abs() > 10_000.0 {
        return Err(
            CommandError::bad_params(format!("`{name}` = {v} mm is implausibly large"))
                .with_param(name),
        );
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation() {
        let mut v = BTreeMap::new();
        v.insert("name".to_owned(), "Ada".to_owned());
        assert_eq!(interpolate("Hi {{name}}, {{ name }}!", &v), "Hi Ada, Ada!");
        assert_eq!(interpolate("{{missing}} {{name", &v), "{{missing}} {{name");
        assert_eq!(interpolate("plain", &v), "plain");
    }

    #[test]
    fn record_ranges() {
        assert_eq!(parse_records("all", 3).unwrap(), [0, 1, 2]);
        assert_eq!(parse_records("", 2).unwrap(), [0, 1]);
        assert_eq!(parse_records("1, 3,5-7", 9).unwrap(), [0, 2, 4, 5, 6]);
        assert_eq!(parse_records("3-1", 3).unwrap(), [2, 1, 0]);
        assert_eq!(parse_records("8-", 9).unwrap(), [7, 8]);
        assert!(parse_records("0", 3).is_err());
        assert!(parse_records("4", 3).unwrap_err().contains("1–3"));
        assert!(parse_records("x", 3).is_err());
    }
}
