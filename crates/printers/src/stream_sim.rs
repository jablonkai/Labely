//! Simulator for stream printers (ESC/POS, Phomemo, Marklife, TSPL, ZPL): it keeps the bytes it receives,
//! answers status requests, and decodes the bytes back into pages for tests and `sim:`.

use std::sync::{Arc, Mutex};

use labely_core::Bitmap1;
use labely_protocol::{Raster, escpos, marklife, phomemo, tspl, zpl};
use labely_transport::{Transport, TransportError, TransportInfo, TransportKind};

use crate::registry::Model;
use crate::stream::Language;

/// What the simulated printer reports and how it behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSimState {
    pub paper_out: bool,
    pub lid_open: bool,
    /// Ignore status requests, as many low-cost printers do.
    pub silent: bool,
}

#[derive(Debug, Default)]
struct Shared {
    bytes: Vec<u8>,
    status_requests: u32,
    /// Replies waiting to be read by the host.
    replies: std::collections::VecDeque<Vec<u8>>,
}

/// Host side of a simulated stream printer. The printer handles each write as it is
/// made, so what a test reads afterwards is complete.
#[derive(Debug)]
pub struct StreamSimTransport {
    info: TransportInfo,
    language: Language,
    shared: Arc<Mutex<Shared>>,
    state: Arc<Mutex<StreamSimState>>,
    open: bool,
}

/// Handle to a running stream simulator.
#[derive(Debug)]
pub struct StreamSim {
    language: Language,
    shared: Arc<Mutex<Shared>>,
    state: Arc<Mutex<StreamSimState>>,
}

fn bitmap(raster: &Raster) -> Bitmap1 {
    let mut out = Bitmap1::new(raster.width(), raster.height());
    for y in 0..raster.height() {
        for x in 0..raster.width() {
            if raster.get(x, y) {
                out.set(x, y, true);
            }
        }
    }
    out
}

impl StreamSim {
    /// Start a simulator for `model`; returns it with the host-side transport. `None` if
    /// the model is not a stream printer.
    pub fn start(model: &Model) -> Option<(Self, StreamSimTransport)> {
        let language = Language::of(model)?;
        let shared = Arc::new(Mutex::new(Shared::default()));
        let state = Arc::new(Mutex::new(StreamSimState {
            paper_out: false,
            lid_open: false,
            silent: false,
        }));
        let host = StreamSimTransport {
            info: TransportInfo {
                kind: TransportKind::Memory,
                address: "sim".into(),
                name: model.name.clone(),
            },
            language,
            shared: Arc::clone(&shared),
            state: Arc::clone(&state),
            open: true,
        };
        Some((
            Self {
                language,
                shared,
                state,
            },
            host,
        ))
    }

    /// Change what the printer reports.
    pub fn set_state(&self, state: StreamSimState) {
        if let Ok(mut s) = self.state.lock() {
            *s = state;
        }
    }

    /// Every job byte received so far (status requests left out).
    pub fn bytes(&self) -> Vec<u8> {
        self.shared
            .lock()
            .map(|s| s.bytes.clone())
            .unwrap_or_default()
    }

    /// Status requests received so far.
    pub fn status_requests(&self) -> u32 {
        self.shared.lock().map(|s| s.status_requests).unwrap_or(0)
    }

    /// The pages the received bytes print, each with its number of copies. Printers that
    /// take no copy count get one entry per copy.
    pub fn pages(&self) -> Result<Vec<(Bitmap1, u32)>, String> {
        let bytes = self.bytes();
        match self.language {
            Language::EscPos { .. } => Ok(escpos::decode(&bytes)
                .map_err(|e| e.to_string())?
                .pages
                .iter()
                .map(|p| (bitmap(p), 1))
                .collect()),
            Language::Phomemo(_) => Ok(phomemo::decode(&bytes)
                .map_err(|e| e.to_string())?
                .pages
                .iter()
                .map(|p| (bitmap(p), 1))
                .collect()),
            Language::MarklifeL11 => Ok(marklife::decode(&bytes)
                .map_err(|e| e.to_string())?
                .pages
                .iter()
                .map(|p| (bitmap(p), 1))
                .collect()),
            Language::Tspl => Ok(tspl::decode(&bytes)
                .map_err(|e| e.to_string())?
                .iter()
                .map(|l| (bitmap(&l.raster), l.copies))
                .collect()),
            Language::Zpl => Ok(zpl::decode(&bytes)
                .map_err(|e| e.to_string())?
                .iter()
                .map(|l| (bitmap(&l.raster), l.copies))
                .collect()),
        }
    }
}

/// The reply to `chunk` if it is a status request of `language`.
fn status_reply(language: Language, chunk: &[u8], state: StreamSimState) -> Option<Vec<u8>> {
    let flag = |on: bool| if on { '1' } else { '0' };
    match language {
        Language::EscPos { .. } => {
            if chunk == escpos::status_request(escpos::STATUS_PAPER) {
                Some(vec![0x12 | if state.paper_out { 0x60 } else { 0 }])
            } else if chunk == escpos::status_request(escpos::STATUS_OFFLINE) {
                Some(vec![0x12 | if state.lid_open { 0x04 } else { 0 }])
            } else {
                None
            }
        }
        Language::Phomemo(_) => {
            if chunk == phomemo::PAPER_REQUEST {
                Some(vec![0x1A, 0x06, if state.paper_out { 0x88 } else { 0x89 }])
            } else if chunk == phomemo::COVER_REQUEST {
                Some(vec![0x1A, 0x05, if state.lid_open { 0x98 } else { 0x99 }])
            } else {
                None
            }
        }
        Language::MarklifeL11 => None,
        Language::Tspl => (chunk == tspl::STATUS_REQUEST)
            .then(|| vec![u8::from(state.lid_open) | if state.paper_out { 0x04 } else { 0 }]),
        Language::Zpl => (chunk == zpl::STATUS_REQUEST).then(|| {
            format!(
                "\u{2}030,{},0,1245,000,0,0,0,000,0,0,0\u{3}\r\n\
                 \u{2}000,0,{},0,1,2,6,0,00000000,1,000\u{3}\r\n\
                 \u{2}1234,0\u{3}\r\n",
                flag(state.paper_out),
                flag(state.lid_open)
            )
            .into_bytes()
        }),
    }
}

impl Transport for StreamSimTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if !self.open {
            return Err(TransportError::Disconnected);
        }
        let poisoned = || TransportError::Other("simulator lock poisoned".into());
        let now = *self.state.lock().map_err(|_| poisoned())?;
        let mut shared = self.shared.lock().map_err(|_| poisoned())?;
        // The driver sends a status request on its own, never inside a job.
        match status_reply(self.language, bytes, now) {
            Some(reply) => {
                shared.status_requests += 1;
                if !now.silent {
                    // A long reply arrives in pieces, as it does over Bluetooth.
                    shared.replies.extend(reply.chunks(20).map(<[u8]>::to_vec));
                }
            }
            None => {
                shared.bytes.extend_from_slice(bytes);
                // A Marklife printer reports each finished job.
                if self.language == Language::MarklifeL11
                    && !now.silent
                    && shared.bytes.ends_with(&marklife::STOP)
                {
                    shared.replies.push_back(vec![marklife::DONE]);
                }
            }
        }
        Ok(())
    }

    fn recv(&mut self, timeout: std::time::Duration) -> Result<Option<Vec<u8>>, TransportError> {
        if !self.open {
            return Err(TransportError::Disconnected);
        }
        let next = self
            .shared
            .lock()
            .map_err(|_| TransportError::Other("simulator lock poisoned".into()))?
            .replies
            .pop_front();
        if next.is_none() {
            // Nothing will arrive: the printer only answers a write.
            std::thread::sleep(timeout.min(std::time::Duration::from_millis(2)));
        }
        Ok(next)
    }

    fn mtu(&self) -> usize {
        512
    }

    fn info(&self) -> &TransportInfo {
        &self.info
    }

    fn close(&mut self) {
        self.open = false;
    }
}
