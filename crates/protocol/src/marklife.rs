//! Marklife "L11" printing (P12, P15): a session frame around one ESC/POS raster block
//! (docs/protocol-languages.md §4).

use crate::escpos::{self, DecodeError, Decoded, PageCollector};
use crate::raster::RasterView;

/// Zero bytes that wake the printer before a job.
pub const WAKE_BYTES: usize = 15;
/// `10 FF F1 02`: start of a job.
pub const ENABLE: [u8; 4] = [0x10, 0xFF, 0xF1, 0x02];
/// `10 FF F1 45`: end of a job.
pub const STOP: [u8; 4] = [0x10, 0xFF, 0xF1, 0x45];
/// Dots fed after a page on continuous tape (`ESC J n`).
pub const CONTINUOUS_FEED_DOTS: u8 = 100;
/// The byte the printer sends when a job is done.
pub const DONE: u8 = 0xAA;
/// Highest density.
pub const MAX_DENSITY: u8 = 3;

/// Settings of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSettings {
    /// 1 (lightest) to 3.
    pub density: u8,
    /// Feed to the next gap after the page; otherwise feed a fixed length.
    pub gap: bool,
}

/// The bytes of one printed page. Copies are the same bytes again.
pub fn page(view: &RasterView<'_>, settings: &JobSettings) -> Vec<u8> {
    let mut out = Vec::with_capacity(view.row_bytes() * view.height() as usize + 48);
    out.extend_from_slice(&[0x1F, 0x70, 0x02, settings.density.clamp(1, MAX_DENSITY)]);
    out.extend_from_slice(&[0; WAKE_BYTES]);
    out.extend_from_slice(&ENABLE);
    escpos::raster(view, u16::MAX, &mut out);
    if settings.gap {
        out.extend_from_slice(&[escpos::GS, 0x0C]);
    } else {
        out.extend_from_slice(&[escpos::ESC, b'J', CONTINUOUS_FEED_DOTS]);
    }
    out.extend_from_slice(&STOP);
    out
}

/// Whether `reply` holds the printer's "job done" byte.
pub fn is_done(reply: &[u8]) -> bool {
    reply.contains(&DONE)
}

/// Decode a stream of the commands this module writes.
pub fn decode(bytes: &[u8]) -> Result<Decoded, DecodeError> {
    let mut pages = PageCollector::new();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        i += match rest {
            [0x00, ..] => 1,
            [0x1F, 0x70, 0x02, _, ..] => 4,
            [0x10, 0xFF, 0xF1, 0x02, ..] => 4,
            [0x10, 0xFF, 0xF1, 0x45, ..] => {
                pages.end_page();
                4
            }
            [0x1D, 0x0C, ..] => {
                pages.end_page();
                2
            }
            [0x1B, b'J', _, ..] => {
                pages.end_page();
                3
            }
            [0x1D, b'v', ..] => {
                let (command, len) = escpos::parse_command(rest, i)?;
                pages.apply(command, i)?;
                len
            }
            _ => {
                return Err(DecodeError::Unknown {
                    offset: i,
                    byte: rest[0],
                });
            }
        };
    }
    Ok(pages.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_bytes() {
        let data = [0xFF, 0xFF, 0x80, 0x1F];
        let view = RasterView::new(12, 2, 2, &data).unwrap();
        let mut expected = vec![0x1F, 0x70, 0x02, 0x03];
        expected.extend_from_slice(&[0; 15]);
        expected.extend_from_slice(&[0x10, 0xFF, 0xF1, 0x02]);
        expected.extend_from_slice(&[0x1D, 0x76, 0x30, 0x00, 0x02, 0x00, 0x02, 0x00]);
        expected.extend_from_slice(&[0xFF, 0xF0, 0x80, 0x10]);
        let mut gap = expected.clone();
        expected.extend_from_slice(&[0x1B, 0x4A, 0x64, 0x10, 0xFF, 0xF1, 0x45]);
        gap.extend_from_slice(&[0x1D, 0x0C, 0x10, 0xFF, 0xF1, 0x45]);
        let s = JobSettings {
            density: 9,
            gap: false,
        };
        assert_eq!(page(&view, &s), expected);
        assert_eq!(page(&view, &JobSettings { gap: true, ..s }), gap);
    }

    #[test]
    fn decodes_back() {
        // Rows of zero bytes must not be taken for wake bytes.
        let data = [0x00, 0x00, 0x12, 0x34, 0x00, 0x00];
        let view = RasterView::new(16, 3, 2, &data).unwrap();
        let s = JobSettings {
            density: 2,
            gap: true,
        };
        let one = page(&view, &s);
        let decoded = decode(&[one.clone(), one].concat()).unwrap();
        assert_eq!(decoded.pages.len(), 2);
        assert_eq!(decoded.pages[0].data(), &data);
        assert!(decode(&[0x1B, 0x40]).is_err());
        assert!(is_done(&[0x01, 0x04, 0xAA]));
        assert!(!is_done(&[0x01, 0x04]));
    }
}
