//! Barcode encoding (rxing) and decoding of rendered bitmaps.

use std::collections::BTreeSet;

use labely_core::document::{EccLevel, Symbology1d, Symbology2d};
use labely_core::raster::Bitmap1;
use labely_core::services::DecodedCode;
use rxing::common::{BitMatrix, HybridBinarizer};
use rxing::multi::{GenericMultipleBarcodeReader, MultipleBarcodeReader};
use rxing::{
    BarcodeFormat, BinaryBitmap, DecodeHints, EncodeHints, Luma8LuminanceSource, MultiFormatWriter,
    MultiUseMultiFormatReader, Writer,
};

/// White border added around a bitmap before decoding, in dots.
const QUIET_ZONE: u32 = 40;

/// A module grid; `true` is a dark module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Modules {
    pub width: u32,
    pub height: u32,
    pub dark: Vec<bool>,
}

impl Modules {
    pub fn get(&self, x: u32, y: u32) -> bool {
        self.dark[(y * self.width + x) as usize]
    }
}

fn format_1d(s: Symbology1d) -> BarcodeFormat {
    match s {
        Symbology1d::Code128 => BarcodeFormat::CODE_128,
        Symbology1d::Code39 => BarcodeFormat::CODE_39,
        Symbology1d::Code93 => BarcodeFormat::CODE_93,
        Symbology1d::Ean13 => BarcodeFormat::EAN_13,
        Symbology1d::Ean8 => BarcodeFormat::EAN_8,
        Symbology1d::UpcA => BarcodeFormat::UPC_A,
        Symbology1d::UpcE => BarcodeFormat::UPC_E,
        Symbology1d::Itf => BarcodeFormat::ITF,
        Symbology1d::Codabar => BarcodeFormat::CODABAR,
    }
}

fn format_2d(s: Symbology2d) -> BarcodeFormat {
    match s {
        Symbology2d::Qr => BarcodeFormat::QR_CODE,
        Symbology2d::DataMatrix => BarcodeFormat::DATA_MATRIX,
        Symbology2d::Pdf417 => BarcodeFormat::PDF_417,
        Symbology2d::Aztec => BarcodeFormat::AZTEC,
    }
}

/// Our JSON name for a decoded rxing format.
fn format_name(f: &BarcodeFormat) -> String {
    match f {
        BarcodeFormat::CODE_128 => "code128",
        BarcodeFormat::CODE_39 => "code39",
        BarcodeFormat::CODE_93 => "code93",
        BarcodeFormat::EAN_13 => "ean13",
        BarcodeFormat::EAN_8 => "ean8",
        BarcodeFormat::UPC_A => "upc_a",
        BarcodeFormat::UPC_E => "upc_e",
        BarcodeFormat::ITF => "itf",
        BarcodeFormat::CODABAR => "codabar",
        BarcodeFormat::QR_CODE => "qr",
        BarcodeFormat::DATA_MATRIX => "data_matrix",
        BarcodeFormat::PDF_417 => "pdf417",
        BarcodeFormat::AZTEC => "aztec",
        other => return other.to_string().to_lowercase(),
    }
    .to_owned()
}

fn encode(data: &str, format: &BarcodeFormat, hints: &EncodeHints) -> Result<BitMatrix, String> {
    if data.is_empty() {
        return Err("no data".to_owned());
    }
    // 1×1 requested size yields the minimal matrix: one pixel per module.
    MultiFormatWriter
        .encode_with_hints(data, format, 1, 1, hints)
        .map_err(|e| e.to_string())
}

/// Encode a 1D code as a single row of modules, quiet zones trimmed.
pub(crate) fn encode_1d(data: &str, sym: Symbology1d) -> Result<Vec<bool>, String> {
    let hints = EncodeHints {
        Margin: Some("0".to_owned()),
        ..Default::default()
    };
    let m = encode(data, &format_1d(sym), &hints)?;
    let row: Vec<bool> = (0..m.width()).map(|x| m.get(x, 0)).collect();
    let first = row.iter().position(|b| *b);
    let last = row.iter().rposition(|b| *b);
    match (first, last) {
        (Some(a), Some(b)) => Ok(row[a..=b].to_vec()),
        _ => Err("encoder produced an empty symbol".to_owned()),
    }
}

/// Encode a 2D code as a module grid without quiet zone.
pub(crate) fn encode_2d(data: &str, sym: Symbology2d, ecc: EccLevel) -> Result<Modules, String> {
    let mut hints = EncodeHints {
        Margin: Some("0".to_owned()),
        ..Default::default()
    };
    if sym == Symbology2d::Qr {
        let level = match ecc {
            EccLevel::L => "L",
            EccLevel::M => "M",
            EccLevel::Q => "Q",
            EccLevel::H => "H",
        };
        hints.ErrorCorrection = Some(level.to_owned());
    }
    let m = encode(data, &format_2d(sym), &hints)?;
    let (w, h) = (m.width(), m.height());
    if w == 0 || h == 0 {
        return Err("encoder produced an empty symbol".to_owned());
    }
    let mut dark = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            dark.push(m.get(x, y));
        }
    }
    Ok(Modules {
        width: w,
        height: h,
        dark,
    })
}

/// Decode every code in `bitmap` after adding a white quiet zone.
pub(crate) fn decode(bitmap: &Bitmap1) -> Vec<DecodedCode> {
    let (bw, bh) = (bitmap.width(), bitmap.height());
    let (w, h) = (bw + 2 * QUIET_ZONE, bh + 2 * QUIET_ZONE);
    let mut luma = vec![255u8; (w * h) as usize];
    for y in 0..bh {
        for x in 0..bw {
            if bitmap.get(x, y) {
                luma[((y + QUIET_ZONE) * w + x + QUIET_ZONE) as usize] = 0;
            }
        }
    }
    decode_luma(luma, w, h)
}

/// Decode an image file (PNG, JPEG, BMP, WebP); a white margin is added around it.
pub(crate) fn decode_image(bytes: &[u8]) -> Result<Vec<DecodedCode>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let img = if img.width().max(img.height()) > 2400 {
        img.resize(2400, 2400, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let g = img.to_luma8();
    let m = 16;
    let (w, h) = (g.width() + 2 * m, g.height() + 2 * m);
    let mut luma = vec![255u8; (w * h) as usize];
    for (x, y, p) in g.enumerate_pixels() {
        luma[((y + m) * w + x + m) as usize] = p.0[0];
    }
    Ok(decode_luma(luma, w, h))
}

fn decode_luma(luma: Vec<u8>, w: u32, h: u32) -> Vec<DecodedCode> {
    let Ok(source) = Luma8LuminanceSource::new(luma, w, h) else {
        return Vec::new();
    };
    let mut image = BinaryBitmap::new(HybridBinarizer::new(source));
    let hints = DecodeHints {
        TryHarder: Some(true),
        ..Default::default()
    };
    let mut reader = GenericMultipleBarcodeReader::new(MultiUseMultiFormatReader::default());
    let results = reader
        .decode_multiple_with_hints(&mut image, &hints)
        .unwrap_or_default();
    let set: BTreeSet<(String, String)> = results
        .iter()
        .map(|r| (format_name(r.getBarcodeFormat()), r.getText().to_owned()))
        .collect();
    set.into_iter()
        .map(|(format, text)| DecodedCode { format, text })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code128_has_no_quiet_zone() {
        let m = encode_1d("ABC", Symbology1d::Code128).unwrap();
        assert!(m[0] && m[m.len() - 1]);
        // start(11) + 3 data(33) + check(11) + stop(13)
        assert_eq!(m.len(), 68);
    }

    #[test]
    fn ean13_is_95_modules() {
        assert_eq!(
            encode_1d("590123412345", Symbology1d::Ean13).unwrap().len(),
            95
        );
        assert!(encode_1d("12AB", Symbology1d::Ean13).is_err());
    }

    #[test]
    fn qr_is_square_without_margin() {
        let m = encode_2d("hello", Symbology2d::Qr, EccLevel::M).unwrap();
        assert_eq!((m.width, m.height), (21, 21));
        assert!(m.get(0, 0) && m.get(20, 0) && m.get(0, 20));
    }

    #[test]
    fn all_symbologies_encode() {
        let cases: &[(Symbology1d, &str)] = &[
            (Symbology1d::Code128, "Labely-1"),
            (Symbology1d::Code39, "LABELY"),
            (Symbology1d::Code93, "LABELY"),
            (Symbology1d::Ean13, "590123412345"),
            (Symbology1d::Ean8, "9638507"),
            (Symbology1d::UpcA, "03600029145"),
            (Symbology1d::UpcE, "0123456"),
            (Symbology1d::Itf, "123456"),
            (Symbology1d::Codabar, "A123456A"),
        ];
        for (s, d) in cases {
            assert!(encode_1d(d, *s).is_ok(), "{s:?}");
        }
        for s in [
            Symbology2d::Qr,
            Symbology2d::DataMatrix,
            Symbology2d::Pdf417,
            Symbology2d::Aztec,
        ] {
            assert!(encode_2d("Labely 123", s, EccLevel::M).is_ok(), "{s:?}");
        }
    }
}
