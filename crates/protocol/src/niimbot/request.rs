//! Request builders (docs/protocol.md §2, §5, §8).

use serde::{Deserialize, Serialize};

use super::Packet;
use super::cmd;

/// PrinterInfo (0x40) sub-types. The reply command is `0x40 + type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum InfoType {
    Density = 1,
    Speed = 2,
    LabelType = 3,
    Language = 6,
    AutoOff = 7,
    ModelId = 8,
    Firmware = 9,
    Battery = 10,
    Serial = 11,
    Hardware = 12,
    BtMac = 13,
    PrintMode = 14,
    Area = 15,
}

impl InfoType {
    /// Reply command for this info type.
    pub fn response_cmd(self) -> u8 {
        cmd::PRINTER_INFO + self as u8
    }
}

/// Heartbeat (0xDC) request type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum HeartbeatType {
    /// Reply 0xDD.
    Advanced1 = 1,
    /// Reply 0xDE.
    Basic = 2,
    /// Reply 0xDF: head width, resolution, RFID support.
    PrinterInfo = 3,
    /// Reply 0xD9.
    Advanced2 = 4,
}

impl HeartbeatType {
    pub fn response_cmd(self) -> u8 {
        match self {
            Self::Advanced1 => cmd::HEARTBEAT_ADV1,
            Self::Basic => cmd::HEARTBEAT_BASIC,
            Self::PrinterInfo => cmd::HEARTBEAT_INFO,
            Self::Advanced2 => cmd::HEARTBEAT_ADV2,
        }
    }
}

/// Label (media) type for SetLabelType (0x23), docs/protocol.md §8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum LabelType {
    Gap = 1,
    BlackMark = 2,
    Continuous = 3,
    Perforated = 4,
    Transparent = 5,
    PvcTag = 6,
    BlackMarkGap = 10,
    HeatShrinkTube = 11,
}

impl LabelType {
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => Self::Gap,
            2 => Self::BlackMark,
            3 => Self::Continuous,
            4 => Self::Perforated,
            5 => Self::Transparent,
            6 => Self::PvcTag,
            10 => Self::BlackMarkGap,
            11 => Self::HeatShrinkTube,
            _ => return None,
        })
    }
}

/// Colour field of the multi-byte PrintStart payloads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ColorMode {
    #[default]
    Single = 0,
    TwoColor = 1,
    Alt = 2,
    Gray = 3,
}

pub fn connect() -> Packet {
    Packet::small(cmd::CONNECT, &[0x01])
}

pub fn status_data() -> Packet {
    Packet::small(cmd::STATUS_DATA, &[0x01])
}

pub fn printer_info(kind: InfoType) -> Packet {
    Packet::small(cmd::PRINTER_INFO, &[kind as u8])
}

pub fn capabilities() -> Packet {
    Packet::small(cmd::CAPABILITIES, &[0x01])
}

pub fn heartbeat(kind: HeartbeatType) -> Packet {
    Packet::small(cmd::HEARTBEAT, &[kind as u8])
}

pub fn rfid_info() -> Packet {
    Packet::small(cmd::RFID_INFO, &[0x01])
}

pub fn paper_info() -> Packet {
    Packet::small(cmd::PAPER_INFO, &[0x01])
}

pub fn set_density(density: u8) -> Packet {
    Packet::small(cmd::SET_DENSITY, &[density])
}

pub fn set_label_type(kind: LabelType) -> Packet {
    Packet::small(cmd::SET_LABEL_TYPE, &[kind as u8])
}

/// Auto-off levels accepted by SetAutoOff (0x27), docs/protocol.md §2.
pub const AUTO_OFF_LEVELS: std::ops::RangeInclusive<u8> = 1..=4;

/// SetAutoOff (0x27): `level` 1–4 (see [`AUTO_OFF_LEVELS`]). Read back with PrinterInfo
/// type 7.
pub fn set_auto_off(level: u8) -> Packet {
    Packet::small(cmd::SET_AUTO_OFF, &[level])
}

/// Label positioning calibration (0x8E): the printer feeds labels until its gap or
/// black-mark sensor finds the label edge. The payload is the label type (§8).
pub fn calibrate(kind: LabelType) -> Packet {
    Packet::small(cmd::CALIBRATE, &[kind as u8])
}

/// Which beep a sound setting (0x58) controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum SoundKind {
    /// Beep when a Bluetooth host connects.
    Bluetooth = 1,
    /// Beep on power on and off.
    Power = 2,
}

/// Read a sound setting (0x58 `[2, kind, 0]`).
pub fn sound_get(kind: SoundKind) -> Packet {
    Packet::small(cmd::SOUND, &[2, kind as u8, 0])
}

/// Turn a sound on or off (0x58 `[1, kind, on]`).
pub fn sound_set(kind: SoundKind, on: bool) -> Packet {
    Packet::small(cmd::SOUND, &[1, kind as u8, u8::from(on)])
}

/// PrintStart payload shapes (docs/protocol.md §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintStart {
    /// 1 byte `01` (D11 / B21 V1 / D110 tasks).
    V1,
    /// 2 bytes: total u16 (H1S task, vendor V3).
    V2 { total_pages: u16 },
    /// 7 bytes: total u16, four zero bytes, colour (B1 task, vendor V4).
    V7 { total_pages: u16, color: ColorMode },
    /// 9 bytes: as V7 plus quality/speed and flag (D110M_V4 task, vendor V6; B21 Pro).
    V9 {
        total_pages: u16,
        color: ColorMode,
        quality: u8,
        flag: u8,
    },
}

pub fn print_start(start: PrintStart) -> Packet {
    match start {
        PrintStart::V1 => Packet::small(cmd::PRINT_START, &[0x01]),
        PrintStart::V2 { total_pages } => {
            Packet::small(cmd::PRINT_START, &total_pages.to_be_bytes())
        }
        PrintStart::V7 { total_pages, color } => {
            let [hi, lo] = total_pages.to_be_bytes();
            Packet::small(cmd::PRINT_START, &[hi, lo, 0, 0, 0, 0, color as u8])
        }
        PrintStart::V9 {
            total_pages,
            color,
            quality,
            flag,
        } => {
            let [hi, lo] = total_pages.to_be_bytes();
            Packet::small(
                cmd::PRINT_START,
                &[hi, lo, 0, 0, 0, 0, color as u8, quality, flag],
            )
        }
    }
}

pub fn page_start() -> Packet {
    Packet::small(cmd::PAGE_START, &[0x01])
}

/// SetPageSize payload shapes (docs/protocol.md §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSizeFormat {
    /// rows
    Len2,
    /// rows, cols
    Len4,
    /// rows, cols, copies
    Len6,
    /// rows, cols, copies, cut height u16 and cut type u8, both zero (H1S)
    Len9,
    /// rows, cols, copies, then 7 zero bytes (B21 Pro per OSS)
    Len13,
}

/// Page size: `rows` along the feed, `cols` across the head (dots).
pub fn set_page_size(format: PageSizeFormat, rows: u16, cols: u16, copies: u16) -> Packet {
    let mut p = Vec::with_capacity(13);
    p.extend_from_slice(&rows.to_be_bytes());
    if format != PageSizeFormat::Len2 {
        p.extend_from_slice(&cols.to_be_bytes());
    }
    if matches!(
        format,
        PageSizeFormat::Len6 | PageSizeFormat::Len9 | PageSizeFormat::Len13
    ) {
        p.extend_from_slice(&copies.to_be_bytes());
    }
    if format == PageSizeFormat::Len9 {
        p.extend_from_slice(&[0; 3]);
    }
    if format == PageSizeFormat::Len13 {
        p.extend_from_slice(&[0; 7]);
    }
    Packet::small(cmd::SET_PAGE_SIZE, &p)
}

pub fn set_quantity(copies: u16) -> Packet {
    Packet::small(cmd::SET_QUANTITY, &copies.to_be_bytes())
}

pub fn print_clear() -> Packet {
    Packet::small(cmd::PRINT_CLEAR, &[0x01])
}

/// Row check-line after `row` (§7): `[row u16][01]`, answered with 0xD3.
pub fn check_line(row: u16) -> Packet {
    let [hi, lo] = row.to_be_bytes();
    Packet::small(cmd::CHECK_LINE, &[hi, lo, 0x01])
}

pub fn page_end() -> Packet {
    Packet::small(cmd::PAGE_END, &[0x01])
}

pub fn print_status() -> Packet {
    Packet::small(cmd::PRINT_STATUS, &[0x01])
}

pub fn print_end() -> Packet {
    Packet::small(cmd::PRINT_END, &[0x01])
}

pub fn cancel() -> Packet {
    Packet::small(cmd::CANCEL, &[0x01])
}

/// Expected reply command for a request, if the printer answers it.
pub fn expected_response(request_cmd: u8, first_payload_byte: Option<u8>) -> Option<u8> {
    Some(match request_cmd {
        cmd::CONNECT => cmd::CONNECT_RESP,
        cmd::STATUS_DATA => cmd::STATUS_DATA_RESP,
        cmd::PRINTER_INFO => cmd::PRINTER_INFO + first_payload_byte?,
        cmd::CAPABILITIES => cmd::CAPABILITIES_RESP,
        cmd::HEARTBEAT => match first_payload_byte? {
            1 => cmd::HEARTBEAT_ADV1,
            2 => cmd::HEARTBEAT_BASIC,
            3 => cmd::HEARTBEAT_INFO,
            4 => cmd::HEARTBEAT_ADV2,
            _ => return None,
        },
        cmd::RFID_INFO => cmd::RFID_INFO_RESP,
        cmd::PAPER_INFO => cmd::PAPER_INFO_RESP,
        cmd::SET_DENSITY => cmd::SET_DENSITY_RESP,
        cmd::SET_LABEL_TYPE => cmd::SET_LABEL_TYPE_RESP,
        cmd::SET_AUTO_OFF => cmd::SET_AUTO_OFF_RESP,
        cmd::CALIBRATE => cmd::CALIBRATE_RESP,
        cmd::SOUND => cmd::SOUND_RESP,
        cmd::PRINT_START => cmd::PRINT_START_RESP,
        cmd::PAGE_START => cmd::PAGE_START_RESP,
        cmd::SET_PAGE_SIZE => cmd::SET_PAGE_SIZE_RESP,
        cmd::SET_QUANTITY => cmd::SET_QUANTITY_RESP,
        cmd::PRINT_CLEAR => cmd::PRINT_CLEAR_RESP,
        cmd::CHECK_LINE => cmd::CHECK_LINE_RESP,
        cmd::PAGE_END => cmd::PAGE_END_RESP,
        cmd::PRINT_STATUS => cmd::PRINT_STATUS_RESP,
        cmd::PRINT_END => cmd::PRINT_END_RESP,
        cmd::CANCEL => cmd::CANCEL_RESP,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_start_v9_layout() {
        let p = print_start(PrintStart::V9 {
            total_pages: 2,
            color: ColorMode::Single,
            quality: 0,
            flag: 0,
        });
        assert_eq!(p.payload, vec![0, 2, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn page_size_13() {
        let p = set_page_size(PageSizeFormat::Len13, 354, 584, 3);
        assert_eq!(p.payload.len(), 13);
        assert_eq!(&p.payload[..6], &[0x01, 0x62, 0x02, 0x48, 0x00, 0x03]);
        assert!(p.payload[6..].iter().all(|b| *b == 0));
    }

    #[test]
    fn page_size_lengths() {
        assert_eq!(
            set_page_size(PageSizeFormat::Len2, 1, 2, 3).payload.len(),
            2
        );
        assert_eq!(
            set_page_size(PageSizeFormat::Len4, 1, 2, 3).payload.len(),
            4
        );
        assert_eq!(
            set_page_size(PageSizeFormat::Len6, 1, 2, 3).payload.len(),
            6
        );
    }

    #[test]
    fn device_settings_layouts() {
        assert_eq!(set_auto_off(2).payload, vec![2]);
        assert_eq!(calibrate(LabelType::Gap).payload, vec![1]);
        assert_eq!(calibrate(LabelType::BlackMark).cmd, 0x8E);
        assert_eq!(sound_get(SoundKind::Power).payload, vec![2, 2, 0]);
        assert_eq!(sound_set(SoundKind::Bluetooth, true).payload, vec![1, 1, 1]);
        assert_eq!(expected_response(cmd::CALIBRATE, Some(1)), Some(0x8F));
        assert_eq!(expected_response(cmd::SOUND, Some(2)), Some(0x68));
        assert_eq!(expected_response(cmd::SET_AUTO_OFF, Some(1)), Some(0x37));
    }

    #[test]
    fn expected_responses() {
        assert_eq!(expected_response(cmd::PRINTER_INFO, Some(8)), Some(0x48));
        assert_eq!(expected_response(cmd::HEARTBEAT, Some(4)), Some(0xD9));
        assert_eq!(expected_response(cmd::ROW_BITMAP, None), None);
    }
}
