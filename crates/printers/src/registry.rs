//! Printer model registry (data-driven; one JSON entry per model).

use std::sync::OnceLock;

use labely_core::document::MediaType;
use labely_core::services::ModelInfo;
use serde::{Deserialize, Serialize};

/// Print direction: how the label maps onto the head (docs/protocol.md §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrintDirection {
    /// Label rows go straight to the head.
    Top,
    /// Label is rotated 90° clockwise first.
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DensityRange {
    pub min: u8,
    pub max: u8,
    pub default: u8,
}

/// One registry entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub id: String,
    pub vendor: String,
    pub name: String,
    #[serde(default)]
    pub model_codes: Vec<u16>,
    #[serde(default)]
    pub ble_name_prefixes: Vec<String>,
    /// Driver family: `niimbot` or `fake`.
    pub driver: String,
    /// Driver-specific task variant. NIIMBOT: `d11_v1`, `b21_v1`, `b21_l2b`, `d110`, `b1`,
    /// `h1s`, `d110m_v4`, or `auto` (by protocol version).
    pub task: String,
    pub dpi: u32,
    pub head_dots: u32,
    pub max_width_mm: f64,
    pub max_length_mm: f64,
    pub media: Vec<MediaType>,
    pub print_direction: PrintDirection,
    pub density: DensityRange,
    #[serde(default)]
    pub rfid: bool,
    #[serde(default)]
    pub transports: Vec<String>,
}

impl Model {
    pub fn info(&self) -> ModelInfo {
        ModelInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            dpi: self.dpi,
            head_dots: self.head_dots,
            max_width_mm: self.max_width_mm,
            density_min: self.density.min,
            density_max: self.density.max,
            density_default: self.density.default,
            media: self.media.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RegistryFile {
    #[allow(dead_code)]
    schema: u32,
    models: Vec<Model>,
}

const BUILTIN: &[&str] = &[include_str!("../models/niimbot.json")];

/// The model registry.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    models: Vec<Model>,
}

impl Registry {
    /// Built-in models. Malformed built-in data is a build bug caught by tests.
    pub fn builtin() -> &'static Registry {
        static REG: OnceLock<Registry> = OnceLock::new();
        REG.get_or_init(|| {
            let mut r = Registry::default();
            for src in BUILTIN {
                if let Ok(file) = serde_json::from_str::<RegistryFile>(src) {
                    r.models.extend(file.models);
                }
            }
            r
        })
    }

    /// Parse additional models (user registry files).
    pub fn extend_from_json(&mut self, json: &str) -> Result<usize, serde_json::Error> {
        let file: RegistryFile = serde_json::from_str(json)?;
        let n = file.models.len();
        self.models.extend(file.models);
        Ok(n)
    }

    pub fn models(&self) -> &[Model] {
        &self.models
    }

    pub fn by_id(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn by_code(&self, vendor: &str, code: u16) -> Option<&Model> {
        self.models
            .iter()
            .find(|m| m.vendor == vendor && m.model_codes.contains(&code))
    }

    /// Best guess from an advertised BLE name (longest matching model name wins).
    pub fn guess_by_name(&self, name: &str) -> Option<&Model> {
        let norm = |s: &str| s.to_ascii_uppercase().replace(['_', '-', ' '], "");
        let n = norm(name);
        self.models
            .iter()
            .filter(|m| n.starts_with(&norm(&m.name)))
            .max_by_key(|m| m.name.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_parses_and_has_b21_pro() {
        let src = BUILTIN[0];
        let file: RegistryFile = serde_json::from_str(src).unwrap();
        assert!(file.models.len() > 40);
        let r = Registry::builtin();
        let m = r.by_code("niimbot", 785).unwrap();
        assert_eq!(m.id, "niimbot.b21_pro");
        assert_eq!((m.dpi, m.head_dots), (300, 591));
        assert_eq!(m.task, "d110m_v4");
        assert_eq!(
            r.guess_by_name("B21_PRO-1234").unwrap().id,
            "niimbot.b21_pro"
        );
        assert_eq!(r.guess_by_name("B21-C2B").unwrap().id, "niimbot.b21_c2b");
    }

    #[test]
    fn ids_unique() {
        let r = Registry::builtin();
        let mut ids: Vec<_> = r.models().iter().map(|m| &m.id).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
