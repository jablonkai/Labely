//! TSPL label printing (docs/protocol-languages.md §5).
//!
//! A label is a few text lines ending in CR LF, with the bitmap bytes inside the `BITMAP`
//! line. The printer answers nothing unless the status byte is requested.

use std::fmt::Write as _;

use thiserror::Error;

use crate::raster::{Raster, RasterView};

/// Highest `DENSITY` value.
pub const MAX_DENSITY: u8 = 15;

/// How the printer finds the start of a label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Media {
    /// Die-cut labels with a gap of `gap_mm` between them.
    Gap {
        gap_mm: f64,
    },
    /// A black mark `mark_mm` high on the back of the liner.
    BlackMark {
        mark_mm: f64,
    },
    Continuous,
}

/// Settings of a label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JobSettings {
    pub dpi: u32,
    /// 0 (lightest) to 15.
    pub density: u8,
    pub media: Media,
    pub copies: u16,
}

fn mm(dots: u32, dpi: u32) -> f64 {
    f64::from(dots) * 25.4 / f64::from(dpi.max(1))
}

/// The bytes of one label, printed `copies` times.
pub fn label(view: &RasterView<'_>, settings: &JobSettings) -> Vec<u8> {
    let mut head = String::new();
    // Writing to a String cannot fail.
    let _ = write!(
        head,
        "SIZE {:.2} mm,{:.2} mm\r\n",
        mm(view.width(), settings.dpi),
        mm(view.height(), settings.dpi)
    );
    let _ = match settings.media {
        Media::Gap { gap_mm } => write!(head, "GAP {gap_mm:.2} mm,0 mm\r\n"),
        Media::BlackMark { mark_mm } => write!(head, "BLINE {mark_mm:.2} mm,0 mm\r\n"),
        Media::Continuous => write!(head, "GAP 0 mm,0 mm\r\n"),
    };
    let _ = write!(
        head,
        "DIRECTION 0\r\nREFERENCE 0,0\r\nDENSITY {}\r\nCLS\r\nBITMAP 0,0,{},{},0,",
        settings.density.min(MAX_DENSITY),
        view.row_bytes(),
        view.height()
    );
    let mut out = head.into_bytes();
    out.reserve(view.row_bytes() * view.height() as usize + 24);
    for y in 0..view.height() {
        // In a TSPL bitmap a 0 bit is a printed dot.
        out.extend(view.row(y).iter().map(|b| !b));
    }
    out.extend_from_slice(format!("\r\nPRINT 1,{}\r\n", settings.copies.max(1)).as_bytes());
    out
}

/// `<ESC>!?`: ask for the status byte.
pub const STATUS_REQUEST: [u8; 3] = [0x1B, b'!', b'?'];

/// The status byte, decoded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Status {
    pub head_open: bool,
    pub paper_jam: bool,
    pub out_of_paper: bool,
    pub out_of_ribbon: bool,
    pub paused: bool,
    pub printing: bool,
}

impl Status {
    pub fn from_byte(byte: u8) -> Self {
        Self {
            head_open: byte & 0x01 != 0,
            paper_jam: byte & 0x02 != 0,
            out_of_paper: byte & 0x04 != 0,
            out_of_ribbon: byte & 0x08 != 0,
            paused: byte & 0x10 != 0,
            printing: byte & 0x20 != 0,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("unknown command at offset {0}")]
    Unknown(usize),
    #[error("bad arguments in the command at offset {0}")]
    BadArguments(usize),
    #[error("stream ends inside the command at offset {0}")]
    Truncated(usize),
    #[error("PRINT at offset {0} without a bitmap")]
    NothingToPrint(usize),
}

/// One label as a printer would have printed it.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedLabel {
    pub raster: Raster,
    pub copies: u32,
    /// `SIZE`, in mm.
    pub size_mm: (f64, f64),
    pub media: Option<Media>,
    pub density: Option<u8>,
}

fn first_mm(args: &str) -> Option<f64> {
    args.split(',')
        .next()?
        .trim()
        .trim_end_matches("mm")
        .trim()
        .parse()
        .ok()
}

fn two_mm(args: &str) -> Option<(f64, f64)> {
    let (a, b) = args.split_once(',')?;
    Some((first_mm(a)?, first_mm(b)?))
}

/// Decode a stream of the commands this module writes.
pub fn decode(bytes: &[u8]) -> Result<Vec<DecodedLabel>, DecodeError> {
    const BITMAP: &[u8] = b"BITMAP ";
    let mut labels = Vec::new();
    let mut size_mm = (0.0, 0.0);
    let mut media = None;
    let mut density = None;
    let mut bitmap: Option<Raster> = None;
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        if rest.starts_with(BITMAP) {
            // Five numbers, then the data starts after the fifth comma.
            let mut commas = rest.iter().enumerate().filter(|(_, b)| **b == b',');
            let end = commas.nth(4).ok_or(DecodeError::Truncated(i))?.0;
            let args = std::str::from_utf8(&rest[BITMAP.len()..end])
                .map_err(|_| DecodeError::BadArguments(i))?;
            let nums: Vec<u32> = args
                .split(',')
                .map(|n| n.trim().parse())
                .collect::<Result<_, _>>()
                .map_err(|_| DecodeError::BadArguments(i))?;
            let [0, 0, width_bytes, height, 0] = nums[..] else {
                return Err(DecodeError::BadArguments(i));
            };
            let len = width_bytes as usize * height as usize;
            let data = rest
                .get(end + 1..end + 1 + len)
                .ok_or(DecodeError::Truncated(i))?;
            if rest.get(end + 1 + len..end + 3 + len) != Some(b"\r\n") {
                return Err(DecodeError::Truncated(i));
            }
            let mut raster = Raster::new(width_bytes * 8);
            raster.push_rows(&data.iter().map(|b| !b).collect::<Vec<u8>>());
            bitmap = Some(raster);
            i += end + 3 + len;
            continue;
        }
        let end = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or(DecodeError::Truncated(i))?;
        let line = std::str::from_utf8(&rest[..end]).map_err(|_| DecodeError::Unknown(i))?;
        let (name, args) = line.split_once(' ').unwrap_or((line, ""));
        let bad = DecodeError::BadArguments(i);
        match name {
            "SIZE" => size_mm = two_mm(args).ok_or(bad)?,
            "GAP" => {
                let gap_mm = first_mm(args).ok_or(bad)?;
                media = Some(if gap_mm == 0.0 {
                    Media::Continuous
                } else {
                    Media::Gap { gap_mm }
                });
            }
            "BLINE" => {
                media = Some(Media::BlackMark {
                    mark_mm: first_mm(args).ok_or(bad)?,
                });
            }
            "DENSITY" => density = Some(args.trim().parse().map_err(|_| bad)?),
            "DIRECTION" | "REFERENCE" => {}
            "CLS" => bitmap = None,
            "PRINT" => {
                let (sets, copies) = args.split_once(',').ok_or(bad)?;
                let sets: u32 = sets
                    .trim()
                    .parse()
                    .map_err(|_| DecodeError::BadArguments(i))?;
                let copies: u32 = copies
                    .trim()
                    .parse()
                    .map_err(|_| DecodeError::BadArguments(i))?;
                labels.push(DecodedLabel {
                    raster: bitmap.clone().ok_or(DecodeError::NothingToPrint(i))?,
                    copies: sets * copies,
                    size_mm,
                    media,
                    density,
                });
            }
            _ => return Err(DecodeError::Unknown(i)),
        }
        i += end + 2;
    }
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> JobSettings {
        JobSettings {
            dpi: 203,
            density: 8,
            media: Media::Gap { gap_mm: 2.0 },
            copies: 3,
        }
    }

    #[test]
    fn label_bytes() {
        // 12 dots wide, 2 rows.
        let data = [0xFF, 0xFF, 0x80, 0x1F];
        let view = RasterView::new(12, 2, 2, &data).unwrap();
        let bytes = label(&view, &settings());
        let mut expected = b"SIZE 1.50 mm,0.25 mm\r\nGAP 2.00 mm,0 mm\r\nDIRECTION 0\r\n\
            REFERENCE 0,0\r\nDENSITY 8\r\nCLS\r\nBITMAP 0,0,2,2,0,"
            .to_vec();
        // Inverted: a 0 bit prints. The padding bits stay white (1).
        expected.extend_from_slice(&[0x00, 0x0F, 0x7F, 0xEF]);
        expected.extend_from_slice(b"\r\nPRINT 1,3\r\n");
        assert_eq!(bytes, expected);
    }

    #[test]
    fn decodes_back_with_commas_and_newlines_in_the_data() {
        // Bytes that invert to ',' '\r' '\n' must not confuse the decoder.
        let data = [!b',', !b'\r', !b'\n', 0x00];
        let view = RasterView::new(16, 2, 2, &data).unwrap();
        let mut s = settings();
        s.media = Media::Continuous;
        let one = label(&view, &s);
        s.media = Media::BlackMark { mark_mm: 3.0 };
        s.copies = 1;
        let labels = decode(&[one, label(&view, &s)].concat()).unwrap();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].raster.data(), &data);
        assert_eq!((labels[0].copies, labels[1].copies), (3, 1));
        assert_eq!(labels[0].media, Some(Media::Continuous));
        assert_eq!(labels[1].media, Some(Media::BlackMark { mark_mm: 3.0 }));
        assert_eq!(labels[0].density, Some(8));
        assert_eq!(labels[0].size_mm, (2.0, 0.25));
    }

    #[test]
    fn decode_errors() {
        assert_eq!(decode(b"FEED 1\r\n"), Err(DecodeError::Unknown(0)));
        assert_eq!(
            decode(b"CLS\r\nPRINT 1,1\r\n"),
            Err(DecodeError::NothingToPrint(5))
        );
        assert_eq!(decode(b"CLS"), Err(DecodeError::Truncated(0)));
        assert_eq!(
            decode(b"BITMAP 0,0,2,2,0,\x00\r\n"),
            Err(DecodeError::Truncated(0))
        );
        assert_eq!(
            decode(b"BITMAP 5,0,1,1,0,\x00\r\n"),
            Err(DecodeError::BadArguments(0))
        );
    }

    #[test]
    fn status_byte() {
        assert_eq!(Status::from_byte(0), Status::default());
        let s = Status::from_byte(0x05);
        assert!(s.head_open && s.out_of_paper && !s.paused);
        assert!(Status::from_byte(0x20).printing);
    }
}
