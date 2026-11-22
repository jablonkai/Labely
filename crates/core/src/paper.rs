//! Label paper library (ADR-0020): a bundled table of common label sizes, the user's own
//! papers, and links from RFID barcodes to papers.
//!
//! The bundled table is Labely's own list of common sizes, not a vendor catalogue. Users
//! add papers and link the barcode the printer reads from a roll's RFID tag to a paper, so
//! the next time that roll is loaded it is recognised offline.
//!
//! User data lives in one JSON file set by the frontend ([`crate::Session::set_paper_file`]).

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{CommandError, ErrorCode};
use crate::document::{LabelSpec, MediaType};

/// Prefix of bundled paper ids.
pub const BUILTIN_PREFIX: &str = "std:";
/// Prefix of user paper ids.
pub const CUSTOM_PREFIX: &str = "my:";
/// Size difference, in mm, still treated as the same paper.
pub const SIZE_TOLERANCE_MM: f64 = 0.5;

/// One kind of label stock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Paper {
    /// `std:<…>` (bundled) or `my:<…>` (user).
    pub id: String,
    pub name: String,
    /// Width across the printhead, mm.
    pub width_mm: f64,
    /// Length along the feed, mm (the default length for continuous rolls).
    pub height_mm: f64,
    #[serde(default)]
    pub corner_radius_mm: f64,
    #[serde(default)]
    pub media: MediaType,
}

impl Paper {
    pub fn is_custom(&self) -> bool {
        self.id.starts_with(CUSTOM_PREFIX)
    }

    /// Same size (within [`SIZE_TOLERANCE_MM`]) and media as `label`.
    pub fn matches(&self, label: &LabelSpec) -> bool {
        self.media == label.media
            && (self.width_mm - label.width_mm).abs() <= SIZE_TOLERANCE_MM
            && (self.height_mm - label.height_mm).abs() <= SIZE_TOLERANCE_MM
    }

    /// A design of `width_mm` × `height_mm` prints on this paper: the width matches (within
    /// [`SIZE_TOLERANCE_MM`]) and so does the height, unless the paper is continuous, which
    /// takes any length. Die-cut media (gap, black mark, clear) of one size are alike.
    pub fn fits(&self, width_mm: f64, height_mm: f64) -> bool {
        (self.width_mm - width_mm).abs() <= SIZE_TOLERANCE_MM
            && (self.media == MediaType::Continuous
                || (self.height_mm - height_mm).abs() <= SIZE_TOLERANCE_MM)
    }
}

#[derive(Default, Deserialize)]
struct BuiltinFile {
    papers: Vec<Paper>,
    /// RFID barcode → bundled paper id, read from rolls we own (ADR-0041).
    #[serde(default)]
    rfid: BTreeMap<String, String>,
}

fn builtin_file() -> &'static BuiltinFile {
    static FILE: std::sync::OnceLock<BuiltinFile> = std::sync::OnceLock::new();
    FILE.get_or_init(|| {
        serde_json::from_str(include_str!("../papers/builtin.json")).unwrap_or_default()
    })
}

/// The bundled papers.
pub fn builtin() -> &'static [Paper] {
    &builtin_file().papers
}

/// The bundled RFID barcode → paper id table. A user link for the same barcode wins.
pub fn builtin_links() -> &'static BTreeMap<String, String> {
    &builtin_file().rfid
}

/// The paper id of `barcode`: the user's link first, then the bundled table.
fn linked_id<'a>(
    user: &'a BTreeMap<String, String>,
    bundled: &'a BTreeMap<String, String>,
    barcode: &str,
) -> Option<&'a str> {
    user.get(barcode)
        .or_else(|| bundled.get(barcode))
        .map(String::as_str)
}

/// The user's papers and barcode links (the paper file).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserPapers {
    #[serde(default = "version_one")]
    pub version: u32,
    #[serde(default)]
    pub papers: Vec<Paper>,
    /// RFID barcode → paper id (bundled or user).
    #[serde(default)]
    pub links: BTreeMap<String, String>,
}

fn version_one() -> u32 {
    1
}

impl UserPapers {
    /// Read `path`; a missing file is an empty library.
    pub fn load(path: &Path) -> Result<Self, CommandError> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| {
                CommandError::new(
                    ErrorCode::Internal,
                    format!("{} is damaged: {e}", path.display()),
                )
                .with_hint("move the file away to start a new paper library")
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                version: 1,
                ..Self::default()
            }),
            Err(e) => Err(io_error("read", path, &e)),
        }
    }

    /// Write `path` through a temporary file and a rename.
    pub fn save(&self, path: &Path) -> Result<(), CommandError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| io_error("create", dir, &e))?;
        }
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = std::path::PathBuf::from(tmp);
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?;
        std::fs::write(&tmp, json).map_err(|e| io_error("write", &tmp, &e))?;
        std::fs::rename(&tmp, path).map_err(|e| io_error("write", path, &e))
    }

    /// Bundled papers followed by the user's.
    pub fn all(&self) -> impl Iterator<Item = &Paper> {
        builtin().iter().chain(self.papers.iter())
    }

    pub fn get(&self, id: &str) -> Option<&Paper> {
        self.all().find(|p| p.id == id)
    }

    /// The paper linked to an RFID `barcode`, if any: the user's link, else the bundled table.
    pub fn by_barcode(&self, barcode: &str) -> Option<&Paper> {
        linked_id(&self.links, builtin_links(), barcode).and_then(|id| self.get(id))
    }

    /// Barcodes linked to paper `id`, by the user or by the bundled table.
    pub fn barcodes_of(&self, id: &str) -> Vec<String> {
        let bundled = builtin_links()
            .iter()
            .filter(|(b, _)| !self.links.contains_key(*b));
        let mut out: Vec<String> = self
            .links
            .iter()
            .chain(bundled)
            .filter(|(_, p)| *p == id)
            .map(|(b, _)| b.clone())
            .collect();
        out.sort_unstable();
        out
    }

    /// A free `my:` id derived from `name`.
    pub fn new_id(&self, name: &str) -> String {
        let mut slug = String::new();
        for c in name.trim().chars().flat_map(char::to_lowercase) {
            if c.is_ascii_alphanumeric() {
                slug.push(c);
            } else if !slug.ends_with('-') && !slug.is_empty() {
                slug.push('-');
            }
        }
        let slug = slug.trim_end_matches('-');
        let base = if slug.is_empty() { "paper" } else { slug };
        let base: String = base.chars().take(40).collect();
        let mut id = format!("{CUSTOM_PREFIX}{base}");
        let mut n = 2;
        while self.get(&id).is_some() {
            id = format!("{CUSTOM_PREFIX}{base}-{n}");
            n += 1;
        }
        id
    }
}

fn io_error(what: &str, path: &Path, e: &std::io::Error) -> CommandError {
    CommandError::new(
        ErrorCode::Internal,
        format!("cannot {what} {}: {e}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_table_is_valid() {
        let papers = builtin();
        assert!(papers.len() > 30);
        let mut ids: Vec<_> = papers.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), papers.len(), "ids are unique");
        for p in papers {
            assert!(p.id.starts_with(BUILTIN_PREFIX), "{}", p.id);
            assert!(p.width_mm > 0.0 && p.height_mm > 0.0, "{}", p.id);
        }
    }

    #[test]
    fn bundled_links_point_at_bundled_papers() {
        for (barcode, id) in builtin_links() {
            assert!(!barcode.is_empty());
            assert!(builtin().iter().any(|p| &p.id == id), "{barcode} -> {id}");
        }
    }

    #[test]
    fn user_link_wins_over_bundled() {
        let bundled = BTreeMap::from([("111".to_owned(), "std:50x30".to_owned())]);
        let mut user = BTreeMap::new();
        assert_eq!(linked_id(&user, &bundled, "111"), Some("std:50x30"));
        assert_eq!(linked_id(&user, &bundled, "222"), None);
        user.insert("111".to_owned(), "std:40x30".to_owned());
        assert_eq!(linked_id(&user, &bundled, "111"), Some("std:40x30"));
    }

    #[test]
    fn ids_and_links() {
        let mut u = UserPapers::default();
        assert_eq!(u.new_id("Jar lids 38 mm!"), "my:jar-lids-38-mm");
        assert_eq!(u.new_id("  "), "my:paper");
        u.papers.push(Paper {
            id: "my:jar".into(),
            name: "Jar".into(),
            width_mm: 38.0,
            height_mm: 38.0,
            corner_radius_mm: 19.0,
            media: MediaType::Gap,
        });
        assert_eq!(u.new_id("jar"), "my:jar-2");
        u.links.insert("123".into(), "std:50x30".into());
        assert_eq!(u.by_barcode("123").map(|p| p.width_mm), Some(50.0));
        assert_eq!(u.barcodes_of("std:50x30"), vec!["123".to_owned()]);
        assert!(u.by_barcode("999").is_none());
    }

    #[test]
    fn matching_uses_tolerance_and_media() {
        let p = builtin().iter().find(|p| p.id == "std:50x30").unwrap();
        let mut l = LabelSpec::default();
        assert!(p.matches(&l));
        l.width_mm = 50.4;
        assert!(p.matches(&l));
        l.height_mm = 40.0;
        assert!(!p.matches(&l));
        l.height_mm = 30.0;
        l.media = MediaType::Continuous;
        assert!(!p.matches(&l));
    }
}
