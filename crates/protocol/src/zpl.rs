//! ZPL II label printing (docs/protocol-languages.md §6).
//!
//! A label is one `^XA … ^XZ` format holding the page as an ASCII-hex graphic field. The
//! printer answers nothing unless the host status is requested.

use std::fmt::Write as _;

use thiserror::Error;

use crate::raster::{Raster, RasterView};

/// Highest `~SD` darkness.
pub const MAX_DARKNESS: u8 = 30;

/// `^MN`: how the printer finds the start of a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    /// Gap or notch between labels (`^MNY`).
    Gap,
    /// Black mark (`^MNM`).
    BlackMark,
    /// Continuous (`^MNN`).
    Continuous,
}

impl Media {
    fn code(self) -> char {
        match self {
            Media::Gap => 'Y',
            Media::BlackMark => 'M',
            Media::Continuous => 'N',
        }
    }
}

/// Settings of a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobSettings {
    /// 0 (lightest) to 30.
    pub darkness: u8,
    pub media: Media,
    pub copies: u16,
}

/// The bytes of one label, printed `copies` times.
pub fn label(view: &RasterView<'_>, settings: &JobSettings) -> Vec<u8> {
    let row_bytes = view.row_bytes();
    let total = row_bytes * view.height() as usize;
    let mut out = String::with_capacity(total * 2 + 96);
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "~SD{:02}\n^XA\n^MN{}\n^PW{}\n^LL{}\n^LH0,0\n^PQ{}\n^FO0,0^GFA,{total},{total},{row_bytes},",
        settings.darkness.min(MAX_DARKNESS),
        settings.media.code(),
        view.width(),
        view.height(),
        settings.copies.max(1),
    );
    for y in 0..view.height() {
        for byte in view.row(y) {
            let _ = write!(out, "{byte:02X}");
        }
    }
    out.push_str("^FS\n^XZ\n");
    out.into_bytes()
}

/// `~HS`: ask for the host status.
pub const STATUS_REQUEST: &[u8] = b"~HS";

/// The flags of a host status reply that Labely uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Status {
    pub paper_out: bool,
    pub paused: bool,
    pub head_up: bool,
    pub ribbon_out: bool,
}

/// Parse a `~HS` reply: three lines, each between STX and ETX. Returns `None` until the
/// first two lines are complete.
pub fn parse_status(reply: &[u8]) -> Option<Status> {
    let text = String::from_utf8_lossy(reply);
    let mut lines = text
        .split('\u{2}')
        .skip(1)
        .filter_map(|l| l.split_once('\u{3}').map(|(body, _)| body));
    let first: Vec<&str> = lines.next()?.split(',').collect();
    let second: Vec<&str> = lines.next()?.split(',').collect();
    let flag = |fields: &[&str], i: usize| fields.get(i).map(|f| f.trim() == "1");
    Some(Status {
        paper_out: flag(&first, 1)?,
        paused: flag(&first, 2)?,
        head_up: flag(&second, 2)?,
        ribbon_out: flag(&second, 3)?,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("unknown command at offset {0}")]
    Unknown(usize),
    #[error("bad arguments in the command at offset {0}")]
    BadArguments(usize),
    #[error("^XZ at offset {0} without a graphic field")]
    NothingToPrint(usize),
    #[error("the stream ends inside a format")]
    Truncated,
}

/// One label as a printer would have printed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedLabel {
    pub raster: Raster,
    pub copies: u32,
    /// `^PW` and `^LL`, in dots.
    pub size: (u32, u32),
    pub media: Option<Media>,
    pub darkness: Option<u8>,
}

fn hex_rows(hex: &str, row_bytes: u32) -> Option<Raster> {
    if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return None;
    }
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16))
        .collect::<Result<_, _>>()
        .ok()?;
    let mut raster = Raster::new(row_bytes * 8);
    raster.push_rows(&bytes);
    Some(raster)
}

/// Decode a stream of the commands this module writes.
pub fn decode(bytes: &[u8]) -> Result<Vec<DecodedLabel>, DecodeError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DecodeError::Unknown(0))?;
    let mut labels = Vec::new();
    let mut darkness = None;
    let mut media = None;
    let mut size = (0, 0);
    let mut copies = 1;
    let mut graphic: Option<Raster> = None;
    let mut open = false;
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let Some(prefix) = rest.chars().next().filter(|c| matches!(c, '^' | '~')) else {
            if rest.starts_with(['\n', '\r', ' ']) {
                i += 1;
                continue;
            }
            return Err(DecodeError::Unknown(i));
        };
        let name = rest.get(1..3).ok_or(DecodeError::Unknown(i))?;
        let end = rest[3..].find(['^', '~']).map_or(rest.len(), |n| n + 3);
        let args = rest[3..end].trim();
        let bad = DecodeError::BadArguments(i);
        match (prefix, name) {
            ('~', "SD") => darkness = Some(args.parse().map_err(|_| bad)?),
            ('^', "XA") => {
                open = true;
                graphic = None;
                copies = 1;
            }
            ('^', "MN") => {
                media = Some(match args {
                    "Y" => Media::Gap,
                    "M" => Media::BlackMark,
                    "N" => Media::Continuous,
                    _ => return Err(bad),
                });
            }
            ('^', "PW") => size.0 = args.parse().map_err(|_| bad)?,
            ('^', "LL") => size.1 = args.parse().map_err(|_| bad)?,
            ('^', "PQ") => copies = args.parse().map_err(|_| bad)?,
            ('^', "LH" | "FO" | "FS") => {}
            ('^', "GF") => {
                let fields: Vec<&str> = args.splitn(5, ',').collect();
                let ["A", total, _, row_bytes, hex] = fields[..] else {
                    return Err(bad);
                };
                let total: usize = total.parse().map_err(|_| DecodeError::BadArguments(i))?;
                let row_bytes: u32 = row_bytes
                    .parse()
                    .map_err(|_| DecodeError::BadArguments(i))?;
                let raster = hex_rows(hex, row_bytes).ok_or(DecodeError::BadArguments(i))?;
                if raster.data().len() != total {
                    return Err(bad);
                }
                graphic = Some(raster);
            }
            ('^', "XZ") => {
                open = false;
                labels.push(DecodedLabel {
                    raster: graphic.take().ok_or(DecodeError::NothingToPrint(i))?,
                    copies,
                    size,
                    media,
                    darkness,
                });
            }
            _ => return Err(DecodeError::Unknown(i)),
        }
        i += end;
    }
    if open {
        return Err(DecodeError::Truncated);
    }
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> JobSettings {
        JobSettings {
            darkness: 15,
            media: Media::Gap,
            copies: 2,
        }
    }

    #[test]
    fn label_bytes() {
        let data = [0xFF, 0xFF, 0x80, 0x1F];
        let view = RasterView::new(12, 2, 2, &data).unwrap();
        assert_eq!(
            String::from_utf8(label(&view, &settings())).unwrap(),
            "~SD15\n^XA\n^MNY\n^PW12\n^LL2\n^LH0,0\n^PQ2\n\
             ^FO0,0^GFA,4,4,2,FFF08010^FS\n^XZ\n"
        );
    }

    #[test]
    fn decodes_back() {
        let data: Vec<u8> = (0..=255u8).collect();
        let view = RasterView::new(64, 32, 8, &data).unwrap();
        let mut s = settings();
        let one = label(&view, &s);
        s.media = Media::Continuous;
        s.copies = 1;
        s.darkness = 99;
        let labels = decode(&[one, label(&view, &s)].concat()).unwrap();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].raster.data(), data.as_slice());
        assert_eq!(labels[0].size, (64, 32));
        assert_eq!((labels[0].copies, labels[1].copies), (2, 1));
        assert_eq!(labels[1].media, Some(Media::Continuous));
        assert_eq!(
            (labels[0].darkness, labels[1].darkness),
            (Some(15), Some(30))
        );
    }

    #[test]
    fn decode_errors() {
        assert_eq!(decode(b"hello"), Err(DecodeError::Unknown(0)));
        assert_eq!(decode(b"^XA^XZ"), Err(DecodeError::NothingToPrint(3)));
        assert_eq!(decode(b"^XA^FO0,0"), Err(DecodeError::Truncated));
        assert_eq!(
            decode(b"^XA^GFA,2,2,1,FF^FS^XZ"),
            Err(DecodeError::BadArguments(3))
        );
        assert_eq!(decode(b"^XA^ZZ1^XZ"), Err(DecodeError::Unknown(3)));
    }

    #[test]
    fn host_status() {
        let reply = b"\x02030,0,0,1245,000,0,0,0,000,0,0,0\x03\r\n\
                      \x02000,0,0,0,1,2,6,0,00000000,1,000\x03\r\n\
                      \x021234,0\x03\r\n";
        assert_eq!(parse_status(reply), Some(Status::default()));
        let out = b"\x02030,1,1,1245,000,0,0,0,000,0,0,0\x03\r\n\
                    \x02000,0,1,1,1,2,6,0,00000000,1,000\x03\r\n";
        assert_eq!(
            parse_status(out),
            Some(Status {
                paper_out: true,
                paused: true,
                head_up: true,
                ribbon_out: true
            })
        );
        assert_eq!(parse_status(b"\x02030,0,0"), None);
    }
}
