//! USB CDC-ACM serial transport (protocol.md §3.2). Framing is identical to BLE.

use std::fmt;
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

use serialport::{DataBits, FlowControl, Parity, SerialPort, SerialPortType, StopBits};

use crate::{Discovered, Transport, TransportError, TransportInfo, TransportKind};

/// Default baud rate (OSS clients; docs/printers.md). CDC-ACM usually ignores it.
pub const DEFAULT_BAUD: u32 = 115_200;
/// Vendor baud rate, to try if the default does not answer (ADR-0010).
pub const ALT_BAUD: u32 = 230_400;

/// Bytes per write call. CDC-ACM has no meaningful packet limit; this just bounds writes.
const SERIAL_MTU: usize = 4096;
/// Per-read timeout of the OS port; `recv` loops over it until its own deadline.
const READ_SLICE: Duration = Duration::from_millis(20);

/// List USB serial ports. Never fails just because nothing is plugged in.
pub fn list() -> Result<Vec<Discovered>, TransportError> {
    let ports = serialport::available_ports().map_err(|e| TransportError::Io(e.into()))?;
    let mut out = Vec::new();
    for p in ports {
        let SerialPortType::UsbPort(usb) = p.port_type else {
            continue;
        };
        // macOS lists each device twice; the callout device (`/dev/cu.*`) is the one to open.
        if cfg!(target_os = "macos") && p.port_name.starts_with("/dev/tty.") {
            continue;
        }
        out.push(Discovered {
            info: TransportInfo {
                kind: TransportKind::Serial,
                name: usb_name(
                    usb.product.as_deref(),
                    usb.manufacturer.as_deref(),
                    usb.vid,
                    usb.pid,
                ),
                address: p.port_name,
            },
            rssi: None,
            services: Vec::new(),
        });
    }
    Ok(out)
}

fn clean(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// Display name for a USB port: `"<product> [vid:pid]"`, falling back to the manufacturer.
pub fn usb_name(product: Option<&str>, manufacturer: Option<&str>, vid: u16, pid: u16) -> String {
    let label = clean(product)
        .or(clean(manufacturer))
        .unwrap_or("USB serial");
    format!("{label} [{vid:04x}:{pid:04x}]")
}

/// Open `path` at `baud`, 8N1, no flow control.
pub fn open(path: &str, baud: u32) -> Result<SerialTransport, TransportError> {
    let port = serialport::new(path, baud)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .timeout(READ_SLICE)
        .open()
        .map_err(|e| match e.kind() {
            serialport::ErrorKind::NoDevice => TransportError::NotFound(path.to_owned()),
            _ => TransportError::Io(e.into()),
        })?;
    let info = TransportInfo {
        kind: TransportKind::Serial,
        address: path.to_owned(),
        name: path.rsplit('/').next().unwrap_or(path).to_owned(),
    };
    Ok(SerialTransport {
        info,
        port: Some(port),
    })
}

/// An open serial port.
pub struct SerialTransport {
    info: TransportInfo,
    port: Option<Box<dyn SerialPort>>,
}

impl fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SerialTransport")
            .field("info", &self.info)
            .field("open", &self.port.is_some())
            .finish()
    }
}

fn map_io(e: std::io::Error) -> TransportError {
    match e.kind() {
        ErrorKind::BrokenPipe | ErrorKind::NotConnected | ErrorKind::UnexpectedEof => {
            TransportError::Disconnected
        }
        _ => TransportError::Io(e),
    }
}

impl Transport for SerialTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let port = self.port.as_mut().ok_or(TransportError::Disconnected)?;
        for chunk in bytes.chunks(SERIAL_MTU) {
            port.write_all(chunk).map_err(map_io)?;
        }
        port.flush().map_err(map_io)
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        let port = self.port.as_mut().ok_or(TransportError::Disconnected)?;
        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; 4096];
        loop {
            match port.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => return Ok(Some(buf[..n].to_vec())),
                Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {}
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(map_io(e)),
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
        }
    }

    fn mtu(&self) -> usize {
        SERIAL_MTU
    }

    fn info(&self) -> &TransportInfo {
        &self.info
    }

    fn close(&mut self) {
        self.port = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_does_not_fail_without_devices() {
        let ports = list().unwrap();
        for p in &ports {
            assert_eq!(p.info.kind, TransportKind::Serial);
            assert!(p.info.name.contains('['));
        }
    }

    #[test]
    fn names() {
        assert_eq!(
            usb_name(Some("B21 Pro"), Some("NIIMBOT"), 0x3513, 2),
            "B21 Pro [3513:0002]"
        );
        assert_eq!(
            usb_name(Some(" "), Some("NIIMBOT"), 1, 2),
            "NIIMBOT [0001:0002]"
        );
        assert_eq!(
            usb_name(None, None, 0xabcd, 0xef01),
            "USB serial [abcd:ef01]"
        );
    }

    #[test]
    fn open_missing_port_errors() {
        assert!(open("/dev/labely-does-not-exist", DEFAULT_BAUD).is_err());
    }
}
