//! NIIMBOT protocol (docs/protocol.md).

pub mod job;
pub mod packet;
pub mod request;
pub mod response;
pub mod rows;

pub use packet::{Decoder, Packet, PacketError};

/// Command codes (docs/protocol.md §2). Names are Labely's own.
pub mod cmd {
    pub const CONNECT: u8 = 0xC1;
    pub const CONNECT_RESP: u8 = 0xC2;
    pub const STATUS_DATA: u8 = 0xA5;
    pub const STATUS_DATA_RESP: u8 = 0xB5;
    pub const PRINTER_INFO: u8 = 0x40;
    pub const CAPABILITIES: u8 = 0xAF;
    pub const CAPABILITIES_RESP: u8 = 0xBF;
    pub const HEARTBEAT: u8 = 0xDC;
    pub const HEARTBEAT_ADV1: u8 = 0xDD;
    pub const HEARTBEAT_BASIC: u8 = 0xDE;
    pub const HEARTBEAT_INFO: u8 = 0xDF;
    pub const HEARTBEAT_ADV2: u8 = 0xD9;
    pub const RFID_INFO: u8 = 0x1A;
    pub const RFID_INFO_RESP: u8 = 0x1B;
    pub const PAPER_INFO: u8 = 0x59;
    pub const PAPER_INFO_RESP: u8 = 0x69;
    pub const SET_DENSITY: u8 = 0x21;
    pub const SET_DENSITY_RESP: u8 = 0x31;
    pub const SET_LABEL_TYPE: u8 = 0x23;
    pub const SET_LABEL_TYPE_RESP: u8 = 0x33;
    pub const SET_AUTO_OFF: u8 = 0x27;
    pub const SET_AUTO_OFF_RESP: u8 = 0x37;
    pub const CALIBRATE: u8 = 0x8E;
    pub const CALIBRATE_RESP: u8 = 0x8F;
    pub const SOUND: u8 = 0x58;
    pub const SOUND_RESP: u8 = 0x68;
    pub const PRINT_START: u8 = 0x01;
    pub const PRINT_START_RESP: u8 = 0x02;
    pub const PAGE_START: u8 = 0x03;
    pub const PAGE_START_RESP: u8 = 0x04;
    pub const SET_PAGE_SIZE: u8 = 0x13;
    pub const SET_PAGE_SIZE_RESP: u8 = 0x14;
    pub const SET_QUANTITY: u8 = 0x15;
    pub const SET_QUANTITY_RESP: u8 = 0x16;
    pub const PRINT_CLEAR: u8 = 0x20;
    pub const PRINT_CLEAR_RESP: u8 = 0x30;
    pub const ROWS_EMPTY: u8 = 0x84;
    pub const ROW_BITMAP: u8 = 0x85;
    pub const PAGE_END: u8 = 0xE3;
    pub const PAGE_END_RESP: u8 = 0xE4;
    pub const PRINT_STATUS: u8 = 0xA3;
    pub const PRINT_STATUS_RESP: u8 = 0xB3;
    pub const PRINT_END: u8 = 0xF3;
    pub const PRINT_END_RESP: u8 = 0xF4;
    pub const CANCEL: u8 = 0xDA;
    pub const CANCEL_RESP: u8 = 0xD0;
    /// Generic error reply; payload byte 0 is an [`super::response::PrinterError`] code.
    pub const ERROR: u8 = 0xDB;
    /// Generic "command not supported" reply.
    pub const NOT_SUPPORTED: u8 = 0x00;
}
