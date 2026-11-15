//! labely-testkit: dev-only fakes for `labely-core` services.
//!
//! - [`FakeRenderer`]: deterministic, no fonts; every visible element becomes a filled box.
//! - [`FakeDeviceHub`]: one fake printer (`test.fake`, 203 dpi, 384 dots) that records jobs.
//! - [`test_session`] / [`test_kit`]: a `Session` wired with both.

use std::sync::{Arc, Mutex, MutexGuard};

use labely_core::commands::interpolate;
use labely_core::document::{Document, ElementKind, MediaType, TextFit};
use labely_core::services::{
    DecodedCode, DeviceError, DeviceHub, DeviceInfo, DeviceSettings, DeviceStatus, JobControl,
    ModelInfo, PaperInfo, PrintReport, PrintRequest, RenderError, RenderIssue, RenderOptions,
    RenderOutput, Renderer, Services,
};
use labely_core::{Bitmap1, Session};

/// Model id of the fake printer.
pub const FAKE_MODEL_ID: &str = "test.fake";
/// Address of the fake printer.
pub const FAKE_ADDRESS: &str = "fake:printer";

const MM_PER_PT: f64 = 25.4 / 72.0;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Deterministic renderer: filled boxes per element, heuristic text metrics.
///
/// Barcodes and 2D codes "decode" when they are fully inside the label and encodable
/// (non-empty data, at least 2 dots per module), which makes `doc.check` testable.
#[derive(Debug, Default)]
pub struct FakeRenderer {
    /// Recent renders: bitmap and the codes it "contains".
    memo: Mutex<Vec<(Bitmap1, Vec<DecodedCode>)>>,
    /// Binding values of every render, in order.
    values: Mutex<Vec<std::collections::BTreeMap<String, String>>>,
}

impl FakeRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Binding values passed to each render so far, oldest first.
    pub fn rendered_values(&self) -> Vec<std::collections::BTreeMap<String, String>> {
        lock(&self.values).clone()
    }

    /// Forget the recorded binding values.
    pub fn clear_values(&self) {
        lock(&self.values).clear();
    }
}

fn mm_to_dots(mm: f64, dpi: u32) -> i64 {
    (mm * f64::from(dpi) / 25.4).round() as i64
}

impl Renderer for FakeRenderer {
    fn render(&self, doc: &Document, opts: &RenderOptions) -> Result<RenderOutput, RenderError> {
        let page = doc
            .pages
            .get(opts.page)
            .ok_or(RenderError::NoSuchPage(opts.page))?;
        lock(&self.values).push(opts.values.clone());
        let (wmm, hmm) = (doc.label.width_mm, doc.label.height_mm);
        let (w, h) = (mm_to_dots(wmm, opts.dpi), mm_to_dots(hmm, opts.dpi));
        if !(1..=20_000).contains(&w) || !(1..=20_000).contains(&h) {
            return Err(RenderError::BadSize {
                width_mm: wmm,
                height_mm: hmm,
            });
        }
        let mut bitmap = Bitmap1::new(w as u32, h as u32);
        let mut issues = Vec::new();
        let mut codes = Vec::new();
        for e in page.elements.iter().filter(|e| !e.hidden) {
            let x0 = mm_to_dots(e.x_mm, opts.dpi);
            let y0 = mm_to_dots(e.y_mm, opts.dpi);
            let x1 = mm_to_dots(e.x_mm + e.width_mm, opts.dpi).max(x0 + 1);
            let y1 = mm_to_dots(e.y_mm + e.height_mm, opts.dpi).max(y0 + 1);
            for y in y0.max(0)..y1.min(h) {
                for x in x0.max(0)..x1.min(w) {
                    bitmap.set(x as u32, y as u32, true);
                }
            }
            let inside = x0 >= 0 && y0 >= 0 && x1 <= w && y1 <= h;
            match &e.kind {
                ElementKind::Text(t) => {
                    let text = interpolate(&t.text, &opts.values);
                    let lines = text.lines().count().max(1) as f64;
                    let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
                    let need_w = longest * t.font_size_pt * 0.6 * MM_PER_PT;
                    let need_h = lines * t.font_size_pt * MM_PER_PT * t.line_spacing;
                    let scale = (e.width_mm / need_w.max(1e-9)).min(e.height_mm / need_h.max(1e-9));
                    if scale < 1.0 {
                        match t.fit {
                            TextFit::None => {
                                issues.push(RenderIssue::TextOverflow { element_id: e.id })
                            }
                            TextFit::Shrink | TextFit::Fill => {
                                issues.push(RenderIssue::TextShrunk {
                                    element_id: e.id,
                                    font_size_pt: t.font_size_pt * scale,
                                })
                            }
                        }
                    }
                }
                ElementKind::Barcode(b) => {
                    let data = interpolate(&b.data, &opts.values);
                    code(
                        e.id,
                        &data,
                        "barcode",
                        (x1 - x0) as f64,
                        11.0 * (data.len() as f64 + 3.0),
                        inside,
                        &mut issues,
                        &mut codes,
                    );
                }
                ElementKind::Code2d(c) => {
                    let data = interpolate(&c.data, &opts.values);
                    let modules = 21.0 + 4.0 * (data.len() as f64 / 20.0).floor();
                    code(
                        e.id,
                        &data,
                        "qr",
                        ((x1 - x0).min(y1 - y0)) as f64,
                        modules,
                        inside,
                        &mut issues,
                        &mut codes,
                    );
                }
                ElementKind::Image(i) if !doc.assets.contains_key(&i.asset) => {
                    issues.push(RenderIssue::BadAsset {
                        element_id: e.id,
                        message: format!("asset {} is missing", i.asset),
                    });
                }
                _ => {}
            }
        }
        let mut memo = lock(&self.memo);
        memo.push((bitmap.clone(), codes));
        if memo.len() > 32 {
            memo.remove(0);
        }
        Ok(RenderOutput { bitmap, issues })
    }

    fn decode_codes(&self, bitmap: &Bitmap1) -> Vec<DecodedCode> {
        lock(&self.memo)
            .iter()
            .rev()
            .find(|(b, _)| b == bitmap)
            .map(|(_, c)| c.clone())
            .unwrap_or_default()
    }

    fn to_png(&self, bitmap: &Bitmap1) -> Result<Vec<u8>, RenderError> {
        Ok(png_1bit(bitmap))
    }

    fn icons(&self) -> Vec<labely_core::services::IconInfo> {
        [("check", "symbols"), ("fragile", "handling")]
            .into_iter()
            .map(|(n, c)| labely_core::services::IconInfo {
                name: n.to_owned(),
                category: c.to_owned(),
            })
            .collect()
    }

    /// "Decodes" UTF-8 bytes of the form `format:text` (one code per line).
    fn decode_image(&self, bytes: &[u8]) -> Result<Vec<DecodedCode>, RenderError> {
        let s =
            std::str::from_utf8(bytes).map_err(|_| RenderError::Other("not an image".into()))?;
        Ok(s.lines()
            .filter_map(|l| l.split_once(':'))
            .map(|(f, t)| DecodedCode {
                format: f.to_owned(),
                text: t.to_owned(),
            })
            .collect())
    }
}

#[allow(clippy::too_many_arguments)]
fn code(
    id: labely_core::ElementId,
    data: &str,
    format: &str,
    width_dots: f64,
    modules: f64,
    inside: bool,
    issues: &mut Vec<RenderIssue>,
    codes: &mut Vec<DecodedCode>,
) {
    if data.is_empty() {
        issues.push(RenderIssue::EncodeFailed {
            element_id: id,
            message: "empty data".into(),
        });
        return;
    }
    let dots = width_dots / modules;
    issues.push(RenderIssue::ModuleWidth {
        element_id: id,
        dots,
    });
    if inside && dots >= 2.0 {
        codes.push(DecodedCode {
            format: format.to_owned(),
            text: data.to_owned(),
        });
    }
}

/// Minimal 1-bit grayscale PNG (stored deflate blocks). Black dots are 0.
pub fn png_1bit(b: &Bitmap1) -> Vec<u8> {
    let mut raw = Vec::with_capacity((b.stride() + 1) * b.height() as usize);
    for row in b.data().chunks(b.stride().max(1)).take(b.height() as usize) {
        raw.push(0);
        raw.extend(row.iter().map(|x| !x));
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65_535).peekable();
    if chunks.peek().is_none() {
        z.extend([1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        let len = c.len() as u16;
        z.extend(len.to_le_bytes());
        z.extend((!len).to_le_bytes());
        z.extend(c);
    }
    z.extend(adler32(&raw).to_be_bytes());

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend(b.width().to_be_bytes());
    ihdr.extend(b.height().to_be_bytes());
    ihdr.extend([1, 0, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend(kind);
    out.extend(data);
    let crc = crc32(&out[start..]);
    out.extend(crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

/// The fake printer model: 203 dpi, 384-dot head (48 mm).
pub fn fake_model() -> ModelInfo {
    ModelInfo {
        id: FAKE_MODEL_ID.to_owned(),
        name: "Test Fake".to_owned(),
        dpi: 203,
        head_dots: 384,
        max_width_mm: 48.0,
        density_min: 1,
        density_max: 5,
        density_default: 3,
        media: vec![MediaType::Gap, MediaType::Continuous],
    }
}

#[derive(Debug, Default)]
struct HubState {
    connected: bool,
    prints: Vec<(String, PrintRequest)>,
    fail_next_print: Option<DeviceError>,
    paper: Option<PaperInfo>,
    /// While set, `print` blocks (until released or cancelled).
    hold: bool,
    /// Prints currently blocked by `hold`.
    held: usize,
    /// Media of each calibration, in order.
    calibrations: Vec<MediaType>,
    settings: DeviceSettings,
}

/// One fake printer at [`FAKE_ADDRESS`]; records every [`PrintRequest`].
#[derive(Debug, Default)]
pub struct FakeDeviceHub {
    state: Mutex<HubState>,
}

impl FakeDeviceHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Jobs printed so far, with their address.
    pub fn prints(&self) -> Vec<(String, PrintRequest)> {
        lock(&self.state).prints.clone()
    }

    /// Paper reported by `status` (RFID).
    pub fn set_paper(&self, paper: Option<PaperInfo>) {
        lock(&self.state).paper = paper;
    }

    /// Media of each `calibrate` call so far.
    pub fn calibrations(&self) -> Vec<MediaType> {
        lock(&self.state).calibrations.clone()
    }

    /// Make the next `print` fail with `err`.
    pub fn fail_next_print(&self, err: DeviceError) {
        lock(&self.state).fail_next_print = Some(err);
    }

    /// While `hold` is true, prints block after reporting their first page, so tests can
    /// watch the queue, pause it and cancel the running job.
    pub fn hold_prints(&self, hold: bool) {
        lock(&self.state).hold = hold;
    }

    /// Wait (up to 5 s) until a print is blocked by [`Self::hold_prints`].
    pub fn wait_held(&self) -> bool {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < until {
            if lock(&self.state).held > 0 {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        false
    }

    fn info(&self, connected: bool) -> DeviceInfo {
        DeviceInfo {
            address: FAKE_ADDRESS.to_owned(),
            name: "Fake Printer".to_owned(),
            connected,
            model: Some(fake_model()),
        }
    }

    fn require(&self, address: &str) -> Result<(), DeviceError> {
        if address != FAKE_ADDRESS {
            return Err(DeviceError::NotFound(address.to_owned()));
        }
        if !lock(&self.state).connected {
            return Err(DeviceError::Unavailable(format!(
                "{address} is not connected"
            )));
        }
        Ok(())
    }
}

impl DeviceHub for FakeDeviceHub {
    fn models(&self) -> Vec<ModelInfo> {
        vec![fake_model()]
    }

    fn scan(&self, _timeout_ms: u64) -> Result<Vec<DeviceInfo>, DeviceError> {
        let connected = lock(&self.state).connected;
        Ok(vec![self.info(connected)])
    }

    fn connected(&self) -> Vec<DeviceInfo> {
        if lock(&self.state).connected {
            vec![self.info(true)]
        } else {
            Vec::new()
        }
    }

    fn connect(&self, address: &str) -> Result<DeviceInfo, DeviceError> {
        if address != FAKE_ADDRESS {
            return Err(DeviceError::NotFound(address.to_owned()));
        }
        lock(&self.state).connected = true;
        Ok(self.info(true))
    }

    fn disconnect(&self, address: &str) -> Result<(), DeviceError> {
        self.require(address)?;
        lock(&self.state).connected = false;
        Ok(())
    }

    fn status(&self, address: &str) -> Result<DeviceStatus, DeviceError> {
        self.require(address)?;
        Ok(DeviceStatus {
            battery_percent: Some(100),
            lid_closed: Some(true),
            paper_present: Some(true),
            paper: lock(&self.state).paper.clone(),
        })
    }

    fn print(
        &self,
        address: &str,
        job: PrintRequest,
        control: &JobControl,
    ) -> Result<PrintReport, DeviceError> {
        self.require(address)?;
        let pages_printed = job.pages.len() as u32 * u32::from(job.copies);
        control.set_progress(0, pages_printed);
        if lock(&self.state).hold {
            control.set_progress(1.min(pages_printed), pages_printed);
            lock(&self.state).held += 1;
            while lock(&self.state).hold && !control.is_cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            lock(&self.state).held -= 1;
            if control.is_cancelled() {
                return Err(DeviceError::Cancelled);
            }
        }
        let mut st = lock(&self.state);
        if let Some(e) = st.fail_next_print.take() {
            return Err(e);
        }
        st.prints.push((address.to_owned(), job));
        control.set_progress(pages_printed, pages_printed);
        Ok(PrintReport {
            pages_printed,
            duration_ms: 0,
        })
    }

    fn calibrate(&self, address: &str, media: MediaType) -> Result<(), DeviceError> {
        self.require(address)?;
        lock(&self.state).calibrations.push(media);
        Ok(())
    }

    fn settings(&self, address: &str) -> Result<DeviceSettings, DeviceError> {
        self.require(address)?;
        Ok(lock(&self.state).settings.clone())
    }

    fn set_settings(
        &self,
        address: &str,
        patch: &DeviceSettings,
    ) -> Result<DeviceSettings, DeviceError> {
        self.require(address)?;
        let mut st = lock(&self.state);
        if patch.auto_off_level.is_some() {
            st.settings.auto_off_level = patch.auto_off_level;
        }
        if patch.sound_power.is_some() {
            st.settings.sound_power = patch.sound_power;
        }
        if patch.sound_bluetooth.is_some() {
            st.settings.sound_bluetooth = patch.sound_bluetooth;
        }
        Ok(st.settings.clone())
    }
}

/// A session plus handles to its fakes.
#[derive(Debug)]
pub struct TestKit {
    pub session: Session,
    pub renderer: Arc<FakeRenderer>,
    pub devices: Arc<FakeDeviceHub>,
}

/// Session wired with fresh fakes, plus handles to them.
pub fn test_kit() -> TestKit {
    let renderer = Arc::new(FakeRenderer::new());
    let devices = Arc::new(FakeDeviceHub::new());
    let services = Services {
        renderer: renderer.clone(),
        devices: devices.clone(),
    };
    TestKit {
        session: Session::new(services),
        renderer,
        devices,
    }
}

/// Session wired with fresh fakes.
pub fn test_session() -> Session {
    test_kit().session
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_header_and_crc() {
        let mut b = Bitmap1::new(10, 3);
        b.set(1, 1, true);
        let png = png_1bit(&b);
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn renderer_is_deterministic() {
        let mut d = Document::new("t", labely_core::LabelSpec::default());
        let id = d.alloc_id();
        d.pages[0].elements.push(labely_core::Element {
            id,
            name: None,
            x_mm: 1.0,
            y_mm: 1.0,
            width_mm: 10.0,
            height_mm: 5.0,
            rotation_deg: 0.0,
            locked: false,
            hidden: false,
            flip_h: false,
            flip_v: false,
            group: None,
            mirror: None,
            kind: ElementKind::Rect(Default::default()),
        });
        let r = FakeRenderer::new();
        let a = r.render(&d, &RenderOptions::at_dpi(203)).unwrap();
        let b = r.render(&d, &RenderOptions::at_dpi(203)).unwrap();
        assert_eq!(a.bitmap, b.bitmap);
        assert_eq!((a.bitmap.width(), a.bitmap.height()), (400, 240));
        assert_eq!(a.bitmap.count_black(), 80 * 40);
    }

    #[test]
    fn hub_records_prints() {
        let hub = FakeDeviceHub::new();
        assert!(hub.print(FAKE_ADDRESS, job(), &JobControl::new()).is_err());
        hub.connect(FAKE_ADDRESS).unwrap();
        let r = hub.print(FAKE_ADDRESS, job(), &JobControl::new()).unwrap();
        assert_eq!(r.pages_printed, 2);
        assert_eq!(hub.prints().len(), 1);
    }

    fn job() -> PrintRequest {
        PrintRequest {
            pages: vec![Bitmap1::new(8, 8)],
            copies: 2,
            density: 3,
            media: MediaType::Gap,
        }
    }
}
