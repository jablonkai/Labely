//! Byte transports (ADR-0006). A transport is a plain byte pipe with no protocol
//! knowledge; framing and request/reply pairing live in the drivers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "ble")]
pub mod ble;
pub mod callback;
pub mod gatt;
pub mod memory;
#[cfg(feature = "serial")]
pub mod serial;

/// Transport kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    Ble,
    Serial,
    Memory,
    Callback,
}

/// Identity of an open or discovered endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportInfo {
    pub kind: TransportKind,
    /// Address without scheme: BLE peripheral id, serial port path, memory name.
    pub address: String,
    /// Human-readable name (BLE advertised name, USB product string).
    pub name: String,
}

impl TransportInfo {
    /// `ble:<id>`, `usb:<port>`, `mem:<name>`, `cb:<id>`.
    pub fn uri(&self) -> String {
        let scheme = match self.kind {
            TransportKind::Ble => "ble",
            TransportKind::Serial => "usb",
            TransportKind::Memory => "mem",
            TransportKind::Callback => "cb",
        };
        format!("{scheme}:{}", self.address)
    }
}

/// An endpoint found by a scan or port listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discovered {
    pub info: TransportInfo,
    /// Signal strength in dBm (BLE only, when the platform reports it).
    pub rssi: Option<i16>,
    /// Advertised service UUIDs, lowercase hyphenated (BLE only).
    pub services: Vec<String>,
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("not connected")]
    Disconnected,
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("bluetooth: {0}")]
    Bluetooth(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

/// A connected, bidirectional byte pipe.
pub trait Transport: Send {
    /// Write bytes. Implementations split into chunks of at most [`Self::mtu`] bytes.
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    /// Wait up to `timeout` for received bytes. `Ok(None)` on timeout. One call may
    /// return part of a packet or several packets.
    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError>;
    /// Maximum bytes per underlying write.
    fn mtu(&self) -> usize;
    /// Recommended pause between consecutive packets.
    fn packet_gap(&self) -> Duration {
        Duration::ZERO
    }
    fn info(&self) -> &TransportInfo;
    fn close(&mut self);
}

/// Slice length used by [`recv_while_open`] to notice a dropped link while waiting.
const RECV_POLL_SLICE: Duration = Duration::from_millis(25);

/// Wait up to `timeout` for one message on `rx`, returning `Disconnected` as soon as
/// `open` turns false. Shared by channel-backed transports (BLE, callback).
pub(crate) fn recv_while_open(
    rx: &Receiver<Vec<u8>>,
    open: &AtomicBool,
    timeout: Duration,
) -> Result<Option<Vec<u8>>, TransportError> {
    let deadline = Instant::now() + timeout;
    loop {
        if !open.load(Ordering::Acquire) {
            return Err(TransportError::Disconnected);
        }
        let now = Instant::now();
        if now >= deadline {
            // Still give already-queued data a chance on a zero timeout.
            return match rx.try_recv() {
                Ok(b) => Ok(Some(b)),
                Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Err(TransportError::Disconnected)
                }
            };
        }
        match rx.recv_timeout((deadline - now).min(RECV_POLL_SLICE)) {
            Ok(b) => return Ok(Some(b)),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(TransportError::Disconnected),
        }
    }
}
