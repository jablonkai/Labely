//! Phomemo printing: ESC/POS raster blocks inside vendor commands
//! (docs/protocol-languages.md §3).

use crate::escpos::{self, Command, DecodeError, Decoded, PageCollector};
use crate::raster::RasterView;

/// First bytes of the vendor commands.
pub const VENDOR: [u8; 2] = [0x1F, 0x11];

/// The three job layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// M02, M02 Pro, T02: receipt-style printers on continuous paper.
    M02,
    /// M110, M120, M220: label printers with a media sensor.
    M110,
    /// D30: narrow labels, one raster block per label.
    D30,
}

/// `1F 11 n`: what the printer looks for between labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    Gap,
    Continuous,
    BlackMark,
}

impl Media {
    fn code(self) -> u8 {
        match self {
            Media::Gap => 0x0A,
            Media::Continuous => 0x0B,
            Media::BlackMark => 0x26,
        }
    }
}

/// Rows per raster block of the M02 family.
pub const M02_BLOCK_ROWS: u16 = 255;
/// Rows per raster block of the M110 family.
pub const M110_BLOCK_ROWS: u16 = 240;
/// M110 print speed, 1 (slow) to 5. Labely always uses the middle.
pub const M110_SPEED: u8 = 3;
/// Highest M110 density.
pub const M110_MAX_DENSITY: u8 = 15;

/// Settings of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSettings {
    /// M110 family only: 1 (lightest) to 15.
    pub density: u8,
    /// M110 and D30 families. D30 has no black-mark code and treats it as a gap.
    pub media: Media,
}

/// The bytes of one printed page. Copies are the same bytes again.
pub fn page(family: Family, view: &RasterView<'_>, settings: &JobSettings) -> Vec<u8> {
    let mut out = Vec::with_capacity(view.row_bytes() * view.height() as usize + 64);
    match family {
        Family::M02 => {
            out.extend_from_slice(&escpos::init());
            // Centre a page narrower than the head.
            out.extend_from_slice(&[escpos::ESC, b'a', 1]);
            out.extend_from_slice(&[0x1F, 0x11, 0x02, 0x04]);
            escpos::raster(view, M02_BLOCK_ROWS, &mut out);
            out.extend_from_slice(&escpos::feed_lines(2));
            out.extend_from_slice(&escpos::feed_lines(2));
        }
        Family::M110 => {
            out.extend_from_slice(&[escpos::ESC, b'N', 0x0D, M110_SPEED]);
            out.extend_from_slice(&[
                escpos::ESC,
                b'N',
                0x04,
                settings.density.clamp(1, M110_MAX_DENSITY),
            ]);
            out.extend_from_slice(&[0x1F, 0x11, settings.media.code()]);
            escpos::raster(view, M110_BLOCK_ROWS, &mut out);
            out.extend_from_slice(&[0x1F, 0xF0, 0x05, 0x00, 0x1F, 0xF0, 0x03, 0x00]);
        }
        Family::D30 => {
            let media = match settings.media {
                Media::Continuous => Media::Continuous,
                Media::Gap | Media::BlackMark => Media::Gap,
            };
            out.extend_from_slice(&[0x1F, 0x11, media.code()]);
            out.extend_from_slice(&escpos::init());
            escpos::raster(view, u16::MAX, &mut out);
            out.extend_from_slice(&escpos::feed_lines(0));
        }
    }
    out
}

/// `1F 11 11`: ask whether paper is loaded.
pub const PAPER_REQUEST: [u8; 3] = [0x1F, 0x11, 0x11];
/// `1F 11 12`: ask whether the cover is closed.
pub const COVER_REQUEST: [u8; 3] = [0x1F, 0x11, 0x12];

/// Find the `1A <kind> <value>` message of `kind` in `reply`.
fn message(reply: &[u8], kind: u8) -> Option<u8> {
    reply
        .windows(3)
        .find(|w| w[0] == 0x1A && w[1] == kind)
        .map(|w| w[2])
}

/// From a `1A 06 n` message: whether paper is loaded (`88` is out of paper).
pub fn paper_present(reply: &[u8]) -> Option<bool> {
    message(reply, 0x06).map(|v| v != 0x88)
}

/// From a `1A 05 n` message: `99` is closed, `98` is open.
pub fn cover_closed(reply: &[u8]) -> Option<bool> {
    match message(reply, 0x05)? {
        0x99 => Some(true),
        0x98 => Some(false),
        _ => None,
    }
}

/// Decode a stream of the commands this module writes.
pub fn decode(bytes: &[u8]) -> Result<Decoded, DecodeError> {
    let mut pages = PageCollector::new();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        let skip = match rest {
            [0x1F, 0x11, 0x02, _, ..] => 4,
            [0x1F, 0x11, 0x0A | 0x0B | 0x26, ..] => 3,
            [0x1B, b'N', 0x0D | 0x04, _, ..] => 4,
            [0x1F, 0xF0, 0x05, 0x00, ..] => {
                pages.end_page();
                4
            }
            [0x1F, 0xF0, 0x03, 0x00, ..] => 4,
            _ => {
                let (command, len) = escpos::parse_command(rest, i)?;
                // A raster block after a feed starts a new page; ESC d 0 ends a D30 label.
                if matches!(command, Command::Cut(_)) {
                    return Err(DecodeError::Unknown {
                        offset: i,
                        byte: rest[0],
                    });
                }
                pages.apply(command, i)?;
                len
            }
        };
        i += skip;
    }
    Ok(pages.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: [u8; 4] = [0xFF, 0xFF, 0x80, 0x1F];

    fn view() -> RasterView<'static> {
        RasterView::new(12, 2, 2, &DATA).unwrap()
    }

    fn settings() -> JobSettings {
        JobSettings {
            density: 8,
            media: Media::Gap,
        }
    }

    #[test]
    fn m02_bytes() {
        assert_eq!(
            page(Family::M02, &view(), &settings()),
            [
                0x1B, 0x40, 0x1B, 0x61, 0x01, 0x1F, 0x11, 0x02,
                0x04, // init, centre, 1F 11 02 04
                0x1D, 0x76, 0x30, 0x00, 0x02, 0x00, 0x02, 0x00, 0xFF, 0xF0, 0x80, 0x10, 0x1B, 0x64,
                0x02, 0x1B, 0x64, 0x02,
            ]
        );
    }

    #[test]
    fn m110_bytes() {
        let mut s = settings();
        s.media = Media::BlackMark;
        s.density = 99;
        assert_eq!(
            page(Family::M110, &view(), &s),
            [
                0x1B, 0x4E, 0x0D, 0x03, 0x1B, 0x4E, 0x04, 0x0F, 0x1F, 0x11, 0x26, //
                0x1D, 0x76, 0x30, 0x00, 0x02, 0x00, 0x02, 0x00, 0xFF, 0xF0, 0x80, 0x10, 0x1F, 0xF0,
                0x05, 0x00, 0x1F, 0xF0, 0x03, 0x00,
            ]
        );
    }

    #[test]
    fn d30_bytes() {
        assert_eq!(
            page(Family::D30, &view(), &settings()),
            [
                0x1F, 0x11, 0x0A, 0x1B, 0x40, //
                0x1D, 0x76, 0x30, 0x00, 0x02, 0x00, 0x02, 0x00, 0xFF, 0xF0, 0x80, 0x10, 0x1B, 0x64,
                0x00,
            ]
        );
    }

    #[test]
    fn every_family_decodes_back_page_by_page() {
        let data: Vec<u8> = (0..1000u32).map(|i| i as u8).collect();
        let tall = RasterView::new(16, 500, 2, &data).unwrap();
        for family in [Family::M02, Family::M110, Family::D30] {
            let one = page(family, &tall, &settings());
            let decoded = decode(&[one.clone(), one].concat()).unwrap();
            assert_eq!(decoded.pages.len(), 2, "{family:?}");
            assert_eq!(decoded.pages[1].data(), data.as_slice(), "{family:?}");
        }
        assert!(decode(&[0x1D, 0x56, 0x00]).is_err());
    }

    #[test]
    fn status_messages() {
        assert_eq!(paper_present(&[0x1A, 0x06, 0x88]), Some(false));
        assert_eq!(paper_present(&[0x00, 0x1A, 0x06, 0x89]), Some(true));
        assert_eq!(cover_closed(&[0x1A, 0x05, 0x99]), Some(true));
        assert_eq!(cover_closed(&[0x1A, 0x05, 0x98]), Some(false));
        assert_eq!(cover_closed(&[0x1A, 0x06, 0x88]), None);
    }
}
