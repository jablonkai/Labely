//! Service traits injected into a `Session` (ADR-0003). Core defines them; `labely-render`
//! and `labely-printers` implement them; `labely-testkit` provides fakes.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::document::{Document, ElementId, MediaType};
use crate::raster::Bitmap1;

/// Render settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RenderOptions {
    /// Output resolution in dots per inch.
    pub dpi: u32,
    /// Page index.
    #[serde(default)]
    pub page: usize,
    /// Values substituted for `{{name}}` bindings (variables and data-record fields).
    #[serde(default)]
    pub values: std::collections::BTreeMap<String, String>,
}

impl RenderOptions {
    pub fn at_dpi(dpi: u32) -> Self {
        Self {
            dpi,
            page: 0,
            values: Default::default(),
        }
    }
}

/// Per-element render diagnostics used by `doc.check`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RenderIssue {
    /// Text did not fit its box (after shrinking, if enabled).
    TextOverflow { element_id: ElementId },
    /// Text was shrunk to this size to fit.
    TextShrunk {
        element_id: ElementId,
        font_size_pt: f64,
    },
    /// Barcode/2D data could not be encoded for the symbology.
    EncodeFailed {
        element_id: ElementId,
        message: String,
    },
    /// Narrowest bar/module in dots.
    ModuleWidth { element_id: ElementId, dots: f64 },
    /// Referenced asset missing or undecodable.
    BadAsset {
        element_id: ElementId,
        message: String,
    },
    /// Font not available; fallback used.
    FontFallback {
        element_id: ElementId,
        requested: String,
    },
}

/// Output of one render.
#[derive(Debug, Clone)]
pub struct RenderOutput {
    /// 1-bit raster of the whole label at `dpi`, label orientation.
    pub bitmap: Bitmap1,
    pub issues: Vec<RenderIssue>,
}

/// A barcode found by decoding a raster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DecodedCode {
    pub format: String,
    pub text: String,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("page {0} does not exist")]
    NoSuchPage(usize),
    #[error("label size {width_mm}×{height_mm} mm is not printable")]
    BadSize { width_mm: f64, height_mm: f64 },
    #[error("{0}")]
    Other(String),
}

/// Rasteriser.
pub trait Renderer: Send + Sync {
    fn render(&self, doc: &Document, opts: &RenderOptions) -> Result<RenderOutput, RenderError>;
    /// Decode every barcode/2D code visible in `bitmap` (for the readability check).
    fn decode_codes(&self, bitmap: &Bitmap1) -> Vec<DecodedCode>;
    /// Encode a 1-bit bitmap as PNG.
    fn to_png(&self, bitmap: &Bitmap1) -> Result<Vec<u8>, RenderError>;
}

/// Printer model facts that commands need (subset of the registry entry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub dpi: u32,
    pub head_dots: u32,
    pub max_width_mm: f64,
    pub density_min: u8,
    pub density_max: u8,
    pub density_default: u8,
    pub media: Vec<MediaType>,
}

/// A known printer (discovered, connected or virtual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceInfo {
    /// Address usable with `device.connect`: `ble:<id>`, `usb:<port>`, `fake:<dir>`.
    pub address: String,
    pub name: String,
    pub connected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelInfo>,
}

/// Label stock reported by the printer (RFID).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PaperInfo {
    pub barcode: String,
    pub serial: String,
    pub total: u32,
    pub used: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaType>,
}

/// Live device status.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceStatus {
    pub battery_percent: Option<u8>,
    pub lid_closed: Option<bool>,
    pub paper_present: Option<bool>,
    pub paper: Option<PaperInfo>,
}

/// A rendered job handed to a device.
#[derive(Debug, Clone)]
pub struct PrintRequest {
    /// One bitmap per distinct page, label orientation, at the model dpi.
    pub pages: Vec<Bitmap1>,
    pub copies: u16,
    pub density: u8,
    pub media: MediaType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PrintReport {
    pub pages_printed: u32,
    pub duration_ms: u64,
}

/// Device-layer error, mapped to `device_unavailable` / `device_error` envelopes.
#[derive(Debug, Error, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum DeviceError {
    #[error("no device at {0}")]
    NotFound(String),
    #[error("device unavailable: {0}")]
    Unavailable(String),
    #[error("printer error: {message}")]
    Printer { message: String, hint: String },
    #[error("timed out: {0}")]
    Timeout(String),
    #[error("{0}")]
    Other(String),
}

/// Printer management.
pub trait DeviceHub: Send + Sync {
    /// Known models in the registry.
    fn models(&self) -> Vec<ModelInfo>;
    /// Discover devices for up to `timeout_ms`.
    fn scan(&self, timeout_ms: u64) -> Result<Vec<DeviceInfo>, DeviceError>;
    /// Currently connected devices.
    fn connected(&self) -> Vec<DeviceInfo>;
    fn connect(&self, address: &str) -> Result<DeviceInfo, DeviceError>;
    fn disconnect(&self, address: &str) -> Result<(), DeviceError>;
    fn status(&self, address: &str) -> Result<DeviceStatus, DeviceError>;
    /// Print and block until done.
    fn print(&self, address: &str, job: PrintRequest) -> Result<PrintReport, DeviceError>;
}

/// Services available to commands.
#[derive(Clone)]
pub struct Services {
    pub renderer: Arc<dyn Renderer>,
    pub devices: Arc<dyn DeviceHub>,
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Services").finish_non_exhaustive()
    }
}
