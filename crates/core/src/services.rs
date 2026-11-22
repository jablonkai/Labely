//! Service traits injected into a `Session` (ADR-0003). Core defines them; `labely-render`
//! and `labely-printers` implement them; `labely-testkit` provides fakes.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::document::{Document, ElementId, MediaType};
use crate::raster::{Bitmap1, Gray4};

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
    /// The longest printed line is wider than the element's `max_length_mm`.
    TextTooLong {
        element_id: ElementId,
        length_mm: f64,
        max_length_mm: f64,
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
    /// Placement of a drawn code: blank dots between the symbol and its box on the
    /// quiet-zone sides, and the quiet zone the symbology needs, in modules.
    CodeLayout {
        element_id: ElementId,
        module_dots: u32,
        slack_dots: u32,
        quiet_modules: u32,
    },
    /// A dithered image ended up almost all white or all black.
    LowContrast {
        element_id: ElementId,
        black_percent: f64,
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

/// 8-bit gray image (row-major, 0 = black, 255 = white) before 1-bit conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Rasteriser.
pub trait Renderer: Send + Sync {
    fn render(&self, doc: &Document, opts: &RenderOptions) -> Result<RenderOutput, RenderError>;
    /// The page as 8-bit gray before 1-bit conversion, for on-screen display at any dpi.
    /// `None` when the renderer has no gray stage.
    fn render_preview_gray(
        &self,
        doc: &Document,
        opts: &RenderOptions,
    ) -> Result<Option<GrayImage>, RenderError> {
        let _ = (doc, opts);
        Ok(None)
    }
    /// [`Renderer::render`] and [`Renderer::render_preview_gray`] of one page in one pass.
    fn render_with_preview(
        &self,
        doc: &Document,
        opts: &RenderOptions,
    ) -> Result<(RenderOutput, Option<GrayImage>), RenderError> {
        Ok((
            self.render(doc, opts)?,
            self.render_preview_gray(doc, opts)?,
        ))
    }
    /// Decode every barcode/2D code visible in `bitmap` (for the readability check).
    fn decode_codes(&self, bitmap: &Bitmap1) -> Vec<DecodedCode>;
    /// Encode a 1-bit bitmap as PNG.
    fn to_png(&self, bitmap: &Bitmap1) -> Result<Vec<u8>, RenderError>;
    /// Installed font family names, sorted; empty when the renderer cannot list them.
    fn fonts(&self) -> Vec<String> {
        Vec::new()
    }
    /// Load a font file's bytes (TTF, OTF or a TTC collection) for the rest of the session.
    /// Returns the family names it added, sorted.
    fn add_font(&self, bytes: Vec<u8>) -> Result<Vec<String>, RenderError> {
        let _ = bytes;
        Err(RenderError::Other(
            "this renderer cannot load font files".to_owned(),
        ))
    }
    /// Names of the built-in icons, with their category.
    fn icons(&self) -> Vec<IconInfo> {
        Vec::new()
    }
    /// Decode every barcode and 2D code in an image file's bytes (PNG, JPEG, BMP, WebP).
    fn decode_image(&self, bytes: &[u8]) -> Result<Vec<DecodedCode>, RenderError> {
        let _ = bytes;
        Err(RenderError::Other(
            "image decoding is not supported".to_owned(),
        ))
    }
}

/// One built-in icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IconInfo {
    pub name: String,
    pub category: String,
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
    /// Prints 16 gray levels (`grayscale` in print.submit).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gray: bool,
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
    /// Firmware version reported at connect, such as "3.12".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    /// Hardware version reported at connect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardware: Option<String>,
    /// Device serial number reported at connect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
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

/// Printer settings that can be read and changed. `None` means the printer does not report
/// (or support) that setting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeviceSettings {
    /// Auto power-off level 1–4 (1 = shortest; NIIMBOT: 15, 30, 45, 60 min).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_off_level: Option<u8>,
    /// Beep on power on and off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_power: Option<bool>,
    /// Beep when a Bluetooth host connects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_bluetooth: Option<bool>,
}

/// Extra planes of the pages of a [`PrintRequest`], for printers that print more than
/// black. A driver that cannot use them prints `pages` alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PagePlanes {
    /// Single colour: `pages` is everything.
    #[default]
    None,
    /// One red plane per page, of the page's size. `pages` holds the black plane. A
    /// printer without red prints the red dots black.
    Red(Vec<Bitmap1>),
    /// One gray map per page, of the page's size. `pages` holds the 1-bit version for
    /// printers without gray levels.
    Gray(Vec<Gray4>),
}

/// A rendered job handed to a device.
#[derive(Debug, Clone)]
pub struct PrintRequest {
    /// One bitmap per distinct page, label orientation, at the model dpi.
    pub pages: Vec<Bitmap1>,
    pub copies: u16,
    pub density: u8,
    pub media: MediaType,
    /// Red or gray planes of `pages`.
    pub planes: PagePlanes,
    /// Switch the printer's half-cut on or off for this job. `None` leaves it alone.
    pub half_cut: Option<bool>,
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
    /// The job was cancelled through its [`JobControl`].
    #[error("print cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

/// Called with `(pages_done, pages_total)` when a driver reports new progress.
pub type ProgressHook = Box<dyn Fn(u32, u32) + Send + Sync>;

/// Shared between a running print and whoever queued it: progress out, cancel in.
#[derive(Default)]
pub struct JobControl {
    cancel: AtomicBool,
    done: AtomicU32,
    total: AtomicU32,
    on_progress: OnceLock<ProgressHook>,
}

impl std::fmt::Debug for JobControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobControl")
            .field("cancel", &self.cancel)
            .field("done", &self.done)
            .field("total", &self.total)
            .finish_non_exhaustive()
    }
}

impl JobControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// A control that calls `hook` on the driver's thread whenever the progress changes.
    pub fn with_progress_hook(hook: ProgressHook) -> Self {
        let c = Self::default();
        let _ = c.on_progress.set(hook);
        c
    }

    /// Ask the driver to stop at the next page boundary.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Report progress (called by drivers).
    pub fn set_progress(&self, done: u32, total: u32) {
        let old_total = self.total.swap(total, Ordering::SeqCst);
        let old_done = self.done.swap(done, Ordering::SeqCst);
        if (old_done, old_total) != (done, total)
            && let Some(hook) = self.on_progress.get()
        {
            hook(done, total);
        }
    }

    /// `(pages_done, pages_total)` reported so far.
    pub fn progress(&self) -> (u32, u32) {
        (
            self.done.load(Ordering::SeqCst),
            self.total.load(Ordering::SeqCst),
        )
    }
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
    /// Print and block until done. Drivers report progress to `control` and stop with
    /// [`DeviceError::Cancelled`] at the next page boundary once it is cancelled.
    fn print(
        &self,
        address: &str,
        job: PrintRequest,
        control: &JobControl,
    ) -> Result<PrintReport, DeviceError>;
    /// Calibrate the label sensor for `media`: the printer feeds until it finds a label edge.
    fn calibrate(&self, address: &str, media: MediaType) -> Result<(), DeviceError> {
        let _ = media;
        Err(DeviceError::Unavailable(format!(
            "{address} does not support calibration"
        )))
    }
    /// Read the printer's settings.
    fn settings(&self, address: &str) -> Result<DeviceSettings, DeviceError> {
        let _ = address;
        Ok(DeviceSettings::default())
    }
    /// Change the settings given in `patch` (fields that are `None` stay), then return them.
    fn set_settings(
        &self,
        address: &str,
        patch: &DeviceSettings,
    ) -> Result<DeviceSettings, DeviceError> {
        let _ = patch;
        Err(DeviceError::Unavailable(format!(
            "{address} has no settings that Labely can change"
        )))
    }
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
