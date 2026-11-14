//! In-process NIIMBOT printer simulator for tests and demos.
//!
//! It answers the handshake, status and RFID requests, reassembles 0x84/0x85 rows into
//! pages, and models the B21 Pro firmware quirks (no reply to the first packet after
//! PrintStart). Behaviour follows docs/protocol.md; it is not a capture replay.

use std::sync::{Arc, Mutex};

use labely_core::Bitmap1;
use labely_protocol::niimbot::{Decoder, Packet, cmd};
use labely_transport::memory::{MemoryPeer, MemoryTransport};

/// What the simulator observed.
#[derive(Debug, Default, Clone)]
pub struct SimLog {
    /// Every request command received, in order.
    pub commands: Vec<u8>,
    pub density: Option<u8>,
    pub label_type: Option<u8>,
    pub print_start_len: Option<usize>,
    pub page_size_len: Option<usize>,
    /// Reassembled pages (as sent, head orientation) with their copy count.
    pub pages: Vec<(Bitmap1, u16)>,
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
}

impl SimConfig {
    pub fn b21_pro() -> Self {
        Self {
            model_code: 785,
            version_code: 300,
            quirk_no_reply_after_start: true,
            rfid: Some(("6972842743589".into(), 230, 12, 1)),
            fail_page_end: None,
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
    cols: u16,
    copies: u16,
    bitmap: Bitmap1,
}

fn reply(peer: &MemoryPeer, cmd: u8, payload: &[u8]) -> bool {
    match Packet::new(cmd, payload.to_vec()) {
        Ok(p) => peer.tx.send(p.encode()).is_ok(),
        Err(_) => false,
    }
}

fn run(cfg: SimConfig, peer: MemoryPeer, log: Arc<Mutex<SimLog>>) {
    let mut dec = Decoder::new();
    let mut page: Option<PageState> = None;
    let mut printed: u16 = 0;
    let mut total: u16 = 0;
    let mut skip_next_reply = false;
    while let Ok(bytes) = peer.rx.recv() {
        // The decoder skips the Connect pre-byte while looking for the header.
        dec.push(&bytes);
        while let Some(Ok(p)) = dec.next_packet() {
            let Ok(mut l) = log.lock() else { return };
            l.commands.push(p.cmd);
            let b0 = p.payload.first().copied().unwrap_or(0);
            if skip_next_reply {
                skip_next_reply = false;
                if !matches!(p.cmd, cmd::ROWS_EMPTY | cmd::ROW_BITMAP) {
                    continue;
                }
            }
            let ok = match p.cmd {
                cmd::CONNECT => reply(&peer, cmd::CONNECT_RESP, &[3]),
                cmd::STATUS_DATA => {
                    let mut b = [0u8; 13];
                    b[11] = (cfg.version_code / 100) as u8;
                    b[12] = (cfg.version_code % 100) as u8;
                    reply(&peer, cmd::STATUS_DATA_RESP, &b)
                }
                cmd::PRINTER_INFO if b0 == 8 => reply(&peer, 0x48, &cfg.model_code.to_be_bytes()),
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
                    skip_next_reply = cfg.quirk_no_reply_after_start;
                    reply(&peer, cmd::PRINT_START_RESP, &[1])
                }
                cmd::PAGE_START | cmd::PRINT_CLEAR | cmd::SET_QUANTITY => {
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
                    let (rows, cols) = (g(0), g(2));
                    let copies = if p.payload.len() >= 6 { g(4) } else { 1 };
                    page = Some(PageState {
                        cols,
                        copies,
                        bitmap: Bitmap1::new(u32::from(cols), u32::from(rows)),
                    });
                    reply(&peer, cmd::SET_PAGE_SIZE_RESP, &[1])
                }
                cmd::ROWS_EMPTY => true,
                cmd::ROW_BITMAP => {
                    if let Some(pg) = page.as_mut()
                        && p.payload.len() >= 6
                    {
                        let row = u32::from(u16::from_be_bytes([p.payload[0], p.payload[1]]));
                        let repeat = u32::from(p.payload[5]);
                        let data = &p.payload[6..];
                        for r in row..row + repeat {
                            for x in 0..u32::from(pg.cols) {
                                let byte = data.get((x / 8) as usize).copied().unwrap_or(0);
                                if byte & (0x80 >> (x % 8)) != 0 {
                                    pg.bitmap.set(x, r, true);
                                }
                            }
                        }
                    }
                    true
                }
                cmd::PAGE_END => {
                    if let Some(code) = cfg.fail_page_end {
                        reply(&peer, cmd::ERROR, &[code])
                    } else {
                        if let Some(pg) = page.take() {
                            printed = printed.saturating_add(pg.copies);
                            l.pages.push((pg.bitmap, pg.copies));
                        }
                        reply(&peer, cmd::PAGE_END_RESP, &[1])
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
