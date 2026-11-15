//! Golden-image tests for labely-render.
//!
//! Goldens live in `tests/golden/*.png` (1-bit) and are compared pixel-exactly.
//! Run with `LABELY_BLESS=1` to (re)write them.

// Test helpers outside `#[test]` fns are not covered by clippy's allow-*-in-tests.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;

use base64::Engine as _;
use labely_core::document::*;
use labely_core::raster::Bitmap1;
use labely_core::services::{RenderError, RenderIssue, RenderOptions, RenderOutput, Renderer};
use labely_render::LabelRenderer;

mod check;

// ---------------------------------------------------------------- helpers

struct Doc {
    doc: Document,
}

impl Doc {
    fn new(w: f64, h: f64) -> Self {
        Self {
            doc: Document::new(
                "t",
                LabelSpec {
                    width_mm: w,
                    height_mm: h,
                    ..Default::default()
                },
            ),
        }
    }

    fn add(&mut self, x: f64, y: f64, w: f64, h: f64, rot: f64, kind: ElementKind) -> ElementId {
        let id = self.doc.alloc_id();
        self.doc.pages[0].elements.push(Element {
            id,
            name: None,
            x_mm: x,
            y_mm: y,
            width_mm: w,
            height_mm: h,
            rotation_deg: rot,
            locked: false,
            hidden: false,
            flip_h: false,
            flip_v: false,
            group: None,
            mirror: None,
            kind,
        });
        id
    }
}

fn text(s: &str, pt: f64) -> TextProps {
    TextProps {
        text: s.into(),
        font_size_pt: pt,
        ..Default::default()
    }
}

fn rect(stroke: f64, fill: bool, radius: f64) -> ElementKind {
    ElementKind::Rect(ShapeProps {
        stroke_mm: stroke,
        fill,
        corner_radius_mm: radius,
    })
}

fn barcode(data: &str, s: Symbology1d) -> ElementKind {
    ElementKind::Barcode(BarcodeProps {
        data: data.into(),
        symbology: s,
        human_readable: HumanReadable::Below,
        text_size_pt: 8.0,
    })
}

fn code2d(data: &str, s: Symbology2d) -> ElementKind {
    ElementKind::Code2d(Code2dProps {
        data: data.into(),
        symbology: s,
        ecc: EccLevel::M,
    })
}

fn render(doc: &Document, dpi: u32) -> RenderOutput {
    LabelRenderer::new()
        .render(doc, &RenderOptions::at_dpi(dpi))
        .expect("render")
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"))
}

fn read_png(bytes: &[u8]) -> Bitmap1 {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().expect("png header");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    let mut b = Bitmap1::new(info.width, info.height);
    for y in 0..info.height {
        for x in 0..info.width {
            if buf[y as usize * info.line_size + x as usize] < 128 {
                b.set(x, y, true);
            }
        }
    }
    b
}

/// Compare `bitmap` with the golden `name`, or write it when `LABELY_BLESS=1`.
fn golden(name: &str, bitmap: &Bitmap1) {
    let path = golden_path(name);
    let png = LabelRenderer::new().to_png(bitmap).expect("png");
    if std::env::var("LABELY_BLESS").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().expect("dir")).expect("mkdir");
        std::fs::write(&path, &png).expect("write golden");
        return;
    }
    let want = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {} ({e}); run with LABELY_BLESS=1",
            path.display()
        )
    });
    let want = read_png(&want);
    let got = read_png(&png);
    assert_eq!(
        (got.width(), got.height()),
        (want.width(), want.height()),
        "{name}: size differs"
    );
    if got != want {
        let mut diff = 0;
        for y in 0..got.height() {
            for x in 0..got.width() {
                diff += u32::from(got.get(x, y) != want.get(x, y));
            }
        }
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.actual.png"));
        let _ = std::fs::write(&out, &png);
        panic!(
            "{name}: {diff} pixels differ; actual written to {}",
            out.display()
        );
    }
}

fn decoded(bitmap: &Bitmap1) -> Vec<(String, String)> {
    LabelRenderer::new()
        .decode_codes(bitmap)
        .into_iter()
        .map(|c| (c.format, c.text))
        .collect()
}

fn has_issue(out: &RenderOutput, f: impl Fn(&RenderIssue) -> bool) -> bool {
    out.issues.iter().any(f)
}

/// Test image: horizontal gray ramp with a dark disc and a white square.
fn test_png(w: u32, h: u32) -> String {
    let img = image::GrayImage::from_fn(w, h, |x, y| {
        let (cx, cy) = (x as f64 - w as f64 * 0.35, y as f64 - h as f64 * 0.5);
        let mut v = (x * 255 / (w - 1)) as u8;
        if cx * cx + cy * cy < (h as f64 * 0.25).powi(2) {
            v = 30;
        }
        if x > w * 3 / 4 && y < h / 4 {
            v = 255;
        }
        image::Luma([v])
    });
    let mut bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .expect("png");
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn add_asset(d: &mut Doc, key: &str, w: u32, h: u32) {
    d.doc.assets.insert(
        key.into(),
        Asset {
            mime: "image/png".into(),
            data_base64: test_png(w, h),
        },
    );
}

fn image(asset: &str, fit: ImageFit, dither: Dither) -> ImageProps {
    ImageProps {
        asset: asset.into(),
        fit,
        dither,
        threshold: 128,
        brightness: 0.0,
        contrast: 0.0,
        invert: false,
        crop: None,
    }
}

// ---------------------------------------------------------------- tests

fn alignment_doc() -> Doc {
    let mut d = Doc::new(50.0, 30.0);
    let aligns = [HAlign::Left, HAlign::Center, HAlign::Right];
    let valigns = [VAlign::Top, VAlign::Middle, VAlign::Bottom];
    for (row, va) in valigns.iter().enumerate() {
        for (col, ha) in aligns.iter().enumerate() {
            let (x, y) = (1.0 + col as f64 * 16.0, 1.0 + row as f64 * 9.5);
            d.add(x, y, 16.0, 9.5, 0.0, rect(0.2, false, 0.0));
            let t = TextProps {
                align: *ha,
                valign: *va,
                ..text("Ag", 9.0)
            };
            d.add(x + 0.5, y + 0.5, 15.0, 8.5, 0.0, ElementKind::Text(t));
        }
    }
    d
}

#[test]
fn text_alignments_203dpi() {
    let out = render(&alignment_doc().doc, 203);
    assert_eq!((out.bitmap.width(), out.bitmap.height()), (400, 240));
    assert!(out.issues.is_empty(), "{:?}", out.issues);
    golden("text_align_203", &out.bitmap);
}

#[test]
fn text_alignments_300dpi() {
    let out = render(&alignment_doc().doc, 300);
    assert_eq!((out.bitmap.width(), out.bitmap.height()), (591, 354));
    assert!(out.issues.is_empty(), "{:?}", out.issues);
    golden("text_align_300", &out.bitmap);
}

#[test]
fn text_styles() {
    let mut d = Doc::new(60.0, 40.0);
    d.add(
        1.0,
        1.0,
        28.0,
        6.0,
        0.0,
        ElementKind::Text(TextProps {
            bold: true,
            ..text("Bold", 12.0)
        }),
    );
    d.add(
        31.0,
        1.0,
        28.0,
        6.0,
        0.0,
        ElementKind::Text(TextProps {
            italic: true,
            ..text("Italic", 12.0)
        }),
    );
    d.add(
        1.0,
        8.0,
        28.0,
        6.0,
        0.0,
        ElementKind::Text(TextProps {
            underline: true,
            ..text("Under", 12.0)
        }),
    );
    d.add(
        31.0,
        8.0,
        28.0,
        6.0,
        0.0,
        ElementKind::Text(TextProps {
            letter_spacing_mm: 1.0,
            ..text("Wide", 12.0)
        }),
    );
    d.add(
        1.0,
        15.0,
        28.0,
        6.0,
        0.0,
        ElementKind::Text(TextProps {
            invert: true,
            align: HAlign::Center,
            valign: VAlign::Middle,
            ..text("Invert", 12.0)
        }),
    );
    let fallback = d.add(
        31.0,
        15.0,
        28.0,
        6.0,
        0.0,
        ElementKind::Text(TextProps {
            font_family: "No Such Font".into(),
            ..text("Hi {{name}} {{x}}", 8.0)
        }),
    );
    d.add(
        1.0,
        22.0,
        58.0,
        17.0,
        0.0,
        ElementKind::Text(TextProps {
            line_spacing: 1.5,
            ..text(
                "Wrapped paragraph text that runs over several lines of the box.",
                10.0,
            )
        }),
    );
    let mut opts = RenderOptions::at_dpi(203);
    opts.values.insert("name".into(), "Ada".into());
    let out = LabelRenderer::new().render(&d.doc, &opts).expect("render");
    assert_eq!(
        out.issues,
        vec![RenderIssue::FontFallback {
            element_id: fallback,
            requested: "No Such Font".into()
        }]
    );
    golden("text_styles_203", &out.bitmap);
}

#[test]
fn text_variables_fall_back_to_document() {
    let mut d = Doc::new(30.0, 10.0);
    d.doc.variables.insert("name".into(), "Ada".into());
    d.add(
        0.0,
        0.0,
        30.0,
        10.0,
        0.0,
        ElementKind::Text(text("{{name}}", 12.0)),
    );
    let a = render(&d.doc, 203);
    let mut e = Doc::new(30.0, 10.0);
    e.add(
        0.0,
        0.0,
        30.0,
        10.0,
        0.0,
        ElementKind::Text(text("Ada", 12.0)),
    );
    assert_eq!(a.bitmap, render(&e.doc, 203).bitmap);
}

#[test]
fn shrink_to_fit() {
    let mut d = Doc::new(50.0, 30.0);
    let long = "A fairly long product name that will not fit";
    let shrunk = d.add(
        1.0,
        1.0,
        48.0,
        12.0,
        0.0,
        ElementKind::Text(TextProps {
            fit: TextFit::Shrink,
            valign: VAlign::Middle,
            ..text(long, 24.0)
        }),
    );
    d.add(1.0, 1.0, 48.0, 12.0, 0.0, rect(0.2, false, 0.0));
    let over = d.add(
        1.0,
        15.0,
        48.0,
        6.0,
        0.0,
        ElementKind::Text(TextProps {
            wrap: false,
            ..text(long, 12.0)
        }),
    );
    d.add(
        1.0,
        22.0,
        48.0,
        7.0,
        0.0,
        ElementKind::Text(TextProps {
            fit: TextFit::Shrink,
            wrap: false,
            align: HAlign::Center,
            ..text("SHRINK ONE LINE", 40.0)
        }),
    );
    let out = render(&d.doc, 300);
    let size = out.issues.iter().find_map(|i| match i {
        RenderIssue::TextShrunk {
            element_id,
            font_size_pt,
        } if *element_id == shrunk => Some(*font_size_pt),
        _ => None,
    });
    let size = size.expect("TextShrunk");
    assert!(size < 24.0 && size > 6.0, "{size}");
    assert!(has_issue(&out, |i| *i
        == RenderIssue::TextOverflow { element_id: over }));
    assert!(!has_issue(&out, |i| *i
        == RenderIssue::TextOverflow { element_id: shrunk }));
    assert_eq!(
        out.issues
            .iter()
            .filter(|i| matches!(i, RenderIssue::TextShrunk { .. }))
            .count(),
        2
    );
    golden("text_shrink_300", &out.bitmap);
}

#[test]
fn codes_round_trip() {
    let mut d = Doc::new(80.0, 50.0);
    let c128 = d.add(
        2.0,
        2.0,
        40.0,
        15.0,
        0.0,
        barcode("LABELY-128", Symbology1d::Code128),
    );
    d.add(
        2.0,
        25.0,
        40.0,
        22.0,
        0.0,
        barcode("590123412345", Symbology1d::Ean13),
    );
    d.add(
        50.0,
        2.0,
        22.0,
        22.0,
        0.0,
        code2d("https://labely.example/q", Symbology2d::Qr),
    );
    d.add(
        52.0,
        30.0,
        16.0,
        16.0,
        0.0,
        code2d("DM-0042", Symbology2d::DataMatrix),
    );
    let out = render(&d.doc, 300);
    let widths: Vec<f64> = out
        .issues
        .iter()
        .filter_map(|i| match i {
            RenderIssue::ModuleWidth { dots, .. } => Some(*dots),
            _ => None,
        })
        .collect();
    assert_eq!(widths.len(), 4, "{:?}", out.issues);
    assert!(
        widths.iter().all(|w| *w >= 2.0 && w.fract() == 0.0),
        "{widths:?}"
    );
    assert!(has_issue(
        &out,
        |i| matches!(i, RenderIssue::ModuleWidth { element_id, .. } if *element_id == c128)
    ));
    let codes = decoded(&out.bitmap);
    for want in [
        ("code128", "LABELY-128"),
        ("ean13", "5901234123457"),
        ("qr", "https://labely.example/q"),
        ("data_matrix", "DM-0042"),
    ] {
        assert!(
            codes.iter().any(|(f, t)| f == want.0 && t == want.1),
            "{want:?} not in {codes:?}"
        );
    }
    golden("codes_300", &out.bitmap);
}

#[test]
fn every_symbology_decodes() {
    let one_d: &[(Symbology1d, &str, &str, &str)] = &[
        (Symbology1d::Code128, "Labely-1", "code128", "Labely-1"),
        (Symbology1d::Code39, "LABELY", "code39", "LABELY"),
        (Symbology1d::Code93, "LABELY", "code93", "LABELY"),
        (Symbology1d::Ean13, "590123412345", "ean13", "5901234123457"),
        (Symbology1d::Ean8, "9638507", "ean8", "96385074"),
        (Symbology1d::UpcA, "03600029145", "upc_a", "036000291452"),
        (Symbology1d::UpcE, "0123456", "upc_e", "01234565"),
        (Symbology1d::Itf, "123456", "itf", "123456"),
        (Symbology1d::Codabar, "A123456A", "codabar", "123456"),
        (Symbology1d::Code128A, "LABELY-1", "code128", "LABELY-1"),
        (Symbology1d::Code128B, "Labely-1", "code128", "Labely-1"),
        (Symbology1d::Code128C, "12345678", "code128", "12345678"),
        (
            Symbology1d::Gs1128,
            "(01)09501101530003(17)250101",
            "code128",
            "010950110153000317250101",
        ),
    ];
    for (sym, data, fmt, text) in one_d {
        let mut d = Doc::new(60.0, 20.0);
        d.add(2.0, 2.0, 56.0, 16.0, 0.0, barcode(data, *sym));
        let out = render(&d.doc, 300);
        let codes = decoded(&out.bitmap);
        assert!(
            codes
                .iter()
                .any(|(f, t)| f == fmt && (t == text || t.contains(text))),
            "{sym:?}: {codes:?} {:?}",
            out.issues
        );
    }
    for (sym, fmt) in [
        (Symbology2d::Qr, "qr"),
        (Symbology2d::DataMatrix, "data_matrix"),
        (Symbology2d::Pdf417, "pdf417"),
        (Symbology2d::Aztec, "aztec"),
    ] {
        let mut d = Doc::new(50.0, 30.0);
        d.add(2.0, 2.0, 46.0, 26.0, 0.0, code2d("Labely 2D 123", sym));
        let out = render(&d.doc, 300);
        let codes = decoded(&out.bitmap);
        assert!(
            codes.iter().any(|(f, t)| f == fmt && t == "Labely 2D 123"),
            "{sym:?}: {codes:?} {:?}",
            out.issues
        );
    }
}

#[test]
fn encode_failures() {
    let mut d = Doc::new(50.0, 30.0);
    let bad = d.add(
        1.0,
        1.0,
        30.0,
        10.0,
        0.0,
        barcode("12AB", Symbology1d::Ean13),
    );
    let tiny = d.add(
        35.0,
        1.0,
        2.0,
        2.0,
        0.0,
        code2d("a long text that needs many modules", Symbology2d::Qr),
    );
    let empty = d.add(
        1.0,
        15.0,
        30.0,
        10.0,
        0.0,
        barcode("", Symbology1d::Code128),
    );
    let out = render(&d.doc, 203);
    let failed = |id: ElementId| {
        has_issue(
            &out,
            |i| matches!(i, RenderIssue::EncodeFailed { element_id, .. } if *element_id == id),
        )
    };
    assert!(
        failed(bad) && failed(tiny) && failed(empty),
        "{:?}",
        out.issues
    );
    assert!(has_issue(
        &out,
        |i| matches!(i, RenderIssue::ModuleWidth { element_id, dots } if *element_id == tiny && *dots < 1.0)
    ));
    assert_eq!(out.bitmap.count_black(), 0);
}

#[test]
fn image_dithers() {
    let mut d = Doc::new(50.0, 30.0);
    add_asset(&mut d, "img", 96, 64);
    for (i, m) in [
        Dither::Threshold,
        Dither::FloydSteinberg,
        Dither::Atkinson,
        Dither::Bayer,
    ]
    .into_iter()
    .enumerate()
    {
        let (x, y) = (1.0 + (i % 2) as f64 * 24.5, 1.0 + (i / 2) as f64 * 14.5);
        d.add(
            x,
            y,
            23.5,
            13.5,
            0.0,
            ElementKind::Image(image("img", ImageFit::Stretch, m)),
        );
    }
    let out = render(&d.doc, 203);
    assert!(out.issues.is_empty(), "{:?}", out.issues);
    golden("image_dither_203", &out.bitmap);
}

#[test]
fn image_fit_and_adjust() {
    let mut d = Doc::new(60.0, 30.0);
    add_asset(&mut d, "img", 64, 32);
    let fits = [ImageFit::Contain, ImageFit::Cover, ImageFit::Stretch];
    for (i, fit) in fits.into_iter().enumerate() {
        let x = 1.0 + i as f64 * 20.0;
        d.add(x, 1.0, 18.0, 13.0, 0.0, rect(0.2, false, 0.0));
        d.add(
            x,
            1.0,
            18.0,
            13.0,
            0.0,
            ElementKind::Image(image("img", fit, Dither::FloydSteinberg)),
        );
    }
    let adjusted = [
        ImageProps {
            brightness: 0.3,
            ..image("img", ImageFit::Stretch, Dither::Atkinson)
        },
        ImageProps {
            contrast: 0.6,
            ..image("img", ImageFit::Stretch, Dither::Atkinson)
        },
        ImageProps {
            invert: true,
            ..image("img", ImageFit::Stretch, Dither::Atkinson)
        },
    ];
    for (i, p) in adjusted.into_iter().enumerate() {
        d.add(
            1.0 + i as f64 * 20.0,
            16.0,
            18.0,
            13.0,
            0.0,
            ElementKind::Image(p),
        );
    }
    let missing = d.add(
        0.0,
        0.0,
        5.0,
        5.0,
        0.0,
        ElementKind::Image(image("nope", ImageFit::Contain, Dither::Bayer)),
    );
    d.doc.assets.insert(
        "broken".into(),
        Asset {
            mime: "image/png".into(),
            data_base64: "AAAA".into(),
        },
    );
    let broken = d.add(
        0.0,
        0.0,
        5.0,
        5.0,
        0.0,
        ElementKind::Image(image("broken", ImageFit::Contain, Dither::Bayer)),
    );
    let out = render(&d.doc, 203);
    for id in [missing, broken] {
        assert!(has_issue(
            &out,
            |i| matches!(i, RenderIssue::BadAsset { element_id, .. } if *element_id == id)
        ));
    }
    assert_eq!(out.issues.len(), 2, "{:?}", out.issues);
    golden("image_fit_203", &out.bitmap);
}

#[test]
fn shapes() {
    let mut d = Doc::new(50.0, 30.0);
    d.add(1.0, 1.0, 14.0, 9.0, 0.0, rect(0.3, false, 0.0));
    d.add(17.0, 1.0, 14.0, 9.0, 0.0, rect(0.6, false, 2.5));
    d.add(33.0, 1.0, 14.0, 9.0, 0.0, rect(0.3, true, 1.5));
    d.add(
        1.0,
        12.0,
        14.0,
        9.0,
        0.0,
        ElementKind::Ellipse(ShapeProps {
            stroke_mm: 0.4,
            ..Default::default()
        }),
    );
    d.add(
        17.0,
        12.0,
        9.0,
        9.0,
        0.0,
        ElementKind::Ellipse(ShapeProps {
            fill: true,
            ..Default::default()
        }),
    );
    d.add(
        28.0,
        12.0,
        20.0,
        0.0,
        0.0,
        ElementKind::Line(LineProps {
            stroke_mm: 0.5,
            dash_mm: None,
        }),
    );
    d.add(
        28.0,
        15.0,
        20.0,
        0.0,
        0.0,
        ElementKind::Line(LineProps {
            stroke_mm: 0.3,
            dash_mm: Some([1.0, 0.5]),
        }),
    );
    d.add(
        28.0,
        17.0,
        20.0,
        4.0,
        0.0,
        ElementKind::Line(LineProps {
            stroke_mm: 0.3,
            dash_mm: None,
        }),
    );
    d.add(
        1.0,
        23.0,
        46.0,
        0.0,
        0.0,
        ElementKind::Line(LineProps {
            stroke_mm: 1.0,
            dash_mm: Some([2.0, 2.0]),
        }),
    );
    // Hidden elements are skipped.
    let id = d.add(0.0, 0.0, 50.0, 30.0, 0.0, rect(1.0, true, 0.0));
    d.doc.element_mut(id).expect("el").hidden = true;
    let out = render(&d.doc, 203);
    assert!(out.issues.is_empty());
    golden("shapes_203", &out.bitmap);
}

#[test]
fn rotated_elements() {
    let mut d = Doc::new(50.0, 30.0);
    d.add(
        1.0,
        10.0,
        20.0,
        8.0,
        90.0,
        ElementKind::Text(TextProps {
            align: HAlign::Center,
            valign: VAlign::Middle,
            ..text("Rot 90", 12.0)
        }),
    );
    d.add(18.0, 3.0, 14.0, 8.0, 30.0, rect(0.4, false, 1.0));
    d.add(
        18.0,
        3.0,
        14.0,
        8.0,
        30.0,
        ElementKind::Text(TextProps {
            align: HAlign::Center,
            valign: VAlign::Middle,
            ..text("Rot 30", 9.0)
        }),
    );
    d.add(
        20.0,
        18.0,
        16.0,
        0.0,
        30.0,
        ElementKind::Line(LineProps {
            stroke_mm: 0.4,
            dash_mm: None,
        }),
    );
    d.add(
        36.0,
        14.0,
        13.0,
        13.0,
        90.0,
        code2d("rotated qr", Symbology2d::Qr),
    );
    let out = render(&d.doc, 300);
    let codes = decoded(&out.bitmap);
    assert!(
        codes.iter().any(|(f, t)| f == "qr" && t == "rotated qr"),
        "{codes:?}"
    );
    golden("rotated_300", &out.bitmap);
}

#[test]
fn b21_pro_label() {
    let mut d = Doc::new(50.0, 30.0);
    d.doc.label.printer_model = Some("niimbot.b21_pro".into());
    d.add(
        2.0,
        1.5,
        46.0,
        7.0,
        0.0,
        ElementKind::Text(TextProps {
            bold: true,
            fit: TextFit::Shrink,
            ..text("{{product}}", 14.0)
        }),
    );
    d.add(
        2.0,
        9.0,
        30.0,
        4.5,
        0.0,
        ElementKind::Text(text("SKU {{sku}}", 8.0)),
    );
    d.add(
        2.0,
        15.0,
        30.0,
        13.5,
        0.0,
        barcode("{{sku}}", Symbology1d::Code128),
    );
    d.add(
        34.0,
        12.0,
        14.0,
        14.0,
        0.0,
        code2d("https://labely.example/p/{{sku}}", Symbology2d::Qr),
    );
    d.add(0.5, 0.5, 49.0, 29.0, 0.0, rect(0.3, false, 2.0));
    let mut opts = RenderOptions::at_dpi(300);
    opts.values
        .insert("product".into(), "Organic Coffee Beans".into());
    opts.values.insert("sku".into(), "CB-2026-001".into());
    let r = LabelRenderer::new();
    let out = r.render(&d.doc, &opts).expect("render");
    // 50 mm × 30 mm at 300 dpi = round(590.55) × round(354.33).
    assert_eq!((out.bitmap.width(), out.bitmap.height()), (591, 354));
    let codes = decoded(&out.bitmap);
    assert!(
        codes.contains(&("code128".into(), "CB-2026-001".into())),
        "{codes:?}"
    );
    assert!(
        codes.contains(&("qr".into(), "https://labely.example/p/CB-2026-001".into())),
        "{codes:?}"
    );
    golden("b21_pro_50x30_300", &out.bitmap);
    // Deterministic and the gray preview matches the label size.
    assert_eq!(r.render(&d.doc, &opts).expect("render").bitmap, out.bitmap);
    let gray = r.render_gray(&d.doc, &opts).expect("gray");
    assert_eq!(
        (gray.width, gray.height, gray.data.len()),
        (591, 354, 591 * 354)
    );
}

#[test]
fn render_errors() {
    let r = LabelRenderer::new();
    let d = Doc::new(50.0, 30.0);
    let opts = RenderOptions {
        page: 3,
        ..RenderOptions::at_dpi(203)
    };
    assert!(matches!(
        r.render(&d.doc, &opts),
        Err(RenderError::NoSuchPage(3))
    ));
    let z = Doc::new(0.0, 30.0);
    assert!(matches!(
        r.render(&z.doc, &RenderOptions::at_dpi(203)),
        Err(RenderError::BadSize { .. })
    ));
}

#[test]
fn png_round_trip() {
    let mut d = Doc::new(20.0, 10.0);
    d.add(1.0, 1.0, 18.0, 8.0, 0.0, rect(0.5, false, 0.0));
    let out = render(&d.doc, 203);
    let png = LabelRenderer::new().to_png(&out.bitmap).expect("png");
    assert_eq!(read_png(&png), out.bitmap);
}

// ---------------------------------------------------------------- phase 3 elements

/// Black dots in the box `[x0, x1) × [y0, y1)`.
fn black_in(b: &Bitmap1, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .filter(|&(x, y)| b.get(x, y))
        .count()
}

#[test]
fn flip_mirrors_inside_the_frame() {
    // A diagonal from the top-left to the bottom-right of a 20×20 mm frame.
    let line = || {
        ElementKind::Line(LineProps {
            stroke_mm: 1.0,
            dash_mm: None,
        })
    };
    let mut d = Doc::new(20.0, 20.0);
    d.add(0.0, 0.0, 20.0, 20.0, 0.0, line());
    let plain = render(&d.doc, 203).bitmap;
    let n = plain.width();
    assert!(black_in(&plain, 0, 0, 20, 20) > 0);
    assert_eq!(black_in(&plain, n - 20, 0, n, 20), 0);

    // flip_h mirrors it into a top-right to bottom-left diagonal.
    let mut d = Doc::new(20.0, 10.0);
    let id = d.add(0.0, 0.0, 20.0, 10.0, 0.0, line());
    d.doc.element_mut(id).unwrap().flip_h = true;
    let flipped = render(&d.doc, 203).bitmap;
    let w = flipped.width();
    assert!(black_in(&flipped, w - 20, 0, w, 15) > 0, "starts top-right");
    assert_eq!(black_in(&flipped, 0, 0, 15, 15), 0, "top-left is empty");

    // Text: flip_v moves the ink from the top half to the bottom half of the frame.
    let mut d = Doc::new(30.0, 20.0);
    let id = d.add(
        0.0,
        0.0,
        30.0,
        20.0,
        0.0,
        ElementKind::Text(text("Ab", 14.0)),
    );
    let up = render(&d.doc, 203).bitmap;
    d.doc.element_mut(id).unwrap().flip_v = true;
    let down = render(&d.doc, 203).bitmap;
    let (w, h) = (up.width(), up.height());
    assert!(black_in(&up, 0, 0, w, h / 2) > black_in(&up, 0, h / 2, w, h));
    assert!(black_in(&down, 0, h / 2, w, h) > black_in(&down, 0, 0, w, h / 2));
    assert_eq!(
        black_in(&up, 0, 0, w, h),
        black_in(&down, 0, 0, w, h),
        "same ink, mirrored"
    );
}

#[test]
fn vertical_text_stacks_characters() {
    let mut d = Doc::new(30.0, 30.0);
    let mut t = text("ABC", 12.0);
    t.direction = TextDirection::Vertical;
    d.add(0.0, 0.0, 30.0, 30.0, 0.0, ElementKind::Text(t));
    let b = render(&d.doc, 203).bitmap;
    let (w, h) = (b.width(), b.height());
    // Ink in a narrow column at the left, reaching well down the label.
    assert_eq!(black_in(&b, w / 2, 0, w, h), 0);
    assert!(black_in(&b, 0, h / 3, w / 2, h) > 0);
}

#[test]
fn table_lines_skip_merged_cells() {
    let mut t = TableProps::new(2, 2);
    t.stroke_mm = 0.5;
    let mut d = Doc::new(20.0, 10.0);
    d.add(0.0, 0.0, 20.0, 10.0, 0.0, ElementKind::Table(t.clone()));
    let open = render(&d.doc, 203).bitmap;
    let (w, h) = (open.width(), open.height());
    // The middle vertical line is there in both rows.
    assert!(black_in(&open, w / 2 - 3, h / 4, w / 2 + 3, h / 4 + 1) > 0);
    t.merges.push(CellRange {
        row: 0,
        col: 0,
        rows: 1,
        cols: 2,
    });
    let mut d = Doc::new(20.0, 10.0);
    d.add(0.0, 0.0, 20.0, 10.0, 0.0, ElementKind::Table(t));
    let merged = render(&d.doc, 203).bitmap;
    assert_eq!(black_in(&merged, w / 2 - 3, h / 4, w / 2 + 3, h / 4 + 1), 0);
    assert!(black_in(&merged, w / 2 - 3, h * 3 / 4, w / 2 + 3, h * 3 / 4 + 1) > 0);
}

#[test]
fn table_text_and_inverted_cells() {
    let mut t = TableProps::new(1, 2);
    t.cells[0][0].text = "Hi".into();
    t.cells[0][1].invert = true;
    t.cells[0][1].text = "Yo".into();
    let mut d = Doc::new(30.0, 10.0);
    d.add(0.0, 0.0, 30.0, 10.0, 0.0, ElementKind::Table(t));
    let out = render(&d.doc, 203);
    let b = out.bitmap;
    let (w, h) = (b.width(), b.height());
    let left = black_in(&b, 4, 4, w / 2 - 4, h - 4);
    let right = black_in(&b, w / 2 + 4, 4, w - 4, h - 4);
    let area = ((w / 2 - 8) * (h - 8)) as usize;
    assert!(left > 0 && left < area / 3, "dark text on white: {left}");
    assert!(
        right > area / 2 && right < area,
        "white text on black: {right}/{area}"
    );
}

#[test]
fn icons_and_borders_draw_inside_their_frame() {
    let r = LabelRenderer::new();
    let icons = r.icons();
    assert!(icons.len() >= 25);
    assert!(
        icons
            .iter()
            .any(|i| i.name == "fragile" && i.category == "handling")
    );
    let mut d = Doc::new(30.0, 30.0);
    d.add(
        5.0,
        5.0,
        10.0,
        10.0,
        0.0,
        ElementKind::Icon(IconProps {
            icon: "check".into(),
            keep_aspect: true,
            invert: false,
        }),
    );
    let out = render(&d.doc, 203);
    let b = &out.bitmap;
    let ink = black_in(b, 0, 0, b.width(), b.height());
    assert!(ink > 50);
    assert_eq!(ink, black_in(b, 40, 40, 120, 120), "only inside the frame");
    let mut d = Doc::new(30.0, 30.0);
    d.add(
        0.0,
        0.0,
        10.0,
        10.0,
        0.0,
        ElementKind::Icon(IconProps {
            icon: "nope".into(),
            keep_aspect: true,
            invert: false,
        }),
    );
    let out = render(&d.doc, 203);
    assert!(matches!(out.issues[0], RenderIssue::BadAsset { .. }));

    for style in [
        BorderStyle::Single,
        BorderStyle::Double,
        BorderStyle::ThickThin,
        BorderStyle::Dashed,
        BorderStyle::Dotted,
        BorderStyle::Corners,
        BorderStyle::Ticket,
        BorderStyle::Scallop,
    ] {
        let mut d = Doc::new(30.0, 20.0);
        d.add(
            0.0,
            0.0,
            30.0,
            20.0,
            0.0,
            ElementKind::Border(BorderProps {
                style,
                ..Default::default()
            }),
        );
        let b = render(&d.doc, 203).bitmap;
        let (w, h) = (b.width(), b.height());
        assert!(black_in(&b, 0, 0, w, h) > 0, "{style:?} draws");
        assert_eq!(
            black_in(&b, w / 4, h / 4, w * 3 / 4, h * 3 / 4),
            0,
            "{style:?} leaves the middle empty"
        );
    }
}

#[test]
fn decode_image_reads_a_rendered_qr() {
    let mut d = Doc::new(30.0, 30.0);
    d.add(
        5.0,
        5.0,
        20.0,
        20.0,
        0.0,
        code2d("labely decode", Symbology2d::Qr),
    );
    let r = LabelRenderer::new();
    let out = render(&d.doc, 203);
    let png = r.to_png(&out.bitmap).unwrap();
    let found = r.decode_image(&png).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].format.as_str(), found[0].text.as_str()),
        ("qr", "labely decode")
    );
    assert!(r.decode_image(b"not an image").is_err());
}

#[test]
fn image_asset_bound_to_a_record() {
    let mut d = Doc::new(20.0, 20.0);
    add_asset(&mut d, "small", 8, 8);
    add_asset(&mut d, "wide", 40, 8);
    let id = d.add(
        0.0,
        0.0,
        20.0,
        20.0,
        0.0,
        ElementKind::Image(image("{{image}}", ImageFit::Contain, Dither::Threshold)),
    );
    let mut opts = RenderOptions::at_dpi(203);
    let mut black = Vec::new();
    for key in ["small", "wide"] {
        opts.values.insert("image".into(), key.into());
        let out = LabelRenderer::new().render(&d.doc, &opts).expect("render");
        assert!(out.issues.is_empty(), "{key}: {:?}", out.issues);
        black.push(out.bitmap.count_black());
    }
    assert_ne!(black[0], black[1], "each record shows its own image");
    // Unfilled binding: reported like a missing asset.
    let out = render(&d.doc, 203);
    assert!(has_issue(
        &out,
        |i| matches!(i, RenderIssue::BadAsset { element_id, .. } if *element_id == id)
    ));
}

// ---------------------------------------------------------------- 3.6c text and data

fn one_text(p: TextProps) -> (RenderOutput, ElementId) {
    let mut d = Doc::new(50.0, 20.0);
    let id = d.add(1.0, 1.0, 48.0, 18.0, 0.0, ElementKind::Text(p));
    (render(&d.doc, 203), id)
}

#[test]
fn fill_grows_text_to_the_box() {
    let (fixed, _) = one_text(text("AB", 8.0));
    let (filled, id) = one_text(TextProps {
        fit: TextFit::Fill,
        ..text("AB", 8.0)
    });
    assert!(filled.bitmap.count_black() > 4 * fixed.bitmap.count_black());
    assert!(!has_issue(&filled, |i| matches!(
        i,
        RenderIssue::TextOverflow { .. } | RenderIssue::TextShrunk { .. }
    )));
    // Too much text for the set size: fill shrinks it like `shrink`.
    let (shrunk, _) = one_text(TextProps {
        fit: TextFit::Fill,
        wrap: false,
        ..text("A long line that cannot fit", 40.0)
    });
    assert!(has_issue(&shrunk, |i| matches!(
        i,
        RenderIssue::TextShrunk { element_id, .. } if *element_id == id
    )));
}

#[test]
fn spans_style_part_of_the_text() {
    let (plain, _) = one_text(text("aaaa bbbb", 12.0));
    let span = |sp: TextSpan| TextProps {
        spans: vec![sp],
        ..text("aaaa bbbb", 12.0)
    };
    let (bold, _) = one_text(span(TextSpan {
        start: 0,
        end: 4,
        bold: Some(true),
        ..Default::default()
    }));
    let (big, _) = one_text(span(TextSpan {
        start: 5,
        end: 9,
        font_size_pt: Some(24.0),
        ..Default::default()
    }));
    let (p, b, g) = (
        plain.bitmap.count_black(),
        bold.bitmap.count_black(),
        big.bitmap.count_black(),
    );
    assert!(b > p && g > b, "{p} {b} {g}");
    // A span beyond the text is clamped, and a binding inside one piece is filled in.
    let mut d = Doc::new(50.0, 20.0);
    d.doc.variables.insert("x".into(), "ZZ".into());
    let a = d.add(
        1.0,
        1.0,
        48.0,
        18.0,
        0.0,
        ElementKind::Text(TextProps {
            spans: vec![TextSpan {
                start: 2,
                end: 99,
                italic: Some(true),
                ..Default::default()
            }],
            ..text("a {{x}}", 12.0)
        }),
    );
    let out = render(&d.doc, 203);
    assert!(out.issues.is_empty(), "{:?} for {a:?}", out.issues);
    let mut e = Doc::new(50.0, 20.0);
    e.add(
        1.0,
        1.0,
        48.0,
        18.0,
        0.0,
        ElementKind::Text(TextProps {
            italic: false,
            ..text("a {{x}}", 12.0)
        }),
    );
    assert_ne!(out.bitmap, render(&e.doc, 203).bitmap);
}

#[test]
fn max_length_guard_reports_long_text() {
    let (out, id) = one_text(TextProps {
        wrap: false,
        max_length_mm: Some(10.0),
        ..text("Hello world", 12.0)
    });
    let found = out.issues.iter().find_map(|i| match i {
        RenderIssue::TextTooLong {
            element_id,
            length_mm,
            max_length_mm,
        } if *element_id == id => Some((*length_mm, *max_length_mm)),
        _ => None,
    });
    let (len, max) = found.expect("TextTooLong");
    assert!(len > 15.0 && max == 10.0, "{len}");
    let (ok, _) = one_text(TextProps {
        max_length_mm: Some(40.0),
        ..text("Hello", 12.0)
    });
    assert!(!has_issue(&ok, |i| matches!(
        i,
        RenderIssue::TextTooLong { .. }
    )));
}

#[test]
fn crop_uses_part_of_the_image() {
    let draw = |crop: Option<Crop>| {
        let mut d = Doc::new(30.0, 20.0);
        add_asset(&mut d, "img", 64, 64);
        d.add(
            0.0,
            0.0,
            20.0,
            20.0,
            0.0,
            ElementKind::Image(ImageProps {
                crop,
                ..image("img", ImageFit::Stretch, Dither::Threshold)
            }),
        );
        render(&d.doc, 203)
    };
    let full = draw(None);
    let right = draw(Some(Crop {
        left: 0.5,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    }));
    assert_ne!(full.bitmap, right.bitmap);
    assert!(
        right
            .issues
            .iter()
            .all(|i| !matches!(i, RenderIssue::BadAsset { .. }))
    );
    // An invalid crop is ignored rather than failing the render.
    let bad = draw(Some(Crop {
        left: 0.8,
        top: 0.0,
        right: 0.2,
        bottom: 1.0,
    }));
    assert_eq!(bad.bitmap, full.bitmap);
}

#[test]
fn font_list_has_the_bundled_family() {
    let fonts = LabelRenderer::new().fonts();
    assert!(fonts.iter().any(|f| f == DEFAULT_FONT), "{fonts:?}");
}
