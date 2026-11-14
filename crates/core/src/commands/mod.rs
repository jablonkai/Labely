//! Built-in commands. Registered in [`crate::registry`].

pub mod device;
pub mod doc;
pub mod element;
pub mod history;
pub mod meta;
pub mod render;
pub mod selection;
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

/// Render options with `{{name}}` values from `doc.variables`.
pub(crate) fn render_options(doc: &Document, dpi: u32, page: usize) -> RenderOptions {
    RenderOptions {
        dpi,
        page,
        values: doc.variables.clone(),
    }
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
}
