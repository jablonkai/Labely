//! `.labely` load/save: pretty JSON with a `format_version` gate. Opening also reads
//! NiimBlue exports ([`niimblue`]) and ZPL labels ([`zpl`]).

pub mod niimblue;
pub mod zpl;

use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;

use crate::document::{Document, FORMAT_VERSION};

/// File extension of the native format.
pub const EXTENSION: &str = "labely";

#[derive(Debug, Error)]
pub enum FileError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("not a valid .labely document: {0}")]
    Parse(String),
    #[error("not a valid NiimBlue label: {0}")]
    NiimBlue(String),
    #[error("not a valid ZPL label: {0}")]
    Zpl(String),
    #[error(
        "document format_version {found} is newer than this Labely supports ({supported}); upgrade Labely to open it"
    )]
    TooNew { found: u64, supported: u32 },
}

/// Parse a `.labely` JSON string.
pub fn from_json(text: &str) -> Result<Document, FileError> {
    let v: Value = serde_json::from_str(text).map_err(|e| FileError::Parse(e.to_string()))?;
    from_value(v)
}

fn from_value(v: Value) -> Result<Document, FileError> {
    let found = v
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| FileError::Parse("missing or invalid `format_version`".to_owned()))?;
    if found > u64::from(FORMAT_VERSION) {
        return Err(FileError::TooNew {
            found,
            supported: FORMAT_VERSION,
        });
    }
    let mut v = v;
    // v1 -> v2 only added optional fields (`data`, `serials`).
    if found < 2
        && let Some(m) = v.as_object_mut()
    {
        m.insert("format_version".to_owned(), Value::from(FORMAT_VERSION));
    }
    serde_path_to_error::deserialize(v).map_err(|e| {
        let path = e.path().to_string();
        FileError::Parse(format!("at `{path}`: {}", e.into_inner()))
    })
}

/// Serialise to pretty JSON with a trailing newline.
pub fn to_json(doc: &Document) -> Result<String, FileError> {
    let mut s = serde_json::to_string_pretty(doc).map_err(|e| FileError::Parse(e.to_string()))?;
    s.push('\n');
    Ok(s)
}

pub fn load(path: &Path) -> Result<Document, FileError> {
    let text = std::fs::read_to_string(path).map_err(|source| FileError::Read {
        path: path.to_owned(),
        source,
    })?;
    from_json(&text)
}

/// A document imported from another format and what did not come across.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub doc: Document,
    pub warnings: Vec<String>,
}

/// A file opened by [`open`].
#[derive(Debug, Clone, PartialEq)]
pub enum Opened {
    Labely(Document),
    /// Imported from NiimBlue; it has no `.labely` file yet.
    NiimBlue(Import),
    /// Imported from ZPL; it has no `.labely` file yet.
    Zpl(Import),
}

/// Open a `.labely` document, or import a NiimBlue export or a ZPL label, told apart by
/// content. `dpi` converts imported dots to mm (default [`niimblue::DEFAULT_DPI`]).
pub fn open(path: &Path, dpi: Option<f64>) -> Result<Opened, FileError> {
    let bytes = std::fs::read(path).map_err(|source| FileError::Read {
        path: path.to_owned(),
        source,
    })?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported");
    let dpi = dpi.unwrap_or(niimblue::DEFAULT_DPI);
    if zpl::detect(&bytes) {
        return zpl::import(&bytes, stem, dpi)
            .map(Opened::Zpl)
            .map_err(FileError::Zpl);
    }
    let v: Value = serde_json::from_slice(&bytes).map_err(|e| FileError::Parse(e.to_string()))?;
    if !niimblue::detect(&v) {
        return from_value(v).map(Opened::Labely);
    }
    niimblue::import(&v, stem, dpi)
        .map(Opened::NiimBlue)
        .map_err(FileError::NiimBlue)
}

/// Save; returns the number of bytes written.
pub fn save(doc: &Document, path: &Path) -> Result<usize, FileError> {
    let text = to_json(doc)?;
    std::fs::write(path, &text).map_err(|source| FileError::Write {
        path: path.to_owned(),
        source,
    })?;
    Ok(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::LabelSpec;

    #[test]
    fn roundtrip_and_version_gate() {
        let d = Document::new("x", LabelSpec::default());
        let s = to_json(&d).unwrap();
        assert!(s.contains("\n  \"format_version\": 2"));
        assert_eq!(from_json(&s).unwrap(), d);
        let v1 = s.replace("\"format_version\": 2", "\"format_version\": 1");
        assert_eq!(from_json(&v1).unwrap(), d, "v1 migrates to v2");
        let newer = s.replace("\"format_version\": 2", "\"format_version\": 99");
        let e = from_json(&newer).unwrap_err();
        assert!(matches!(e, FileError::TooNew { found: 99, .. }), "{e}");
        assert!(e.to_string().contains("upgrade Labely"));
        assert!(matches!(from_json("{}"), Err(FileError::Parse(_))));
    }
}
