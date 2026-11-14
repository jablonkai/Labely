//! `device.*` and `print.submit`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{DEFAULT_DPI, render_options_for};
use crate::command::{Command, CommandError, CommandMeta, Ctx, ErrorCode};
use crate::services::{DeviceInfo, DeviceStatus, PrintRequest};

/// Longest allowed scan.
pub const MAX_SCAN_MS: u64 = 60_000;

/// Explicit address, else the single connected device.
fn pick_address(ctx: &Ctx<'_>, address: Option<String>) -> Result<String, CommandError> {
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
            let dir = rest.split('?').next().unwrap_or(rest);
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
    pub address: String,
    /// Distinct labels rendered (records or count, times pages).
    pub labels: u32,
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
             `copies_column`; density defaults to the label's, else the model's. Blocks until \
             done. Run doc.check first.",
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
        for &(row, n, c) in &labels {
            for page in 0..doc.pages.len() {
                let out = renderer
                    .render(&doc, &render_options_for(&doc, dpi, page, row, n))
                    .map_err(|e| {
                        let at = row.map_or(String::new(), |r| format!(" of record {}", r + 1));
                        CommandError::bad_params(format!("cannot render page {page}{at}: {e}"))
                    })?;
                let reps = if uniform { 1 } else { c };
                for _ in 1..reps {
                    pages.push(out.bitmap.clone());
                }
                pages.push(out.bitmap);
            }
        }
        let report = devices.print(
            &address,
            PrintRequest {
                pages,
                copies: if uniform { copies } else { 1 },
                density,
                media: doc.label.media,
            },
        )?;
        Ok(PrintOutput {
            address,
            labels: (labels.len() * doc.pages.len()) as u32,
            pages_printed: report.pages_printed,
            duration_ms: report.duration_ms,
            copies,
            density,
            dpi,
        })
    }
}
