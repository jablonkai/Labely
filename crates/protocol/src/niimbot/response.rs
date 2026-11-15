//! Reply parsers (docs/protocol.md §2, §4, §9, §10, §13). All parsers are bounded and
//! return `None`/errors instead of panicking on short or malformed payloads.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::Packet;
use super::cmd;

/// Parse failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("unexpected reply 0x{got:02X}, expected 0x{expected:02X}")]
    UnexpectedCommand { expected: u8, got: u8 },
    #[error("reply 0x{cmd:02X} too short: {len} bytes")]
    TooShort { cmd: u8, len: usize },
    #[error("printer reported error: {0}")]
    Printer(PrinterError),
    #[error("printer does not support command")]
    NotSupported,
}

/// Turn generic error / not-supported replies into errors and check the reply code.
pub fn check(packet: &Packet, expected: u8) -> Result<&Packet, ParseError> {
    match packet.cmd {
        c if c == expected => Ok(packet),
        cmd::ERROR => Err(ParseError::Printer(PrinterError::from_code(
            packet.payload.first().copied().unwrap_or(0),
        ))),
        cmd::NOT_SUPPORTED => Err(ParseError::NotSupported),
        got => Err(ParseError::UnexpectedCommand { expected, got }),
    }
}

fn need(packet: &Packet, len: usize) -> Result<(), ParseError> {
    if packet.payload.len() < len {
        Err(ParseError::TooShort {
            cmd: packet.cmd,
            len: packet.payload.len(),
        })
    } else {
        Ok(())
    }
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}

/// Connect (0xC2) result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectResult {
    Disconnected,
    ConnectedLegacy,
    ConnectedV1,
    /// Connected; query 0xA5 for the protocol version.
    ConnectedQueryVersion,
    FirmwareError,
    Other(u8),
}

pub fn connect_result(p: &Packet) -> Result<ConnectResult, ParseError> {
    check(p, cmd::CONNECT_RESP)?;
    need(p, 1)?;
    Ok(match p.payload[0] {
        0 => ConnectResult::Disconnected,
        1 => ConnectResult::ConnectedLegacy,
        2 => ConnectResult::ConnectedV1,
        3 => ConnectResult::ConnectedQueryVersion,
        90 => ConnectResult::FirmwareError,
        n => ConnectResult::Other(n),
    })
}

/// Protocol generation derived at connect time (docs/protocol.md §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtocolVersion(pub u8);

/// Parse the 0xB5 status data reply. Returns the protocol version and colour support.
pub fn status_data(p: &Packet) -> Result<(ProtocolVersion, bool), ParseError> {
    check(p, cmd::STATUS_DATA_RESP)?;
    need(p, 13)?;
    let n = u16::from(p.payload[11]) * 100 + u16::from(p.payload[12]);
    let v = match n {
        0..=203 => 2,
        204..=299 => 3,
        300..=301 => 4,
        _ => 5,
    };
    Ok((ProtocolVersion(v), p.payload[10] != 0))
}

/// Model id from a PrinterInfo type 8 reply (0x48). One-byte replies mean `b << 8`.
pub fn model_id(p: &Packet) -> Result<u16, ParseError> {
    check(p, cmd::PRINTER_INFO + 8)?;
    match p.payload.as_slice() {
        [b] => Ok(u16::from(*b) << 8),
        [hi, lo, ..] => Ok(u16::from_be_bytes([*hi, *lo])),
        [] => Err(ParseError::TooShort { cmd: p.cmd, len: 0 }),
    }
}

/// Auto-off level from a PrinterInfo type 7 reply (0x47).
pub fn auto_off(p: &Packet) -> Result<u8, ParseError> {
    check(p, cmd::PRINTER_INFO + 7)?;
    need(p, 1)?;
    Ok(p.payload[0])
}

/// Sound setting reply (0x68): `[mode, kind, on]`.
pub fn sound(p: &Packet) -> Result<bool, ParseError> {
    check(p, cmd::SOUND_RESP)?;
    need(p, 3)?;
    Ok(p.payload[2] != 0)
}

/// Generic boolean setter / page reply.
pub fn ok(p: &Packet, expected: u8) -> Result<bool, ParseError> {
    check(p, expected)?;
    Ok(p.as_bool())
}

/// Print progress (0xB3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintStatus {
    pub page: u16,
    pub print_percent: u8,
    pub feed_percent: u8,
    pub error: Option<PrinterError>,
}

pub fn print_status(p: &Packet) -> Result<PrintStatus, ParseError> {
    check(p, cmd::PRINT_STATUS_RESP)?;
    need(p, 4)?;
    let b = &p.payload;
    let error = (b.len() >= 10 && b[6] != 0).then(|| PrinterError::from_code(b[6]));
    Ok(PrintStatus {
        page: u16_at(b, 0),
        print_percent: b[2],
        feed_percent: b[3],
        error,
    })
}

/// Device status from a heartbeat reply. Fields are `None` when the variant omits them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    pub lid_closed: Option<bool>,
    /// Battery in percent.
    pub battery_percent: Option<u8>,
    pub paper_present: Option<bool>,
    pub rfid_ok: Option<bool>,
}

fn battery(raw: u8) -> u8 {
    if raw <= 4 { raw * 25 } else { raw.min(100) }
}

/// Parse a heartbeat reply (0xDD advanced-1 or 0xD9 advanced-2).
///
/// `lid_inverted` flips the lid bit for models listed in docs/protocol.md §9.
pub fn heartbeat(p: &Packet, lid_inverted: bool) -> Result<Heartbeat, ParseError> {
    let b = &p.payload;
    let lid = |v: u8| Some((v == 0) != lid_inverted);
    let hb = match (p.cmd, b.len()) {
        (cmd::HEARTBEAT_ADV1, 10) => Heartbeat {
            lid_closed: lid(b[8]),
            battery_percent: Some(battery(b[9])),
            ..Default::default()
        },
        (cmd::HEARTBEAT_ADV1, 13) => Heartbeat {
            lid_closed: lid(b[9]),
            battery_percent: Some(battery(b[10])),
            paper_present: Some(b[11] == 0),
            rfid_ok: Some(b[12] != 0),
        },
        (cmd::HEARTBEAT_ADV1, 19) => Heartbeat {
            lid_closed: lid(b[15]),
            battery_percent: Some(battery(b[16])),
            paper_present: Some(b[17] == 0),
            rfid_ok: Some(b[18] != 0),
        },
        (cmd::HEARTBEAT_ADV1, 20) => Heartbeat {
            paper_present: Some(b[18] == 0),
            rfid_ok: Some(b[19] != 0),
            ..Default::default()
        },
        (cmd::HEARTBEAT_ADV2, n) if n >= 9 => Heartbeat {
            battery_percent: Some(battery(b[2])),
            lid_closed: lid(b[4]),
            paper_present: Some(b[5] == 0),
            rfid_ok: Some(b[6] != 0),
        },
        (cmd::HEARTBEAT_ADV1 | cmd::HEARTBEAT_ADV2, n) => {
            return Err(ParseError::TooShort { cmd: p.cmd, len: n });
        }
        (cmd::ERROR | cmd::NOT_SUPPORTED, _) => {
            check(p, cmd::HEARTBEAT_ADV1)?;
            Heartbeat::default()
        }
        (got, _) => {
            return Err(ParseError::UnexpectedCommand {
                expected: cmd::HEARTBEAT_ADV1,
                got,
            });
        }
    };
    Ok(hb)
}

/// Printer-info heartbeat (0xDF).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadInfo {
    pub head_dots: u16,
    /// 2 = 203 dpi, 3 = 300 dpi.
    pub resolution_class: u8,
    pub rfid_supported: bool,
}

pub fn head_info(p: &Packet) -> Result<HeadInfo, ParseError> {
    check(p, cmd::HEARTBEAT_INFO)?;
    need(p, 10)?;
    let b = &p.payload;
    Ok(HeadInfo {
        head_dots: u16_at(b, 4),
        resolution_class: b[6],
        rfid_supported: b[8] != 0,
    })
}

/// Label RFID data (0x1B), docs/protocol.md §10.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfidInfo {
    pub uuid: String,
    pub barcode: String,
    pub serial: String,
    pub total: u16,
    pub used: u16,
    pub label_type: u8,
}

/// Returns `Ok(None)` when no tag is present.
pub fn rfid_info(p: &Packet) -> Result<Option<RfidInfo>, ParseError> {
    check(p, cmd::RFID_INFO_RESP)?;
    let b = &p.payload;
    if b.len() <= 1 {
        return Ok(None);
    }
    let short = || ParseError::TooShort {
        cmd: p.cmd,
        len: b.len(),
    };
    let uuid: String = b
        .get(..8)
        .ok_or_else(short)?
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect();
    let mut i = 8;
    let mut lp_string = || -> Result<String, ParseError> {
        let len = *b.get(i).ok_or_else(short)? as usize;
        let s = b.get(i + 1..i + 1 + len).ok_or_else(short)?;
        i += 1 + len;
        Ok(String::from_utf8_lossy(s).into_owned())
    };
    let barcode = lp_string()?;
    let serial = lp_string()?;
    let rest = b.get(i..i + 5).ok_or_else(short)?;
    Ok(Some(RfidInfo {
        uuid,
        barcode,
        serial,
        total: u16_at(rest, 0),
        used: u16_at(rest, 2),
        label_type: rest[4],
    }))
}

/// Printer error codes (0xDB payload byte 0), docs/protocol.md §13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrinterError {
    CoverOpen,
    NoPaper,
    LowBattery,
    BatteryFault,
    UserCancel,
    DataError,
    Overheat,
    PaperOutFault,
    Busy,
    NoPrinthead,
    TemperatureLow,
    PrintheadLoose,
    NoRibbon,
    WrongRibbon,
    UsedRibbon,
    WrongPaper,
    SetPaperFailed,
    SetPrintModeFailed,
    SetDensityFailed,
    CommunicationError,
    Disconnected,
    IllegalPage,
    ReceiveTimeout,
    DensityNotSupported,
    Unknown(u8),
}

impl PrinterError {
    pub fn from_code(code: u8) -> Self {
        match code {
            0x01 => Self::CoverOpen,
            0x02 => Self::NoPaper,
            0x03 => Self::LowBattery,
            0x04 => Self::BatteryFault,
            0x05 => Self::UserCancel,
            0x06 => Self::DataError,
            0x07 => Self::Overheat,
            0x08 => Self::PaperOutFault,
            0x09 => Self::Busy,
            0x0A => Self::NoPrinthead,
            0x0B => Self::TemperatureLow,
            0x0C => Self::PrintheadLoose,
            0x0D => Self::NoRibbon,
            0x0E => Self::WrongRibbon,
            0x0F => Self::UsedRibbon,
            0x10 => Self::WrongPaper,
            0x11 => Self::SetPaperFailed,
            0x12 => Self::SetPrintModeFailed,
            0x13 => Self::SetDensityFailed,
            0x16 => Self::CommunicationError,
            0x17 => Self::Disconnected,
            0x1E => Self::DensityNotSupported,
            0x32 => Self::IllegalPage,
            0x34 => Self::ReceiveTimeout,
            n => Self::Unknown(n),
        }
    }

    /// Short English advice for users and agents.
    pub fn hint(self) -> &'static str {
        match self {
            Self::CoverOpen => "close the printer lid",
            Self::NoPaper | Self::PaperOutFault => "load labels",
            Self::LowBattery | Self::BatteryFault => "charge the printer",
            Self::Overheat => "let the printer cool down",
            Self::Busy => "wait for the current job to finish",
            Self::WrongPaper => "check the loaded label type matches the document",
            _ => "retry; if it persists, power-cycle the printer",
        }
    }
}

impl std::fmt::Display for PrinterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?} ({})", self.hint())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(cmd: u8, payload: &[u8]) -> Packet {
        Packet::small(cmd, payload)
    }

    #[test]
    fn settings_replies() {
        assert_eq!(auto_off(&pk(0x47, &[3])).unwrap(), 3);
        assert!(auto_off(&pk(0x47, &[])).is_err());
        assert!(sound(&pk(0x68, &[2, 2, 1])).unwrap());
        assert!(!sound(&pk(0x68, &[2, 1, 0])).unwrap());
        assert!(matches!(
            sound(&pk(cmd::NOT_SUPPORTED, &[])),
            Err(ParseError::NotSupported)
        ));
    }

    #[test]
    fn protocol_version_thresholds() {
        let mut b = [0u8; 13];
        b[11] = 3;
        b[12] = 0;
        assert_eq!(status_data(&pk(0xB5, &b)).unwrap().0, ProtocolVersion(4));
        b[12] = 2;
        assert_eq!(status_data(&pk(0xB5, &b)).unwrap().0, ProtocolVersion(5));
        b[11] = 2;
        b[12] = 50;
        assert_eq!(status_data(&pk(0xB5, &b)).unwrap().0, ProtocolVersion(3));
    }

    #[test]
    fn model_id_forms() {
        assert_eq!(model_id(&pk(0x48, &[0x03, 0x11])).unwrap(), 785);
        assert_eq!(model_id(&pk(0x48, &[0x02])).unwrap(), 512);
    }

    #[test]
    fn error_reply_maps() {
        let e = ok(&pk(cmd::ERROR, &[0x01]), cmd::SET_DENSITY_RESP).unwrap_err();
        assert_eq!(e, ParseError::Printer(PrinterError::CoverOpen));
    }

    #[test]
    fn rfid_parse() {
        let mut b = vec![1, 2, 3, 4, 5, 6, 7, 8];
        b.push(3);
        b.extend_from_slice(b"690");
        b.push(2);
        b.extend_from_slice(b"SN");
        b.extend_from_slice(&[0x00, 0xE6, 0x00, 0x05, 0x01]);
        let r = rfid_info(&pk(0x1B, &b)).unwrap().unwrap();
        assert_eq!(r.uuid, "0102030405060708");
        assert_eq!(r.barcode, "690");
        assert_eq!((r.total, r.used, r.label_type), (230, 5, 1));
        assert_eq!(rfid_info(&pk(0x1B, &[0])).unwrap(), None);
        assert!(rfid_info(&pk(0x1B, &b[..12])).is_err());
    }

    #[test]
    fn heartbeat_13() {
        let mut b = [0u8; 13];
        b[10] = 4;
        b[12] = 1;
        let hb = heartbeat(&pk(0xDD, &b), false).unwrap();
        assert_eq!(hb.lid_closed, Some(true));
        assert_eq!(hb.battery_percent, Some(100));
        assert_eq!(hb.paper_present, Some(true));
        assert_eq!(hb.rfid_ok, Some(true));
    }

    #[test]
    fn print_status_with_error() {
        let s = print_status(&pk(0xB3, &[0, 1, 100, 100, 0, 0, 2, 0, 0, 0])).unwrap();
        assert_eq!(s.page, 1);
        assert_eq!(s.error, Some(PrinterError::NoPaper));
    }
}
