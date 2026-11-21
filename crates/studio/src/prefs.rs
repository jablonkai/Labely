//! Studio preferences kept between runs: recent files and the length unit.
//!
//! One JSON file under `<app-data>` (`studio.json`), written through a temp file and a rename.
//! A missing or unreadable file gives the defaults.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// File name of the preferences under `<app-data>`.
pub const PREFS_FILE: &str = "studio.json";

/// Most entries in the recent-files list.
pub const MAX_RECENT: usize = 10;

/// Unit for showing and typing lengths. Documents always store mm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    #[default]
    Mm,
    Inch,
}

impl Units {
    pub const ALL: [Units; 2] = [Units::Mm, Units::Inch];

    /// Shown value for `mm`.
    pub fn from_mm(self, mm: f64) -> f64 {
        match self {
            Units::Mm => mm,
            Units::Inch => mm / 25.4,
        }
    }

    /// mm for a shown value.
    pub fn to_mm(self, v: f64) -> f64 {
        match self {
            Units::Mm => v,
            Units::Inch => v * 25.4,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Units::Mm => "mm",
            Units::Inch => "in",
        }
    }

    pub fn decimals(self) -> usize {
        match self {
            Units::Mm => 2,
            Units::Inch => 3,
        }
    }

    /// i18n key of the unit's name.
    pub fn key(self) -> &'static str {
        match self {
            Units::Mm => "units-mm",
            Units::Inch => "units-inch",
        }
    }

    /// Format `mm` in this unit, e.g. `12.5 mm` or `0.492 in`.
    pub fn format(self, mm: f64) -> String {
        let v = self.from_mm(mm);
        let s = format!("{v:.*}", self.decimals());
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!("{s} {}", self.symbol())
    }

    /// Parse typed text in this unit (a trailing `mm`, `in` or `"` picks that unit), as mm.
    pub fn parse(self, text: &str) -> Option<f64> {
        let t = text.trim().replace(',', ".");
        let (num, unit) = if let Some(n) = t.strip_suffix("mm") {
            (n, Units::Mm)
        } else if let Some(n) = t.strip_suffix("in").or_else(|| t.strip_suffix('"')) {
            (n, Units::Inch)
        } else {
            (t.as_str(), self)
        };
        let v: f64 = num.trim().parse().ok()?;
        v.is_finite().then(|| unit.to_mm(v))
    }
}

/// Everything the studio remembers between runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// Recently opened or saved documents, newest first.
    pub recent: Vec<PathBuf>,
    pub units: Units,
    /// UI language code ("en", "hu"); `None` follows the system language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Label colour shown on the canvas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_color: Option<[u8; 3]>,
    /// Font files added with File → Add Font, loaded at start.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<PathBuf>,
    /// Print offset `[right, down]` in mm per printer address, kept from the last print.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub print_offsets: std::collections::BTreeMap<String, [f32; 2]>,
    /// Apply a recognised RFID paper to the label without asking (ADR-0041).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub auto_apply_paper: bool,
}

impl Prefs {
    /// Read `path`; defaults when it is missing or invalid.
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }

    /// Put `path` first in the recent list. Returns whether the list changed.
    pub fn push_recent(&mut self, path: &Path) -> bool {
        if self.recent.first().is_some_and(|p| p == path) {
            return false;
        }
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(MAX_RECENT);
        true
    }

    pub fn remove_recent(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_convert_format_and_parse() {
        assert_eq!(Units::Inch.format(25.4), "1 in");
        assert_eq!(Units::Mm.format(12.5), "12.5 mm");
        assert_eq!(Units::Inch.parse("2"), Some(50.8));
        assert_eq!(Units::Inch.parse("10 mm"), Some(10.0));
        assert_eq!(Units::Mm.parse("1\""), Some(25.4));
        assert_eq!(Units::Mm.parse("1,5"), Some(1.5));
        assert_eq!(Units::Mm.parse("abc"), None);
    }

    #[test]
    fn recent_list_and_round_trip() {
        let mut p = Prefs::default();
        for i in 0..12 {
            p.push_recent(Path::new(&format!("/l/{i}.labely")));
        }
        assert_eq!(p.recent.len(), MAX_RECENT);
        assert!(p.push_recent(Path::new("/l/5.labely")));
        assert_eq!(p.recent[0], Path::new("/l/5.labely"));
        assert_eq!(
            p.recent.iter().filter(|r| r.ends_with("5.labely")).count(),
            1
        );
        assert!(!p.push_recent(Path::new("/l/5.labely")));

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub").join(PREFS_FILE);
        p.units = Units::Inch;
        p.save(&file).unwrap();
        assert_eq!(Prefs::load(&file), p);
        assert_eq!(Prefs::load(&dir.path().join("none.json")), Prefs::default());
    }
}
