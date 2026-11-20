//! In-process NIIMBOT printer simulator for tests and demos.
//!
//! It answers the handshake, status and RFID requests, reassembles 0x84/0x85 rows, the
//! two-colour 0x8A rows and the grayscale 0x8D rows into pages, and models the B21 Pro firmware quirks (no reply to the first packet after
//! PrintStart). [`SimConfig::for_model`] sets it up for any registry model, so every print
//! task runs against it. Behaviour follows docs/protocol.md; it is not a capture replay.

use std::sync::{Arc, Mutex};

use labely_core::{Bitmap1, Gray4};
use labely_protocol::niimbot::rows::{decode_color_row, decode_gray_row};
use labely_protocol::niimbot::{Decoder, Packet, cmd};
use labely_transport::memory::{MemoryPeer, MemoryTransport};

use crate::registry::Model;

/// What the simulator observed.
#[derive(Debug, Default, Clone)]
pub struct SimLog {
    /// Every request command received, in order.
    pub commands: Vec<u8>,
    pub density: Option<u8>,
    pub label_type: Option<u8>,
    pub print_start_len: Option<usize>,
    pub page_size_len: Option<usize>,
    /// Reassembled pages (as sent, head orientation) with their copy count. For a
    /// two-colour job this is the black plane; for a grayscale job it stays white.
    pub pages: Vec<(Bitmap1, u16)>,
    /// Colour field of the last PrintStart that has one (0 single, 1 two-colour, 3 gray).
    pub color_mode: Option<u8>,
    /// Red plane of every page of a two-colour job.
    pub red_pages: Vec<Bitmap1>,
    /// Every page of a grayscale job.
    pub gray_pages: Vec<Gray4>,
    /// Cut height and cut type of the last SetPageSize that has them.
    pub cut: Option<(u16, u8)>,
    /// The last half-cut request.
    pub half_cut: Option<bool>,
    /// Label type byte of each calibration request.
    pub calibrations: Vec<u8>,
}

/// Simulator settings.
#[derive(Debug, Clone)]
pub struct SimConfig {
    pub model_code: u16,
    /// `n` in B5 bytes 11–12 (`major*100 + minor`); 300 → protocol v4.
    pub version_code: u16,
    /// Drop the reply to the first packet after PrintStart (B21 Pro quirk).
    pub quirk_no_reply_after_start: bool,
    pub rfid: Option<(String, u16, u16, u8)>,
    /// Report this error code on PageEnd (simulate e.g. cover open = 1).
    pub fail_page_end: Option<u8>,
    /// Send an unsolicited PageIndex (0xE0) for every printed page (old D11).
    pub page_index_events: bool,
    /// Page width when SetPageSize carries no columns (old D11).
    pub head_dots: u16,
    /// Report a second colour in the status data (B5 byte 10).
    pub color: bool,
}

impl SimConfig {
    pub fn b21_pro() -> Self {
        Self {
            model_code: 785,
            version_code: 300,
            quirk_no_reply_after_start: true,
            rfid: Some(("6972842743589".into(), 230, 12, 1)),
            fail_page_end: None,
            page_index_events: false,
            head_dots: 591,
            color: false,
        }
    }

    /// A printer of `model`, with the firmware its print task needs. `auto` models report
    /// protocol 4.
    pub fn for_model(model: &Model) -> Self {
        let v4 = matches!(model.task.as_str(), "auto" | "d110m_v4");
        Self {
            model_code: model.model_codes.first().copied().unwrap_or(0),
            version_code: if v4 { 300 } else { 200 },
            quirk_no_reply_after_start: v4,
            rfid: None,
            fail_page_end: None,
            page_index_events: model.task == "d11_v1",
            head_dots: u16::try_from(model.head_dots).unwrap_or(u16::MAX),
            color: false,
        }
    }
}

/// Handle to a running simulator.
#[derive(Debug)]
pub struct Simulator {
    log: Arc<Mutex<SimLog>>,
}

impl Simulator {
    /// Start a simulator; returns it with the host-side transport.
    pub fn start(config: SimConfig) -> (Self, MemoryTransport) {
        let (host, peer) = MemoryTransport::pair("sim", 128);
        let log = Arc::new(Mutex::new(SimLog::default()));
        let log2 = Arc::clone(&log);
        // The thread exits when the host transport is dropped.
        std::thread::spawn(move || run(config, peer, log2));
        (Self { log }, host)
    }

    pub fn log(&self) -> SimLog {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

struct PageState {
    copies: u16,
    bitmap: Bitmap1,
    red: Bitmap1,
    gray: Gray4,
}

/// Set the dots of `data` (MSB first) in `rows` of `bitmap`.
fn put_rows(bitmap: &mut Bitmap1, rows: std::ops::Range<u32>, data: &[u8]) {
    for r in rows {
        for x in 0..bitmap.width() {
            let byte = data.get((x / 8) as usize).copied().unwrap_or(0);
            if byte & (0x80 >> (x % 8)) != 0 {
                bitmap.set(x, r, true);
            }
        }
    }
}

fn reply(peer: &MemoryPeer, cmd: u8, payload: &[u8]) -> bool {
    match Packet::new(cmd, payload.to_vec()) {
        Ok(p) => peer.tx.send(p.encode()).is_ok(),
        Err(_) => false,
    }
}

fn run(cfg: SimConfig, peer: MemoryPeer, log: Arc<Mutex<SimLog>>) {
    let mut dec = Decoder::for_requests();
    let mut page: Option<PageState> = None;
    let mut color_mode: u8 = 0;
    let mut printed: u16 = 0;
    let mut total: u16 = 0;
    let mut skip_next_reply = false;
    let mut auto_off: u8 = 2;
    // [bluetooth, power]
    let mut sound = [true, true];
    while let Ok(bytes) = peer.rx.recv() {
        // The decoder skips the Connect pre-byte while looking for the header.
        dec.push(&bytes);
        while let Some(Ok(p)) = dec.next_packet() {
            let Ok(mut l) = log.lock() else { return };
            l.commands.push(p.cmd);
            let b0 = p.payload.first().copied().unwrap_or(0);
            if skip_next_reply {
                skip_next_reply = false;
                if !matches!(
                    p.cmd,
                    cmd::ROWS_EMPTY | cmd::ROW_BITMAP | cmd::ROW_COLOR | cmd::ROW_GRAY
                ) {
                    continue;
                }
            }
            let ok = match p.cmd {
                cmd::CONNECT => reply(&peer, cmd::CONNECT_RESP, &[3]),
                cmd::STATUS_DATA => {
                    let mut b = [0u8; 13];
                    b[11] = (cfg.version_code / 100) as u8;
                    b[12] = (cfg.version_code % 100) as u8;
                    b[10] = u8::from(cfg.color);
                    reply(&peer, cmd::STATUS_DATA_RESP, &b)
                }
                cmd::PRINTER_INFO if b0 == 8 => reply(&peer, 0x48, &cfg.model_code.to_be_bytes()),
                cmd::PRINTER_INFO if b0 == 7 => reply(&peer, 0x47, &[auto_off]),
                // Versions and serial as a B21 Pro reports them (firmware 3.12, hardware 3.01).
                cmd::PRINTER_INFO if b0 == 9 => reply(&peer, 0x49, &[3, 12]),
                cmd::PRINTER_INFO if b0 == 12 => reply(&peer, 0x4C, &[3, 1]),
                cmd::PRINTER_INFO if b0 == 11 => reply(&peer, 0x4B, b"SIM0000001"),
                cmd::SET_AUTO_OFF => {
                    let valid = (1..=4).contains(&b0);
                    if valid {
                        auto_off = b0;
                    }
                    reply(&peer, cmd::SET_AUTO_OFF_RESP, &[u8::from(valid)])
                }
                cmd::CALIBRATE => {
                    l.calibrations.push(b0);
                    reply(&peer, cmd::CALIBRATE_RESP, &[1])
                }
                cmd::SOUND if p.payload.len() >= 3 => {
                    let slot = usize::from(p.payload[1] == 2);
                    if p.payload[0] == 1 {
                        sound[slot] = p.payload[2] != 0;
                    }
                    reply(
                        &peer,
                        cmd::SOUND_RESP,
                        &[p.payload[0], p.payload[1], u8::from(sound[slot])],
                    )
                }
                cmd::PRINTER_INFO => reply(&peer, cmd::PRINTER_INFO + b0, &[0]),
                cmd::HEARTBEAT => match b0 {
                    4 => reply(&peer, cmd::HEARTBEAT_ADV2, &[0, 0, 4, 30, 0, 0, 1, 0, 0]),
                    _ => {
                        let mut b = [0u8; 13];
                        b[10] = 4;
                        b[12] = 1;
                        reply(&peer, cmd::HEARTBEAT_ADV1, &b)
                    }
                },
                cmd::RFID_INFO => match &cfg.rfid {
                    None => reply(&peer, cmd::RFID_INFO_RESP, &[0]),
                    Some((barcode, total_n, used, kind)) => {
                        let mut b = vec![0xA1, 0xB2, 0xC3, 0xD4, 0, 0, 0, 1];
                        b.push(barcode.len() as u8);
                        b.extend_from_slice(barcode.as_bytes());
                        b.push(4);
                        b.extend_from_slice(b"SIM1");
                        b.extend_from_slice(&total_n.to_be_bytes());
                        b.extend_from_slice(&used.to_be_bytes());
                        b.push(*kind);
                        reply(&peer, cmd::RFID_INFO_RESP, &b)
                    }
                },
                cmd::SET_DENSITY => {
                    l.density = Some(b0);
                    reply(&peer, cmd::SET_DENSITY_RESP, &[1])
                }
                cmd::SET_LABEL_TYPE => {
                    l.label_type = Some(b0);
                    reply(&peer, cmd::SET_LABEL_TYPE_RESP, &[1])
                }
                cmd::PRINT_START => {
                    l.print_start_len = Some(p.payload.len());
                    total = if p.payload.len() >= 2 {
                        u16::from_be_bytes([p.payload[0], p.payload[1]])
                    } else {
                        0
                    };
                    printed = 0;
                    color_mode = p.payload.get(6).copied().unwrap_or(0);
                    if p.payload.len() >= 7 {
                        l.color_mode = Some(color_mode);
                    }
                    skip_next_reply = cfg.quirk_no_reply_after_start;
                    reply(&peer, cmd::PRINT_START_RESP, &[1])
                }
                cmd::PAGE_START | cmd::PRINT_CLEAR | cmd::SET_QUANTITY => {
                    if p.cmd == cmd::SET_QUANTITY
                        && let (Some(pg), [hi, lo, ..]) = (page.as_mut(), p.payload.as_slice())
                    {
                        pg.copies = u16::from_be_bytes([*hi, *lo]);
                    }
                    let resp = match p.cmd {
                        cmd::PAGE_START => cmd::PAGE_START_RESP,
                        cmd::PRINT_CLEAR => cmd::PRINT_CLEAR_RESP,
                        _ => cmd::SET_QUANTITY_RESP,
                    };
                    reply(&peer, resp, &[1])
                }
                cmd::SET_PAGE_SIZE => {
                    l.page_size_len = Some(p.payload.len());
                    let g = |i: usize| {
                        p.payload
                            .get(i..i + 2)
                            .map_or(0, |s| u16::from_be_bytes([s[0], s[1]]))
                    };
                    let rows = g(0);
                    let cols = if p.payload.len() >= 4 {
                        g(2)
                    } else {
                        cfg.head_dots
                    };
                    let copies = if p.payload.len() >= 6 { g(4) } else { 1 };
                    if p.payload.len() >= 9 {
                        l.cut = Some((g(6), p.payload[8]));
                    }
                    let (w, h) = (u32::from(cols), u32::from(rows));
                    page = Some(PageState {
                        copies,
                        bitmap: Bitmap1::new(w, h),
                        red: Bitmap1::new(w, h),
                        gray: Gray4::new(w, h),
                    });
                    reply(&peer, cmd::SET_PAGE_SIZE_RESP, &[1])
                }
                cmd::ROWS_EMPTY => true,
                cmd::CHECK_LINE => reply(&peer, cmd::CHECK_LINE_RESP, &[1]),
                cmd::ROW_BITMAP => {
                    if let Some(pg) = page.as_mut()
                        && p.payload.len() >= 6
                    {
                        let row = u32::from(u16::from_be_bytes([p.payload[0], p.payload[1]]));
                        let repeat = u32::from(p.payload[5]);
                        put_rows(&mut pg.bitmap, row..row + repeat, &p.payload[6..]);
                    }
                    true
                }
                cmd::ROW_COLOR => {
                    if let Some(pg) = page.as_mut()
                        && let Some(row) =
                            decode_color_row(&p.payload, (pg.bitmap.width() as usize).div_ceil(8))
                    {
                        let rows = u32::from(row.row)..u32::from(row.row) + u32::from(row.repeat);
                        put_rows(&mut pg.bitmap, rows.clone(), &row.black);
                        put_rows(&mut pg.red, rows, &row.red);
                    }
                    true
                }
                cmd::ROW_GRAY => {
                    if let Some(pg) = page.as_mut()
                        && let Some(row) = decode_gray_row(&p.payload)
                    {
                        for r in u32::from(row.row)..u32::from(row.row) + u32::from(row.repeat) {
                            for (x, level) in row.levels.iter().enumerate() {
                                pg.gray.set(x as u32, r, *level);
                            }
                        }
                    }
                    true
                }
                cmd::HALF_CUT if p.payload.len() >= 2 => {
                    l.half_cut = Some(p.payload[1] != 0);
                    reply(&peer, cmd::HALF_CUT_RESP, &[1])
                }
                cmd::PAGE_END => {
                    if let Some(code) = cfg.fail_page_end {
                        reply(&peer, cmd::ERROR, &[code])
                    } else {
                        let before = printed;
                        if let Some(pg) = page.take() {
                            printed = printed.saturating_add(pg.copies);
                            l.pages.push((pg.bitmap, pg.copies));
                            match color_mode {
                                1 => l.red_pages.push(pg.red),
                                3 => l.gray_pages.push(pg.gray),
                                _ => {}
                            }
                        }
                        let mut ok = reply(&peer, cmd::PAGE_END_RESP, &[1]);
                        if cfg.page_index_events {
                            for n in before + 1..=printed {
                                ok = ok && reply(&peer, cmd::PAGE_INDEX, &n.to_be_bytes());
                            }
                        }
                        ok
                    }
                }
                cmd::PRINT_STATUS => {
                    let page_no = printed.min(total.max(printed));
                    let [hi, lo] = page_no.to_be_bytes();
                    reply(&peer, cmd::PRINT_STATUS_RESP, &[hi, lo, 100, 100])
                }
                cmd::PRINT_END => reply(&peer, cmd::PRINT_END_RESP, &[1]),
                cmd::CANCEL => reply(&peer, cmd::CANCEL_RESP, &[1]),
                _ => reply(&peer, cmd::NOT_SUPPORTED, &[]),
            };
            if !ok {
                return;
            }
        }
    }
}

/// A [`NativeBleHost`] backed by simulators: it finds one printer and runs a fresh simulator
/// per connection. It exercises the mobile BLE path (ADR-0025) without hardware.
///
/// [`NativeBleHost`]: labely_transport::callback::NativeBleHost
#[derive(Debug)]
pub struct SimBleHost {
    config: SimConfig,
    address: String,
    name: String,
    mtu: u32,
    last: Arc<Mutex<Option<Simulator>>>,
}

impl SimBleHost {
    /// One simulated B21 Pro at `SIM-B21`, advertised as `B21_Pro-SIM`, with a 20-byte MTU
    /// (the BLE minimum, so chunking is exercised).
    pub fn b21_pro() -> Self {
        Self {
            config: SimConfig::b21_pro(),
            address: "SIM-B21".into(),
            name: "B21_Pro-SIM".into(),
            mtu: 20,
            last: Arc::default(),
        }
    }

    /// What the most recently connected simulator observed.
    pub fn log(&self) -> Option<SimLog> {
        self.last.lock().ok()?.as_ref().map(Simulator::log)
    }
}

struct SimLink {
    host: Arc<Mutex<MemoryTransport>>,
    open: Arc<std::sync::atomic::AtomicBool>,
    mtu: u32,
}

impl labely_transport::callback::NativeLink for SimLink {
    fn write(&self, bytes: Vec<u8>) -> Result<(), String> {
        use labely_transport::Transport;
        let mut t = self
            .host
            .lock()
            .map_err(|_| "sim lock poisoned".to_string())?;
        t.send(&bytes).map_err(|e| e.to_string())
    }

    fn mtu(&self) -> u32 {
        self.mtu
    }

    fn close(&self) {
        self.open.store(false, std::sync::atomic::Ordering::Release);
    }
}

impl labely_transport::callback::NativeBleHost for SimBleHost {
    fn scan(
        &self,
        _timeout: std::time::Duration,
        name_prefixes: &[String],
    ) -> Result<Vec<labely_transport::Discovered>, labely_transport::TransportError> {
        let matches =
            name_prefixes.is_empty() || name_prefixes.iter().any(|p| self.name.starts_with(p));
        Ok(matches
            .then(|| labely_transport::Discovered {
                info: labely_transport::TransportInfo {
                    kind: labely_transport::TransportKind::Ble,
                    address: self.address.clone(),
                    name: self.name.clone(),
                },
                rssi: Some(-50),
                services: Vec::new(),
            })
            .into_iter()
            .collect())
    }

    fn connect(
        &self,
        address: &str,
        sink: labely_transport::callback::NotifySink,
    ) -> Result<
        (Arc<dyn labely_transport::callback::NativeLink>, String),
        labely_transport::TransportError,
    > {
        use labely_transport::Transport;
        use std::sync::atomic::{AtomicBool, Ordering};
        if address != self.address {
            return Err(labely_transport::TransportError::NotFound(address.into()));
        }
        let (sim, transport) = Simulator::start(self.config.clone());
        let host = Arc::new(Mutex::new(transport));
        let open = Arc::new(AtomicBool::new(true));
        // Pump simulator replies into the sink, as GATT notifications would arrive.
        {
            let (host, open) = (Arc::clone(&host), Arc::clone(&open));
            std::thread::spawn(move || {
                while open.load(Ordering::Acquire) {
                    let got = match host.lock() {
                        Ok(mut t) => t.recv(std::time::Duration::from_millis(5)),
                        Err(_) => break,
                    };
                    match got {
                        Ok(Some(bytes)) => sink.push(bytes),
                        Ok(None) => std::thread::yield_now(),
                        Err(_) => break,
                    }
                }
                sink.disconnected();
            });
        }
        if let Ok(mut last) = self.last.lock() {
            *last = Some(sim);
        }
        let link = SimLink {
            host,
            open,
            mtu: self.mtu,
        };
        Ok((Arc::new(link), self.name.clone()))
    }
}
