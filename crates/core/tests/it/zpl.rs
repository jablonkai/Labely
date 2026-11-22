//! ZPL import through `doc.open` (ADR-0044).

use base64::Engine as _;
use labely_core::ErrorCode;
use serde_json::{Value, json};

use crate::{elements, err, ok};

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/zpl/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Dots at 8 dots/mm (the default 203.2 dpi) to mm.
fn mm(dots: f64) -> f64 {
    dots / 8.0
}

fn close(a: &Value, b: f64) -> bool {
    a.as_f64().is_some_and(|a| (a - b).abs() < 1e-6)
}

fn frame(e: &Value, x: f64, y: f64, w: f64, h: f64) -> bool {
    close(&e["x_mm"], mm(x))
        && close(&e["y_mm"], mm(y))
        && close(&e["width_mm"], mm(w))
        && close(&e["height_mm"], mm(h))
}

/// Font size whose 1.2 line height is `dots` high.
fn pt(dots: f64) -> f64 {
    mm(dots / 1.2) * 72.0 / 25.4
}

fn open(name: &str) -> (labely_core::Session, Vec<String>) {
    let mut s = labely_testkit::test_session();
    let e = s.execute("doc.open", json!({"path": fixture(name)}));
    assert!(e.ok, "{:?}", e.error);
    (s, e.warnings)
}

#[test]
fn shipping_label_maps_text_boxes_and_barcode() {
    let (mut s, warnings) = open("shipping_4x6.zpl");
    assert!(warnings[0].contains("ZPL at 203 dpi"), "{warnings:?}");
    for w in [
        "`^XG` is not supported and was skipped",
        "a white box (`^GB`) was skipped",
        "the file holds 2 labels; only the first was opened",
    ] {
        assert!(warnings.iter().any(|x| x == w), "{w}: {warnings:?}");
    }
    let g = ok(&mut s, "doc.get", json!({}));
    assert_eq!(g["dirty"], true);
    assert_eq!(g["path"], Value::Null);
    let doc = serde_json::to_value(s.doc(None).unwrap()).unwrap();
    assert_eq!(doc["name"], "shipping_4x6");
    assert!(close(&doc["label"]["width_mm"], 101.5));
    assert!(close(&doc["label"]["height_mm"], 152.25));
    assert_eq!(doc["label"]["media"], "gap");

    let els = elements(&s);
    let types: Vec<&str> = els.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        types,
        ["text", "text", "rect", "rect", "text", "barcode", "text"]
    );

    // ^CF0,30 at ^FO40,30 plus ^LH10,20; 9 characters at 0.6 × 30 dots.
    let t = &els[0];
    assert_eq!(t["text"], "Acme Kft.");
    assert_eq!(t["bold"], true);
    assert_eq!(t["wrap"], false);
    assert_eq!(t["fit"], "shrink");
    assert!(close(&t["font_size_pt"], pt(30.0)));
    assert!(frame(t, 50.0, 50.0, 162.0, 30.0), "{t}");

    // ^FB400,3,5: three lines of 25 + 5 dots, UTF-8 under ^CI28.
    let b = &els[1];
    assert_eq!(b["text"], "Váci út 1\n1134 Budapest\nHungary");
    assert_eq!(b["wrap"], true);
    assert!(close(&b["line_spacing"], 30.0 / 25.0));
    assert!(frame(b, 50.0, 90.0, 400.0, 90.0), "{b}");

    // A 4-dot box is a filled bar; the 3-dot box keeps its outline and rounding.
    assert_eq!(els[2]["fill"], true);
    assert!(frame(&els[2], 10.0, 200.0, 792.0, 4.0));
    assert_eq!(els[3]["fill"], false);
    assert!(close(&els[3]["stroke_mm"], mm(3.0)));
    assert!(close(&els[3]["corner_radius_mm"], mm(2.0 / 8.0 * 75.0)));

    assert_eq!(els[4]["text"], "PRIORITY");
    assert_eq!(els[4]["invert"], true);

    // ^BY3 and ^BC height 100: 101 modules of set C, the interpretation line below.
    let bc = &els[5];
    assert_eq!(bc["symbology"], "code128_c");
    assert_eq!(bc["data"], "123456789012");
    assert_eq!(bc["human_readable"], "below");
    assert!(frame(bc, 40.0, 420.0, 303.0, 127.0), "{bc}");

    // Bitmap font D: 10-dot cells; ^FH decodes the UTF-8 bytes.
    let d = &els[6];
    assert_eq!(d["text"], "Café");
    assert_eq!(d["bold"], false);
    assert!(close(&d["width_mm"], mm(40.0)));
}

#[test]
fn orientation_codes_and_shapes() {
    let (s, warnings) = open("rotated.zpl");
    for w in [
        "the label has no `^PW`; its width was taken from the content",
        "the label has no `^LL`; its length was taken from the content",
        "field variables (`^FN`) were imported as their default data",
        "`^ZZ` is not supported and was skipped",
    ] {
        assert!(warnings.iter().any(|x| x == w), "{w}: {warnings:?}");
    }
    let els = elements(&s);
    let types: Vec<&str> = els.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        types,
        [
            "text", "text", "text", "code2d", "code2d", "ellipse", "ellipse", "line", "barcode",
            "barcode", "text"
        ]
    );

    // ^FWR turns the field: its 40×144 bounding box starts at ^FO50,50.
    let t = &els[0];
    assert!(close(&t["rotation_deg"], 90.0));
    assert!(frame(t, 70.0 - 72.0, 122.0 - 20.0, 144.0, 40.0), "{t}");
    assert!(close(&els[1]["rotation_deg"], 0.0));
    // ^FT is the bottom-left corner.
    assert!(frame(&els[2], 200.0, 270.0, 144.0, 30.0), "{}", els[2]);

    // 21 bytes at ECC H need version 3: 29 modules × 4.
    let qr = &els[3];
    assert_eq!(qr["symbology"], "qr");
    assert_eq!(qr["ecc"], "H");
    assert_eq!(qr["data"], "https://example.com/a");
    assert!(frame(qr, 400.0, 50.0, 116.0, 116.0), "{qr}");
    let dm = &els[4];
    assert_eq!(dm["symbology"], "data_matrix");
    assert!(close(&dm["rotation_deg"], 0.0));
    assert!(frame(dm, 400.0, 300.0, 72.0, 72.0), "{dm}");

    assert_eq!(els[5]["fill"], false);
    assert!(frame(&els[5], 50.0, 400.0, 80.0, 80.0));
    assert_eq!(els[6]["fill"], true);
    // `L` leans left: top-left to bottom-right.
    assert!(frame(&els[7], 300.0, 400.0, 100.0, 50.0), "{}", els[7]);

    let c39 = &els[8];
    assert_eq!(c39["symbology"], "code39");
    assert!(frame(c39, 50.0, 500.0, 158.0, 78.0), "{c39}");
    let ean = &els[9];
    assert_eq!(ean["symbology"], "ean13");
    assert!(close(&ean["width_mm"], mm(190.0)));
    assert_eq!(els[10]["text"], "default");

    let doc = serde_json::to_value(s.doc(None).unwrap()).unwrap();
    // The widest element ends at the QR code; the lowest is the turned ^FN text.
    assert!(close(&doc["label"]["width_mm"], mm(516.0)));
    assert!(close(&doc["label"]["height_mm"], mm(635.0)));
}

#[test]
fn graphic_fields_become_bitmaps() {
    let (s, _) = open("graphics.zpl");
    let els = elements(&s);
    assert_eq!(els.len(), 3);
    let doc = s.doc(None).unwrap();
    let rows = |e: &Value| -> Vec<u8> {
        assert!(frame(e, e["x_mm"].as_f64().unwrap() * 8.0, 10.0, 16.0, 4.0));
        let asset = &doc.assets[e["asset"].as_str().unwrap()];
        assert_eq!(asset.mime, "image/bmp");
        let bmp = base64::engine::general_purpose::STANDARD
            .decode(&asset.data_base64)
            .unwrap();
        // Rows are bottom-up and padded to 4 bytes.
        bmp[62..]
            .chunks(4)
            .rev()
            .flat_map(|r| r[..2].to_vec())
            .collect()
    };
    // Compressed hex: a row, `:` repeat, `,` empty row, `!` full row.
    assert_eq!(rows(&els[0]), [0xFF, 0, 0xFF, 0, 0, 0, 0xFF, 0xFF]);
    // :Z64: zlib + base64.
    assert_eq!(rows(&els[1]), [0xFF, 0, 0, 0xFF, 0xFF, 0, 0, 0xFF]);
    // `K` repeats five times across a row end.
    assert_eq!(rows(&els[2]), [0xFF, 0xFF, 0xF0, 0, 0, 0, 0xFF, 0xFF]);
}

#[test]
fn dpi_scales_and_bad_files_fail() {
    let mut s = labely_testkit::test_session();
    let e = s.execute(
        "doc.open",
        json!({"path": fixture("shipping_4x6.zpl"), "dpi": 300}),
    );
    assert!(e.ok, "{:?}", e.error);
    assert!(e.warnings[0].contains("ZPL at 300 dpi"));
    let doc = serde_json::to_value(s.doc(None).unwrap()).unwrap();
    assert!(close(&doc["label"]["width_mm"], 812.0 / 300.0 * 25.4));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.zpl");
    std::fs::write(&path, "^XA^XZ").unwrap();
    let e = err(&mut s, "doc.open", json!({"path": path.to_str().unwrap()}));
    assert_eq!(e.code, ErrorCode::BadParams);
    assert!(e.message.contains("not a valid ZPL label"), "{}", e.message);
}
