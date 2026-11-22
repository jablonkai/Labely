//! NiimBlue import through `doc.open` (ADR-0043).

use labely_core::ErrorCode;
use serde_json::{Value, json};

use crate::{elements, err, ok};

fn fixture(name: &str) -> String {
    format!(
        "{}/tests/fixtures/niimblue/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn close(a: &Value, b: f64) -> bool {
    a.as_f64().is_some_and(|a| (a - b).abs() < 1e-6)
}

fn open(name: &str, params: Value) -> (labely_core::Session, Vec<String>) {
    let mut s = labely_testkit::test_session();
    let mut p = json!({"path": fixture(name)});
    if let (Some(p), Some(extra)) = (p.as_object_mut(), params.as_object()) {
        p.extend(extra.clone());
    }
    let e = s.execute("doc.open", p);
    assert!(e.ok, "{:?}", e.error);
    (s, e.warnings)
}

#[test]
fn left_label_turns_and_maps_every_object() {
    let (mut s, warnings) = open("sample_40x12.json", json!({}));
    assert!(warnings[0].contains("NiimBlue at 203 dpi"), "{warnings:?}");
    let g = ok(&mut s, "doc.get", json!({}));
    assert_eq!(g["dirty"], true);
    assert_eq!(g["path"], Value::Null);
    let doc = serde_json::to_value(s.doc(None).unwrap()).unwrap();
    assert_eq!(doc["name"], "Sample 40x12");
    // 320×96 dots at 8 dpmm, printed turned clockwise: 12 mm across the head.
    assert!(close(&doc["label"]["width_mm"], 12.0));
    assert!(close(&doc["label"]["height_mm"], 40.0));
    assert_eq!(doc["data"]["columns"], json!(["name", "code"]));
    assert_eq!(doc["data"]["rows"].as_array().unwrap().len(), 2);

    let els = elements(&s);
    let types: Vec<&str> = els.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        types,
        [
            "text", "code2d", "barcode", "rect", "ellipse", "line", "image"
        ]
    );
    let t = &els[0];
    assert_eq!(t["text"], "{{name}}");
    assert_eq!(t["bold"], true);
    assert_eq!(t["fit"], "shrink");
    assert_eq!(t["align"], "center");
    assert_eq!(t["font_family"], "Noto Sans");
    assert!(close(&t["font_size_pt"], 20.0 / 8.0 * 72.0 / 25.4));
    // Centre (60, 20) dots = (7.5, 2.5) mm turns to (12 − 2.5, 7.5).
    assert!(close(&t["rotation_deg"], 90.0));
    assert!(close(&t["width_mm"], 12.5) && close(&t["height_mm"], 2.5));
    assert!(
        close(&t["x_mm"], 9.5 - 6.25) && close(&t["y_mm"], 7.5 - 1.25),
        "{t}"
    );

    assert_eq!(els[1]["data"], "https://example.com/{{code}}");
    assert_eq!(els[2]["symbology"], "ean13");
    assert_eq!(els[2]["data"], "{{code}}");
    assert_eq!(els[3]["fill"], false);
    assert!(close(&els[3]["corner_radius_mm"], 1.0));
    assert!(close(&els[3]["stroke_mm"], 3.0 / 8.0));
    assert!(
        close(&els[4]["width_mm"], 27.0 / 8.0),
        "radius 12 + stroke 3"
    );
    // A 200-dot line, turned with the label.
    assert!(close(&els[5]["width_mm"], 25.0) && close(&els[5]["rotation_deg"], 90.0));
    let img = &els[6];
    assert!(close(&img["width_mm"], 4.0));
    assert!(doc["assets"][img["asset"].as_str().unwrap()]["mime"] == "image/png");

    // Imported documents need a path to save, then reopen as .labely.
    assert_eq!(
        err(&mut s, "doc.save", json!({})).param.as_deref(),
        Some("path")
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.labely");
    ok(&mut s, "doc.save", json!({"path": path.to_str().unwrap()}));
    let saved = s.doc(None).unwrap().clone();
    let e = s.execute(
        "doc.open",
        json!({"path": path.to_str().unwrap(), "dpi": 300}),
    );
    assert!(
        e.ok && e.warnings[0].contains("ignored"),
        "{:?}",
        e.warnings
    );
    assert_eq!(*s.doc(None).unwrap(), saved);
}

#[test]
fn dpi_scales_the_import() {
    let (s, warnings) = open("sample_40x12.json", json!({"dpi": 300}));
    assert!(warnings[0].contains("300 dpi"));
    let doc = s.doc(None).unwrap();
    assert!((doc.label.width_mm - 96.0 * 25.4 / 300.0).abs() < 1e-9);
    assert!((doc.label.height_mm - 320.0 * 25.4 / 300.0).abs() < 1e-9);

    let mut s = labely_testkit::test_session();
    let e = err(
        &mut s,
        "doc.open",
        json!({"path": fixture("sample_40x12.json"), "dpi": 50}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);
    assert_eq!(e.param.as_deref(), Some("dpi"));
}

#[test]
fn top_label_keeps_orientation_and_reports_drops() {
    let (s, warnings) = open("shelf_48x30_top.json", json!({}));
    for w in ["pharmacode", "ArUco", "`group`"] {
        assert!(warnings.iter().any(|x| x.contains(w)), "{w}: {warnings:?}");
    }
    let doc = s.doc(None).unwrap();
    assert_eq!((doc.label.width_mm, doc.label.height_mm), (48.0, 30.0));
    assert_eq!(doc.label.corner_radius_mm, 1.25);
    assert!(doc.data.is_none());

    let els = elements(&s);
    assert_eq!(els.len(), 3);
    let t = &els[0];
    assert_eq!(t["text"], "Shelf {{now:%d.%m.%Y}}");
    assert_eq!((&t["italic"], &t["wrap"]), (&json!(true), &json!(false)));
    assert!(
        close(&t["font_size_pt"], 40.0 / 8.0 * 72.0 / 25.4),
        "scaled 2×"
    );
    assert!(close(&t["x_mm"], 2.0) && close(&t["width_mm"], 50.0));
    let b = &els[1];
    assert_eq!(b["human_readable"], "none");
    assert!(close(&b["rotation_deg"], 90.0));
    assert!(
        close(&b["x_mm"], 18.75 - 11.25) && close(&b["y_mm"], 18.75 - 5.0),
        "{b}"
    );
    assert_eq!(
        (&els[2]["fill"], &els[2]["stroke_mm"]),
        (&json!(true), &json!(0.0))
    );
}

#[test]
fn list_export_opens_the_first_label() {
    let (s, warnings) = open("labels_list.json", json!({}));
    assert!(
        warnings.iter().any(|w| w.contains("2 labels")),
        "{warnings:?}"
    );
    assert_eq!(s.doc(None).unwrap().name, "Sample 40x12");
}

#[test]
fn broken_niimblue_file_is_bad_params() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.json");
    std::fs::write(
        &path,
        r#"{"label": {"printDirection": "top"}, "canvas": {"objects": []}}"#,
    )
    .unwrap();
    let mut s = labely_testkit::test_session();
    let e = err(&mut s, "doc.open", json!({"path": path.to_str().unwrap()}));
    assert_eq!(e.code, ErrorCode::BadParams);
    assert!(
        e.message.contains("NiimBlue") && e.message.contains("size"),
        "{}",
        e.message
    );
}
