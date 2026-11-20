//! ESC/POS raster printing (docs/protocol-languages.md §2).
//!
//! A job is a plain byte stream: initialise, raster blocks, feed, optional cut. The
//! printer answers nothing unless a real-time status is requested.

use thiserror::Error;

use crate::raster::{Raster, RasterView};

pub const ESC: u8 = 0x1B;
pub const GS: u8 = 0x1D;
pub const DLE: u8 = 0x10;
pub const EOT: u8 = 0x04;

/// Rows per `GS v 0` block. Every ESC/POS printer takes at least this many (§2.2).
pub const DEFAULT_BLOCK_ROWS: u16 = 255;

/// Paper cut after a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    Full,
    Partial,
}

/// One command of the stream, as the encoder writes it and the decoder reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `ESC @`
    Init,
    /// `ESC a n`
    Justify(u8),
    /// `GS v 0 m xL xH yL yH d…`: `rows` hold `width_bytes` bytes per row.
    Raster { width_bytes: u16, rows: Vec<u8> },
    /// `ESC d n`
    FeedLines(u8),
    /// `GS V m`
    Cut(Cut),
}

/// `ESC @`: reset the printer to its power-on settings.
pub fn init() -> [u8; 2] {
    [ESC, b'@']
}

/// `ESC d n`: print the buffer and feed `n` lines.
pub fn feed_lines(n: u8) -> [u8; 3] {
    [ESC, b'd', n]
}

/// `GS V m`: cut the paper.
pub fn cut(kind: Cut) -> [u8; 3] {
    [
        GS,
        b'V',
        match kind {
            Cut::Full => 0,
            Cut::Partial => 1,
        },
    ]
}

/// Append `view` as `GS v 0` blocks of at most `block_rows` rows.
pub fn raster(view: &RasterView<'_>, block_rows: u16, out: &mut Vec<u8>) {
    let block_rows = u32::from(block_rows.max(1));
    let width_bytes = view.row_bytes().min(usize::from(u16::MAX)) as u16;
    let mut y = 0;
    while y < view.height() {
        let rows = (view.height() - y).min(block_rows) as u16;
        out.extend_from_slice(&[GS, b'v', b'0', 0]);
        out.extend_from_slice(&width_bytes.to_le_bytes());
        out.extend_from_slice(&rows.to_le_bytes());
        for r in 0..u32::from(rows) {
            out.extend_from_slice(&view.row(y + r));
        }
        y += u32::from(rows);
    }
}

/// Settings of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSettings {
    /// Lines fed after the page, so the print clears the tear bar.
    pub feed_lines: u8,
    pub cut: Option<Cut>,
    pub block_rows: u16,
}

impl Default for JobSettings {
    fn default() -> Self {
        Self {
            feed_lines: 3,
            cut: None,
            block_rows: DEFAULT_BLOCK_ROWS,
        }
    }
}

/// The bytes of one printed page. Copies are the same bytes again.
pub fn page(view: &RasterView<'_>, settings: &JobSettings) -> Vec<u8> {
    let mut out = Vec::with_capacity(view.row_bytes() * view.height() as usize + 64);
    out.extend_from_slice(&init());
    raster(view, settings.block_rows, &mut out);
    out.extend_from_slice(&feed_lines(settings.feed_lines));
    if let Some(kind) = settings.cut {
        out.extend_from_slice(&cut(kind));
    }
    out
}

/// `DLE EOT n`: ask for real-time status byte `n` (1–4).
pub fn status_request(n: u8) -> [u8; 3] {
    [DLE, EOT, n]
}

/// Offline cause status.
pub const STATUS_OFFLINE: u8 = 2;
/// Paper sensor status.
pub const STATUS_PAPER: u8 = 4;

/// Whether `byte` has the fixed bits of a real-time status reply (bits 1 and 4 set, bits 0
/// and 7 clear).
pub fn is_status_byte(byte: u8) -> bool {
    byte & 0x93 == 0x12
}

/// From the reply to `DLE EOT 2`: whether the cover is closed.
pub fn cover_closed(byte: u8) -> Option<bool> {
    is_status_byte(byte).then_some(byte & 0x04 == 0)
}

/// From the reply to `DLE EOT 4`: whether paper is present at the end sensor.
pub fn paper_present(byte: u8) -> Option<bool> {
    is_status_byte(byte).then_some(byte & 0x60 == 0)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("unknown command byte 0x{byte:02x} at offset {offset}")]
    Unknown { offset: usize, byte: u8 },
    #[error("stream ends inside the command at offset {0}")]
    Truncated(usize),
    #[error("raster block at offset {offset} is {got} bytes wide, the page is {expected}")]
    WidthChanged {
        offset: usize,
        expected: u16,
        got: u16,
    },
}

/// Read the command at the start of `bytes`. Returns it with its length; `offset` is only
/// used in errors.
pub fn parse_command(bytes: &[u8], offset: usize) -> Result<(Command, usize), DecodeError> {
    let need = |n: usize| {
        if bytes.len() < n {
            Err(DecodeError::Truncated(offset))
        } else {
            Ok(())
        }
    };
    let unknown = |i: usize| DecodeError::Unknown {
        offset: offset + i,
        byte: bytes.get(i).copied().unwrap_or(0),
    };
    need(2)?;
    match (bytes[0], bytes[1]) {
        (ESC, b'@') => Ok((Command::Init, 2)),
        (ESC, b'a') => {
            need(3)?;
            Ok((Command::Justify(bytes[2]), 3))
        }
        (ESC, b'd') => {
            need(3)?;
            Ok((Command::FeedLines(bytes[2]), 3))
        }
        (GS, b'V') => {
            need(3)?;
            match bytes[2] {
                0 | b'0' => Ok((Command::Cut(Cut::Full), 3)),
                1 | b'1' => Ok((Command::Cut(Cut::Partial), 3)),
                _ => Err(unknown(2)),
            }
        }
        (GS, b'v') => {
            need(8)?;
            if bytes[2] != b'0' {
                return Err(unknown(2));
            }
            let width_bytes = u16::from_le_bytes([bytes[4], bytes[5]]);
            let rows = u16::from_le_bytes([bytes[6], bytes[7]]);
            let len = usize::from(width_bytes) * usize::from(rows);
            need(8 + len)?;
            Ok((
                Command::Raster {
                    width_bytes,
                    rows: bytes[8..8 + len].to_vec(),
                },
                8 + len,
            ))
        }
        (ESC | GS, _) => Err(unknown(1)),
        _ => Err(unknown(0)),
    }
}

/// What a printer would have printed from a stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decoded {
    /// One raster per page: the blocks between two feeds or cuts.
    pub pages: Vec<Raster>,
    pub cuts: u32,
}

/// Collects raster blocks into pages. Shared with the dialects built on ESC/POS.
#[derive(Debug, Default)]
pub struct PageCollector {
    current: Option<Raster>,
    done: Decoded,
}

impl PageCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one command.
    pub fn apply(&mut self, command: Command, offset: usize) -> Result<(), DecodeError> {
        match command {
            Command::Init | Command::Justify(_) => {}
            Command::Raster { width_bytes, rows } => {
                let page = self
                    .current
                    .get_or_insert_with(|| Raster::new(u32::from(width_bytes) * 8));
                if page.row_bytes() != usize::from(width_bytes) {
                    return Err(DecodeError::WidthChanged {
                        offset,
                        expected: page.row_bytes() as u16,
                        got: width_bytes,
                    });
                }
                page.push_rows(&rows);
            }
            Command::FeedLines(_) => self.end_page(),
            Command::Cut(_) => {
                self.end_page();
                self.done.cuts += 1;
            }
        }
        Ok(())
    }

    /// Close the page being collected, if any.
    pub fn end_page(&mut self) {
        if let Some(page) = self.current.take() {
            self.done.pages.push(page);
        }
    }

    pub fn finish(mut self) -> Decoded {
        self.end_page();
        self.done
    }
}

/// Decode a stream of the commands this module writes.
pub fn decode(bytes: &[u8]) -> Result<Decoded, DecodeError> {
    let mut pages = PageCollector::new();
    let mut i = 0;
    while i < bytes.len() {
        let (command, len) = parse_command(&bytes[i..], i)?;
        pages.apply(command, i)?;
        i += len;
    }
    Ok(pages.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_bytes() {
        // 12 dots wide, 2 rows: 1111 1111 1111 / 1000 0000 0001.
        let data = [0xFF, 0xFF, 0x80, 0x1F];
        let view = RasterView::new(12, 2, 2, &data).unwrap();
        let bytes = page(
            &view,
            &JobSettings {
                feed_lines: 2,
                cut: Some(Cut::Partial),
                block_rows: 255,
            },
        );
        assert_eq!(
            bytes,
            [
                0x1B, 0x40, // ESC @
                0x1D, 0x76, 0x30, 0x00, 0x02, 0x00, 0x02, 0x00, // GS v 0, 2 bytes x 2 rows
                0xFF, 0xF0, 0x80, 0x10, // padding bits cleared
                0x1B, 0x64, 0x02, // ESC d 2
                0x1D, 0x56, 0x01, // GS V 1
            ]
        );
    }

    #[test]
    fn long_pages_are_split_and_decode_back() {
        let data: Vec<u8> = (0..600u32)
            .flat_map(|y| [y as u8, (y >> 8) as u8])
            .collect();
        let view = RasterView::new(16, 600, 2, &data).unwrap();
        let bytes = page(&view, &JobSettings::default());
        assert_eq!(bytes.windows(3).filter(|w| w == b"\x1dv0").count(), 3);
        let decoded = decode(&[bytes.clone(), bytes].concat()).unwrap();
        assert_eq!(decoded.pages.len(), 2);
        assert_eq!(decoded.cuts, 0);
        assert_eq!(decoded.pages[0].data(), data.as_slice());
        assert_eq!(
            (decoded.pages[1].width(), decoded.pages[1].height()),
            (16, 600)
        );
    }

    #[test]
    fn decode_errors() {
        assert_eq!(decode(&[0x1B, 0x40, 0x99]), Err(DecodeError::Truncated(2)));
        assert_eq!(
            decode(&[0x1B, 0x40, 0x99, 0x00]),
            Err(DecodeError::Unknown {
                offset: 2,
                byte: 0x99
            })
        );
        assert_eq!(
            decode(&[0x1D, 0x76, 0x30, 0, 1, 0, 2, 0, 0xAA]),
            Err(DecodeError::Truncated(0))
        );
        let mut two = vec![0x1D, 0x76, 0x30, 0, 1, 0, 1, 0, 0xAA];
        two.extend_from_slice(&[0x1D, 0x76, 0x30, 0, 2, 0, 1, 0, 0xAA, 0xBB]);
        assert!(matches!(
            decode(&two),
            Err(DecodeError::WidthChanged { offset: 9, .. })
        ));
    }

    #[test]
    fn status_bytes() {
        assert_eq!(paper_present(0x12), Some(true));
        assert_eq!(paper_present(0x72), Some(false));
        assert_eq!(cover_closed(0x12), Some(true));
        assert_eq!(cover_closed(0x16), Some(false));
        assert_eq!(paper_present(0x00), None);
        assert_eq!(status_request(STATUS_PAPER), [0x10, 0x04, 0x04]);
    }
}
