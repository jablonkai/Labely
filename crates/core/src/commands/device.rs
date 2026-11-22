//! `device.*` and `print.submit`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{DEFAULT_DPI, render_options_for};
use crate::command::{Command, CommandError, CommandMeta, Ctx, ErrorCode};
use crate::document::MediaType;
use crate::jobs::{JobSpec, JobState};
use crate::services::{
    DeviceInfo, DeviceSettings, DeviceStatus, GrayImage, ModelInfo, PagePlanes, PrintRequest,
    RenderOutput,
};

/// Longest allowed scan.
pub const MAX_SCAN_MS: u64 = 60_000;

/// Explicit address, else the single connected device.
pub(crate) fn pick_address(ctx: &Ctx<'_>, address: Option<String>) -> Result<String, CommandError> {
    if let Some(a) = address {
        return Ok(a);
    }
    let connected = ctx.services().devices.connected();
    match connected.as_slice() {
        [one] => Ok(one.address.clone()),
        [] => Err(
            CommandError::new(ErrorCode::DeviceUnavailable, "no printer is connected")
                .with_hint("run device.scan, then device.connect {address}"),
        ),
        many => Err(
            CommandError::bad_params("several printers are connected; pass `address`")
                .with_param("address")
                .with_accepted(many.iter().map(|d| d.address.clone())),
        ),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanParams {
    /// How long to scan, milliseconds (default 5000, max 60000).
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DevicesOutput {
    pub devices: Vec<DeviceInfo>,
}

/// `device.scan`
#[derive(Debug)]
pub struct DeviceScan;

impl Command for DeviceScan {
    const ID: &'static str = "device.scan";
    type Params = ScanParams;
    type Output = DevicesOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Scan for printers",
            "Discover printers for timeout_ms milliseconds (default 5000). Returns {devices} \
             with addresses usable in device.connect.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ScanParams) -> Result<DevicesOutput, CommandError> {
        let t = p.timeout_ms.unwrap_or(5000);
        if t > MAX_SCAN_MS {
            return Err(
                CommandError::bad_params(format!("timeout_ms must be ≤ {MAX_SCAN_MS}"))
                    .with_param("timeout_ms"),
            );
        }
        Ok(DevicesOutput {
            devices: ctx.services().devices.scan(t)?,
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddressParams {
    /// Device address, e.g. `ble:…`, `usb:…`; defaults to the single connected printer.
    #[serde(default)]
    pub address: Option<String>,
}

/// `device.list`
#[derive(Debug)]
pub struct DeviceList;

impl Command for DeviceList {
    const ID: &'static str = "device.list";
    type Params = crate::command::NoParams;
    type Output = DevicesOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device("Connected printers", "List currently connected printers.")
    }

    fn run(ctx: &mut Ctx<'_>, _: crate::command::NoParams) -> Result<DevicesOutput, CommandError> {
        Ok(DevicesOutput {
            devices: ctx.services().devices.connected(),
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ModelsOutput {
    pub models: Vec<ModelInfo>,
}

/// `device.models`
#[derive(Debug)]
pub struct DeviceModels;

impl Command for DeviceModels {
    const ID: &'static str = "device.models";
    type Params = crate::command::NoParams;
    type Output = ModelsOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Printer models",
            "List the printer models Labely knows, with resolution, print width, density range \
             and media. The ids go in a label's printer_model and in `?model=<id>` addresses.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, _: crate::command::NoParams) -> Result<ModelsOutput, CommandError> {
        Ok(ModelsOutput {
            models: ctx.services().devices.models(),
        })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectParams {
    /// Address from device.scan, e.g. `ble:…` or `usb:…`.
    pub address: String,
}

/// `device.connect`
#[derive(Debug)]
pub struct DeviceConnect;

impl Command for DeviceConnect {
    const ID: &'static str = "device.connect";
    type Params = ConnectParams;
    type Output = DeviceInfo;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Connect printer",
            "Connect to a printer by address (from device.scan). Returns the device with its model.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ConnectParams) -> Result<DeviceInfo, CommandError> {
        if let Some(rest) = p.address.strip_prefix("fake:") {
            // FakePrinter writes PNGs into this directory: it is a file write.
            let (dir, _) = crate::address::split_query(rest);
            ctx.check_path("address", dir, true)?;
        }
        Ok(ctx.services().devices.connect(&p.address)?)
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DisconnectOutput {
    pub address: String,
}

/// `device.disconnect`
#[derive(Debug)]
pub struct DeviceDisconnect;

impl Command for DeviceDisconnect {
    const ID: &'static str = "device.disconnect";
    type Params = AddressParams;
    type Output = DisconnectOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Disconnect printer",
            "Disconnect a printer (default: the single connected one).",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: AddressParams) -> Result<DisconnectOutput, CommandError> {
        let address = pick_address(ctx, p.address)?;
        ctx.services().devices.disconnect(&address)?;
        Ok(DisconnectOutput { address })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StatusOutput {
    pub address: String,
    pub status: DeviceStatus,
}

/// `device.status`
#[derive(Debug)]
pub struct DeviceStatusCmd;

impl Command for DeviceStatusCmd {
    const ID: &'static str = "device.status";
    type Params = AddressParams;
    type Output = StatusOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Printer status",
            "Read battery %, lid, paper presence and RFID paper info (default: the single \
             connected printer).",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: AddressParams) -> Result<StatusOutput, CommandError> {
        let address = pick_address(ctx, p.address)?;
        let status = ctx.services().devices.status(&address)?;
        Ok(StatusOutput { address, status })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalibrateParams {
    /// Printer address; defaults to the single connected printer.
    #[serde(default)]
    pub address: Option<String>,
    /// Paper type to calibrate for; defaults to the active document's label media, else gap.
    #[serde(default)]
    pub media: Option<MediaType>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CalibrateOutput {
    pub address: String,
    pub media: MediaType,
}

/// `device.calibrate`
#[derive(Debug)]
pub struct DeviceCalibrate;

impl Command for DeviceCalibrate {
    const ID: &'static str = "device.calibrate";
    type Params = CalibrateParams;
    type Output = CalibrateOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Calibrate paper",
            "Calibrate the printer's label sensor: it feeds labels until it finds a label \
             edge (gap or black mark). Run it after loading a new kind of paper when labels \
             print out of position. media defaults to the active label's. Feeds paper, so \
             it needs print permission like print.submit.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: CalibrateParams) -> Result<CalibrateOutput, CommandError> {
        let address = pick_address(ctx, p.address)?;
        let media = p
            .media
            .or_else(|| ctx.doc(None).ok().map(|d| d.label.media))
            .unwrap_or_default();
        if media == MediaType::Continuous {
            return Err(CommandError::bad_params(
                "continuous paper has no label edge to calibrate on",
            )
            .with_param("media")
            .with_accepted(["gap", "black_mark", "transparent"].map(String::from)));
        }
        if ctx.session().jobs().is_busy() {
            return Err(CommandError::new(
                ErrorCode::DeviceUnavailable,
                "print jobs are queued or printing",
            )
            .with_hint("wait for the jobs to finish (job.list) or cancel them"));
        }
        ctx.services().devices.calibrate(&address, media)?;
        Ok(CalibrateOutput { address, media })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SettingsOutput {
    pub address: String,
    /// Settings the printer reported; missing ones are unsupported or unknown.
    pub settings: DeviceSettings,
}

/// `device.settings`
#[derive(Debug)]
pub struct DeviceSettingsCmd;

impl Command for DeviceSettingsCmd {
    const ID: &'static str = "device.settings";
    type Params = AddressParams;
    type Output = SettingsOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Printer settings",
            "Read the printer's settings: auto_off_level (1–4; NIIMBOT 15, 30, 45, 60 min), \
             sound_power and sound_bluetooth beeps. Settings the printer does not report are \
             left out.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: AddressParams) -> Result<SettingsOutput, CommandError> {
        let address = pick_address(ctx, p.address)?;
        let settings = ctx.services().devices.settings(&address)?;
        Ok(SettingsOutput { address, settings })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSettingsParams {
    /// Printer address; defaults to the single connected printer.
    #[serde(default)]
    pub address: Option<String>,
    /// Auto power-off level 1–4 (1 = shortest).
    #[serde(default)]
    pub auto_off_level: Option<u8>,
    /// Beep on power on and off.
    #[serde(default)]
    pub sound_power: Option<bool>,
    /// Beep when a Bluetooth host connects.
    #[serde(default)]
    pub sound_bluetooth: Option<bool>,
}

/// `device.set_settings`
#[derive(Debug)]
pub struct DeviceSetSettings;

impl Command for DeviceSetSettings {
    const ID: &'static str = "device.set_settings";
    type Params = SetSettingsParams;
    type Output = SettingsOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Change printer settings",
            "Change printer settings; only the given fields change. auto_off_level 1–4, \
             sound_power and sound_bluetooth on/off. Returns the settings read back.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: SetSettingsParams) -> Result<SettingsOutput, CommandError> {
        let address = pick_address(ctx, p.address)?;
        if let Some(l) = p.auto_off_level
            && !(1..=4).contains(&l)
        {
            return Err(
                CommandError::bad_params("auto_off_level must be 1–4").with_param("auto_off_level")
            );
        }
        let patch = DeviceSettings {
            auto_off_level: p.auto_off_level,
            sound_power: p.sound_power,
            sound_bluetooth: p.sound_bluetooth,
        };
        if patch == DeviceSettings::default() {
            return Err(CommandError::bad_params("no setting to change")
                .with_hint("pass auto_off_level, sound_power or sound_bluetooth"));
        }
        let settings = ctx.services().devices.set_settings(&address, &patch)?;
        Ok(SettingsOutput { address, settings })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrintParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Printer address; defaults to the single connected printer.
    #[serde(default)]
    pub address: Option<String>,
    /// Number of copies, 1–999 (default 1).
    #[serde(default)]
    pub copies: Option<u16>,
    /// Print density; defaults to the label's density, else the model default.
    #[serde(default)]
    pub density: Option<u8>,
    /// Data records to print, 1-based: `all` (default), `3`, `1,4,6-9`. Needs a data source.
    #[serde(default)]
    pub records: Option<String>,
    /// Labels to print without a data source, 1–5000 (default 1); serials advance per label.
    #[serde(default)]
    pub count: Option<u32>,
    /// Column holding each record's copy count (0 skips the record); overrides `copies`.
    #[serde(default)]
    pub copies_column: Option<String>,
    /// Move the print right (positive) or left, in mm, −10 to 10 (default 0).
    #[serde(default)]
    pub offset_x_mm: Option<f32>,
    /// Move the print down (positive, later in the feed) or up, in mm, −10 to 10 (default 0).
    #[serde(default)]
    pub offset_y_mm: Option<f32>,
    /// Mirror the print left to right, for transparent or iron-on media (default false).
    #[serde(default)]
    pub mirror: Option<bool>,
    /// Turn the print 180° so labels leave the printer the other way round (default false).
    #[serde(default)]
    pub rotate_180: Option<bool>,
    /// Print photos and shades in 16 gray levels instead of 1-bit dots (default false).
    /// Only for models whose `gray` is true (device.list); others reject it.
    #[serde(default)]
    pub grayscale: Option<bool>,
    /// Wait until the job has printed (default true). With false the job is queued and the
    /// command returns at once; follow it with job.get or job.wait.
    #[serde(default)]
    pub wait: Option<bool>,
}

/// Largest print offset either way, in mm.
pub const MAX_OFFSET_MM: f32 = 10.0;

/// Job-wide changes to every rendered page: offset, mirror and 180° turn.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct PageTransform {
    dx: i64,
    dy: i64,
    mirror: bool,
    rotate_180: bool,
}

impl PageTransform {
    fn from_params(p: &PrintParams, dpi: u32) -> Result<Self, CommandError> {
        let dots = |mm: Option<f32>, name: &'static str| -> Result<i64, CommandError> {
            let mm = mm.unwrap_or(0.0);
            if !mm.is_finite() || mm.abs() > MAX_OFFSET_MM {
                return Err(CommandError::bad_params(format!(
                    "{name} must be between −{MAX_OFFSET_MM} and {MAX_OFFSET_MM} mm"
                ))
                .with_param(name));
            }
            Ok((f64::from(mm) * f64::from(dpi) / 25.4).round() as i64)
        };
        Ok(Self {
            dx: dots(p.offset_x_mm, "offset_x_mm")?,
            dy: dots(p.offset_y_mm, "offset_y_mm")?,
            mirror: p.mirror.unwrap_or(false),
            rotate_180: p.rotate_180.unwrap_or(false),
        })
    }

    /// Offset first (in label coordinates), then mirror, then the 180° turn.
    fn apply(&self, mut b: crate::raster::Bitmap1) -> crate::raster::Bitmap1 {
        if self.dx != 0 || self.dy != 0 {
            b = b.shifted(self.dx, self.dy);
        }
        if self.mirror {
            b = b.mirror_h();
        }
        if self.rotate_180 {
            b = b.rotate_180();
        }
        b
    }

    /// As [`Self::apply`], for a gray map.
    fn apply_gray(&self, mut g: crate::raster::Gray4) -> crate::raster::Gray4 {
        if self.dx != 0 || self.dy != 0 {
            g = g.shifted(self.dx, self.dy);
        }
        if self.mirror {
            g = g.mirror_h();
        }
        if self.rotate_180 {
            g = g.rotate_180();
        }
        g
    }
}

/// `page` with the dots of `red` cleared: the black plane of a two-colour page.
fn without(
    mut page: crate::raster::Bitmap1,
    red: &crate::raster::Bitmap1,
) -> crate::raster::Bitmap1 {
    for y in 0..page.height().min(red.height()) {
        for x in 0..page.width().min(red.width()) {
            if red.get(x, y) {
                page.set(x, y, false);
            }
        }
    }
    page
}

/// The gray map of a rendered page: the renderer's gray stage, else the 1-bit page.
fn gray_of(out: &RenderOutput, preview: Option<GrayImage>) -> crate::raster::Gray4 {
    use crate::raster::Gray4;
    match preview {
        Some(g) if g.width == out.bitmap.width() && g.height == out.bitmap.height() => {
            Gray4::from_gray8(g.width, g.height, &g.data)
        }
        _ => {
            let b = &out.bitmap;
            let mut g = Gray4::new(b.width(), b.height());
            for y in 0..b.height() {
                for x in 0..b.width() {
                    if b.get(x, y) {
                        g.set(x, y, Gray4::BLACK);
                    }
                }
            }
            g
        }
    }
}

/// Most distinct labels (records × pages, with per-record copies expanded) in one job.
pub const MAX_JOB_LABELS: usize = 5000;

/// The distinct labels of a job: (data row, job position, copies).
fn plan_labels(
    doc: &crate::document::Document,
    p: &PrintParams,
    copies: u16,
) -> Result<Vec<(Option<usize>, u64, u16)>, CommandError> {
    let Some(data) = &doc.data else {
        if p.records.is_some() || p.copies_column.is_some() {
            return Err(CommandError::bad_params(
                "`records` and `copies_column` need a data source",
            )
            .with_hint("attach data with data.attach_csv or data.attach_xlsx"));
        }
        let count = p.count.unwrap_or(1);
        if !(1..=MAX_JOB_LABELS as u32).contains(&count) {
            return Err(
                CommandError::bad_params(format!("count must be 1–{MAX_JOB_LABELS}"))
                    .with_param("count"),
            );
        }
        return Ok((0..u64::from(count)).map(|n| (None, n, copies)).collect());
    };
    if p.count.is_some() {
        return Err(CommandError::bad_params(
            "`count` is for documents without data; use `records` to pick records",
        )
        .with_param("count"));
    }
    let rows = super::parse_records(p.records.as_deref().unwrap_or("all"), data.rows.len())
        .map_err(|e| {
            CommandError::bad_params(e)
                .with_param("records")
                .with_hint("records are 1-based, e.g. `1,3,5-7` or `all`")
        })?;
    if rows.is_empty() {
        return Err(CommandError::bad_params("no records to print").with_param("records"));
    }
    let mut out = Vec::with_capacity(rows.len());
    for (n, row) in rows.into_iter().enumerate() {
        let c = match &p.copies_column {
            None => copies,
            Some(col) => {
                let Some(cell) = data.cell(row, col) else {
                    return Err(CommandError::not_found(format!("no column `{col}`"))
                        .with_param("copies_column")
                        .with_accepted(data.columns.clone()));
                };
                let cell = cell.trim();
                match cell.parse::<u16>() {
                    Ok(c) if c <= 999 => c,
                    _ if cell.is_empty() => 0,
                    _ => {
                        return Err(CommandError::bad_params(format!(
                            "record {}: copies `{cell}` in column `{col}` is not 0–999",
                            row + 1
                        ))
                        .with_param("copies_column"));
                    }
                }
            }
        };
        if c > 0 {
            out.push((Some(row), n as u64, c));
        }
    }
    Ok(out)
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PrintOutput {
    /// Queue id for job.get, job.wait and job.cancel.
    pub job_id: String,
    /// `done` after a waited print; `queued` or `printing` with `wait: false`.
    pub state: JobState,
    pub address: String,
    /// Distinct labels rendered (records or count, times pages).
    pub labels: u32,
    /// Pages printed so far (0 while queued).
    pub pages_printed: u32,
    pub duration_ms: u64,
    pub copies: u16,
    pub density: u8,
    pub dpi: u32,
}

/// `print.submit`
#[derive(Debug)]
pub struct PrintSubmit;

impl Command for PrintSubmit {
    const ID: &'static str = "print.submit";
    type Params = PrintParams;
    type Output = PrintOutput;

    fn meta() -> CommandMeta {
        CommandMeta::device(
            "Print",
            "Render and print at the connected model's dpi: one label per data record \
             (`records` like `1,3,5-7`, default all) or `count` labels without data, with \
             bindings and serials filled in. copies 1–999 (default 1) or per record from \
             `copies_column`; density defaults to the label's, else the model's. Jobs go \
             through the print queue; by default the command waits until the job has \
             printed, `wait: false` returns {job_id} at once. offset_x_mm/offset_y_mm (±10) \
             move the print, mirror flips it for transparent media, rotate_180 turns it \
             for the other output direction. grayscale prints 16 gray levels instead of \
             dithered dots, on models with `gray`. Elements whose color is red print red on \
             models with `red` and black on the others. Run doc.check first.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: PrintParams) -> Result<PrintOutput, CommandError> {
        let doc = ctx.doc_arc(p.doc_id.as_deref())?;
        let address = pick_address(ctx, p.address.clone())?;
        let devices = ctx.services().devices.clone();
        let model = devices
            .connected()
            .into_iter()
            .find(|d| d.address == address)
            .ok_or_else(|| {
                CommandError::new(
                    ErrorCode::DeviceUnavailable,
                    format!("printer {address} is not connected"),
                )
                .with_param("address")
                .with_hint("run device.connect {address} first")
            })?
            .model;
        let copies = p.copies.unwrap_or(1);
        if !(1..=999).contains(&copies) {
            return Err(CommandError::bad_params("copies must be 1–999").with_param("copies"));
        }
        let dpi = model.as_ref().map_or(DEFAULT_DPI, |m| m.dpi);
        let density = match (&model, p.density.or(doc.label.density)) {
            (Some(m), Some(d)) if !(m.density_min..=m.density_max).contains(&d) => {
                return Err(CommandError::bad_params(format!(
                    "density {d} is outside the {} range {}–{}",
                    m.name, m.density_min, m.density_max
                ))
                .with_param("density"));
            }
            (_, Some(d)) => d,
            (Some(m), None) => m.density_default,
            (None, None) => 3,
        };
        if let Some(m) = &model
            && doc.label.width_mm > m.max_width_mm + 0.01
        {
            ctx.warn(format!(
                "label width {} mm exceeds the {} print width {} mm; the print will be clipped",
                doc.label.width_mm, m.name, m.max_width_mm
            ));
        }
        let grayscale = p.grayscale.unwrap_or(false);
        if grayscale && !model.as_ref().is_some_and(|m| m.gray) {
            let name = model.as_ref().map_or("this printer", |m| m.name.as_str());
            return Err(
                CommandError::bad_params(format!("{name} does not print gray levels"))
                    .with_param("grayscale")
                    .with_hint("print without grayscale; see `gray` in device.list"),
            );
        }
        let transform = PageTransform::from_params(&p, dpi)?;
        let labels = plan_labels(&doc, &p, copies)?;
        let uniform = labels.iter().all(|l| l.2 == copies);
        let total: usize = if uniform {
            labels.len() * doc.pages.len()
        } else {
            labels.iter().map(|l| usize::from(l.2)).sum::<usize>() * doc.pages.len()
        };
        if total > MAX_JOB_LABELS {
            return Err(CommandError::bad_params(format!(
                "the job has {total} labels; at most {MAX_JOB_LABELS} are printed at once"
            ))
            .with_param("records")
            .with_hint("print fewer records per job"));
        }
        if labels.is_empty() {
            return Err(CommandError::bad_params(
                "every selected record has 0 copies; nothing to print",
            )
            .with_param("copies_column"));
        }
        let renderer = ctx.services().renderer.clone();
        let mut pages = Vec::with_capacity(total);
        let mut grays = Vec::with_capacity(if grayscale { total } else { 0 });
        let mut reds: Vec<Option<crate::raster::Bitmap1>> = Vec::with_capacity(total);
        for &(row, n, c) in &labels {
            for page in 0..doc.pages.len() {
                let opts = render_options_for(&doc, dpi, page, row, n);
                let rendered = if grayscale {
                    renderer.render_with_preview(&doc, &opts)
                } else {
                    renderer.render(&doc, &opts).map(|o| (o, None))
                };
                let (out, preview) = rendered.map_err(|e| {
                    let at = row.map_or(String::new(), |r| format!(" of record {}", r + 1));
                    CommandError::bad_params(format!("cannot render page {page}{at}: {e}"))
                })?;
                let reps = if uniform { 1 } else { c };
                if grayscale {
                    let gray = transform.apply_gray(gray_of(&out, preview));
                    for _ in 1..reps {
                        grays.push(gray.clone());
                    }
                    grays.push(gray);
                }
                let (black, red) = match out.red {
                    Some(red) if !grayscale => (without(out.bitmap, &red), Some(red)),
                    _ => (out.bitmap, None),
                };
                let red = red.map(|r| transform.apply(r));
                let bitmap = transform.apply(black);
                for _ in 1..reps {
                    reds.push(red.clone());
                }
                reds.push(red);
                for _ in 1..reps {
                    pages.push(bitmap.clone());
                }
                pages.push(bitmap);
            }
        }
        let distinct = (labels.len() * doc.pages.len()) as u32;
        let planes = if grayscale {
            PagePlanes::Gray(grays)
        } else if reds.iter().any(Option::is_some) {
            PagePlanes::Red(
                reds.into_iter()
                    .zip(&pages)
                    .map(|(r, b)| {
                        r.unwrap_or_else(|| crate::raster::Bitmap1::new(b.width(), b.height()))
                    })
                    .collect(),
            )
        } else {
            PagePlanes::None
        };
        let request = PrintRequest {
            pages,
            copies: if uniform { copies } else { 1 },
            density,
            media: doc.label.media,
            planes,
            half_cut: None,
        };
        let spec = JobSpec {
            address: address.clone(),
            model: model.as_ref().map(|m| m.id.clone()),
            labels: distinct,
            copies,
            density,
            pages_total: (request.pages.len() * usize::from(request.copies)) as u32,
            doc,
        };
        let queue = ctx.session().jobs().clone();
        if p.wait.unwrap_or(true) && queue.is_paused() {
            return Err(CommandError::bad_params(
                "the print queue is paused; a waited print would never start",
            )
            .with_param("wait")
            .with_hint("run job.resume, or pass wait: false to queue the job"));
        }
        let job = queue.submit(spec, request)?;
        let job = if p.wait.unwrap_or(true) {
            let done = queue.wait(&job.id, None)?;
            if let Some(e) = done.error {
                return Err(e);
            }
            done
        } else {
            job
        };
        Ok(PrintOutput {
            job_id: job.id,
            state: job.state,
            address,
            labels: distinct,
            pages_printed: job.pages_done,
            duration_ms: job.duration_ms.unwrap_or(0),
            copies,
            density,
            dpi,
        })
    }
}
