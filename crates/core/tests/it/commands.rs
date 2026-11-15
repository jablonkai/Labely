use std::sync::Arc;

use labely_core::services::{RenderError, RenderOptions, RenderOutput, Renderer, Services};
use labely_core::{Bitmap1, Document, ErrorCode, Session};
use labely_testkit::FakeDeviceHub;
use serde_json::{Value, json};

use crate::{add_text, elements, err, ok, session_with_doc};

#[test]
fn unknown_command_suggests_nearest() {
    let mut s = labely_testkit::test_session();
    let e = err(&mut s, "element.add_txt", json!({}));
    assert_eq!(e.code, ErrorCode::UnknownCommand);
    assert!(
        e.message.contains("did you mean `element.add_text`"),
        "{}",
        e.message
    );
    assert_eq!(e.accepted.unwrap()[0], "element.add_text");
    let e = err(&mut s, "doc.sav", json!({}));
    assert!(e.accepted.unwrap().contains(&"doc.save".to_owned()));
}

#[test]
fn bad_params_messages() {
    let mut s = session_with_doc();
    let e = err(
        &mut s,
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 3, "height_mm": 3}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);
    assert_eq!(e.param.as_deref(), Some("text"));
    assert!(e.message.contains("missing field `text`"), "{}", e.message);
    assert!(e.hint.unwrap().contains("command.list"));

    let e = err(
        &mut s,
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 3, "height_mm": 3, "text": "a", "colour": 1}),
    );
    assert_eq!(e.param.as_deref(), Some("colour"));
    assert!(e.accepted.unwrap().contains(&"font_size_pt".to_owned()));

    let e = err(
        &mut s,
        "element.add_text",
        json!({"x_mm": "left", "y_mm": 1, "width_mm": 3, "height_mm": 3, "text": "a"}),
    );
    assert_eq!(e.param.as_deref(), Some("x_mm"));
    assert!(e.message.contains("invalid type"), "{}", e.message);

    let e = err(
        &mut s,
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 3, "height_mm": 3, "text": "a", "align": "middle"}),
    );
    assert_eq!(e.param.as_deref(), Some("align"));
    assert_eq!(
        e.accepted.unwrap(),
        vec!["left", "center", "right", "justify"]
    );

    let e = err(&mut s, "doc.get", json!([1]));
    assert!(e.message.contains("must be a JSON object"));
    let e = err(&mut s, "doc.get", json!({"terse": "yes"}));
    assert_eq!(e.param.as_deref(), Some("terse"));
}

#[test]
fn no_document_error() {
    let mut s = labely_testkit::test_session();
    let e = err(
        &mut s,
        "element.add_rect",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 3, "height_mm": 3}),
    );
    assert_eq!(e.code, ErrorCode::NoDocument);
    assert!(e.hint.unwrap().contains("doc.new"));
}

#[test]
fn envelope_changed_created_and_terse() {
    let mut s = session_with_doc();
    let env = s.execute(
        "element.add_rect",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 3, "height_mm": 3, "terse": true}),
    );
    assert!(env.ok);
    assert!(env.result.is_none());
    assert_eq!(env.created.len(), 1);
    let v = env.to_value();
    assert!(v.get("result").is_none());
    assert_eq!(v["warnings"], json!([]));
    let id = env.created[0];
    let env = s.execute("element.move", json!({"id": id, "x_mm": 2, "y_mm": 2}));
    assert_eq!(env.changed, vec![id]);
    assert!(env.created.is_empty());
}

#[test]
fn every_add_kind() {
    let mut s = session_with_doc();
    let frame = json!({"x_mm": 1, "y_mm": 1, "width_mm": 20, "height_mm": 10});
    let with = |extra: Value| {
        let mut p = frame.clone();
        p.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        p
    };
    ok(
        &mut s,
        "element.add_text",
        with(json!({"text": "Hi", "bold": true, "fit": "shrink"})),
    );
    ok(
        &mut s,
        "element.add_barcode",
        with(json!({"data": "123", "symbology": "code39"})),
    );
    ok(
        &mut s,
        "element.add_code2d",
        with(json!({"data": "https://x", "ecc": "H"})),
    );
    ok(
        &mut s,
        "element.add_rect",
        with(json!({"fill": true, "corner_radius_mm": 1})),
    );
    ok(
        &mut s,
        "element.add_ellipse",
        with(json!({"stroke_mm": 0.5})),
    );
    ok(
        &mut s,
        "element.add_line",
        json!({"x_mm": 0, "y_mm": 5, "width_mm": 30, "height_mm": 0, "dash_mm": [1, 1]}),
    );
    let png = [0x89u8, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0];
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    let r = ok(
        &mut s,
        "element.add_image",
        with(json!({"data_base64": b64, "dither": "atkinson"})),
    );
    let types: Vec<_> = elements(&s)
        .iter()
        .map(|e| e["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        types,
        [
            "text", "barcode", "code2d", "rect", "ellipse", "line", "image"
        ]
    );
    let doc = s.doc(None).unwrap();
    let key = labely_core::commands::element::sha256_hex(&png);
    assert_eq!(doc.assets[&key].mime, "image/png");
    assert_eq!(elements(&s)[6]["asset"], key.as_str());
    assert_eq!(r["id"], 7);

    let e = err(&mut s, "element.add_image", with(json!({})));
    assert!(e.message.contains("exactly one of"));
    let e = err(
        &mut s,
        "element.add_image",
        with(json!({"path": "/nonexistent/x.png"})),
    );
    assert_eq!(e.code, ErrorCode::NotFound);
    let e = err(
        &mut s,
        "element.add_line",
        json!({"x_mm": 0, "y_mm": 5, "width_mm": 0, "height_mm": 0}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);
}

#[test]
fn set_props_merges_and_rejects() {
    let mut s = session_with_doc();
    let id = add_text(&mut s, "a");
    let r = ok(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"text": "b", "font_size_pt": 20, "x_mm": 7, "name": "title"}}),
    );
    assert_eq!(r["element"]["text"], "b");
    assert_eq!(r["element"]["font_size_pt"], 20.0);
    assert_eq!(r["element"]["x_mm"], 7.0);
    assert_eq!(
        r["element"]["bold"], false,
        "untouched props keep their values"
    );

    let e = err(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"data": "x"}}),
    );
    assert_eq!(e.param.as_deref(), Some("props.data"));
    assert!(
        e.message
            .contains("unknown property `data` for a text element")
    );
    assert!(e.accepted.unwrap().contains(&"font_family".to_owned()));
    let e = err(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"font_size_pt": "big"}}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);
    assert!(e.param.unwrap().contains("font_size_pt"));
    let e = err(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"type": "rect"}}),
    );
    assert_eq!(e.param.as_deref(), Some("props.type"));
    let e = err(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"font_size_pt": 0}}),
    );
    assert_eq!(e.param.as_deref(), Some("props.font_size_pt"));
    let e = err(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"text": null}}),
    );
    assert!(e.message.contains("missing field `text`"), "{}", e.message);
    ok(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"name": null}}),
    );
    assert!(elements(&s)[0].get("name").is_none());

    // Locking.
    ok(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"locked": true}}),
    );
    let e = err(
        &mut s,
        "element.move",
        json!({"id": id, "x_mm": 0, "y_mm": 0}),
    );
    assert_eq!(e.code, ErrorCode::Disabled);
    ok(
        &mut s,
        "element.set_props",
        json!({"id": id, "props": {"locked": false}}),
    );
    ok(
        &mut s,
        "element.move",
        json!({"id": id, "x_mm": 0, "y_mm": 0}),
    );
}

#[test]
fn delete_duplicate_reorder() {
    let mut s = session_with_doc();
    let a = add_text(&mut s, "a");
    let b = add_text(&mut s, "b");
    let c = ok(&mut s, "element.duplicate", json!({"id": a}))["id"]
        .as_u64()
        .unwrap();
    let els = elements(&s);
    assert_eq!(els[2]["x_mm"], 3.0);
    assert_eq!(els[2]["text"], "a");
    let r = ok(&mut s, "element.reorder", json!({"id": c, "to": "back"}));
    assert_eq!(r["index"], 0);
    ok(&mut s, "element.reorder", json!({"id": a, "to": "front"}));
    ok(
        &mut s,
        "element.reorder",
        json!({"id": a, "to": "backward"}),
    );
    let ids: Vec<_> = elements(&s)
        .iter()
        .map(|e| e["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, [c, a, b]);
    let e = err(&mut s, "element.delete", json!({"ids": [a, 99]}));
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(elements(&s).len(), 3);
    let env = s.execute("element.delete", json!({"ids": [a, b]}));
    assert_eq!(env.changed.len(), 2);
    assert_eq!(elements(&s).len(), 1);
}

#[test]
fn variables_and_preview() {
    let mut s = session_with_doc();
    ok(
        &mut s,
        "variable.set",
        json!({"name": "sku", "value": "A-1"}),
    );
    ok(
        &mut s,
        "element.add_barcode",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 38, "height_mm": 10, "data": "{{sku}}"}),
    );
    let r = ok(&mut s, "render.preview", json!({}));
    assert_eq!(r["dpi"], 203, "model dpi");
    assert_eq!(r["width_dots"], 320);
    assert_eq!(r["height_dots"], 240);
    use base64::Engine;
    let png = base64::engine::general_purpose::STANDARD
        .decode(r["png_base64"].as_str().unwrap())
        .unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let r = ok(&mut s, "render.preview", json!({"dpi": 300}));
    assert_eq!(r["dpi"], 300);
    let e = err(&mut s, "render.preview", json!({"page": 3}));
    assert_eq!(e.param.as_deref(), Some("page"));
    // The check decodes the interpolated value.
    let r = ok(&mut s, "doc.check", json!({}));
    assert_eq!(r["ok"], true, "{r}");
    assert!(
        r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["rule"] != "code_unreadable"),
        "{r}"
    );
}

#[test]
fn doc_check_rules() {
    let mut s = labely_testkit::test_session();
    ok(
        &mut s,
        "doc.new",
        json!({"width_mm": 60, "height_mm": 30, "printer_model": "test.fake", "density": 9}),
    );
    ok(
        &mut s,
        "element.add_rect",
        json!({"x_mm": 55, "y_mm": 1, "width_mm": 10, "height_mm": 5}),
    );
    ok(
        &mut s,
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 5, "height_mm": 3, "text": "much too long"}),
    );
    ok(
        &mut s,
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 10, "width_mm": 5, "height_mm": 3, "text": "shrink me please", "fit": "shrink"}),
    );
    ok(
        &mut s,
        "element.add_barcode",
        json!({"x_mm": 1, "y_mm": 15, "width_mm": 3, "height_mm": 5, "data": "123456789"}),
    );
    ok(
        &mut s,
        "element.add_code2d",
        json!({"x_mm": 20, "y_mm": 15, "width_mm": 10, "height_mm": 10, "data": ""}),
    );
    ok(
        &mut s,
        "element.add_code2d",
        json!({"x_mm": 35, "y_mm": 15, "width_mm": 10, "height_mm": 10, "data": "ok"}),
    );
    let r = ok(&mut s, "doc.check", json!({}));
    assert_eq!(r["ok"], false);
    let rules: Vec<(String, Option<u64>)> = r["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["rule"].as_str().unwrap().to_owned(),
                i["element_id"].as_u64(),
            )
        })
        .collect();
    let has = |rule: &str, id: Option<u64>| rules.iter().any(|(r, i)| r == rule && *i == id);
    assert!(has("out_of_bounds", Some(1)), "{rules:?}");
    assert!(has("label_too_wide", None));
    assert!(has("density_out_of_range", None));
    assert!(has("text_overflow", Some(2)));
    assert!(has("text_shrunk", Some(3)));
    assert!(has("module_too_narrow", Some(4)));
    assert!(has("code_unreadable", Some(4)));
    assert!(has("encode_failed", Some(5)));
    assert!(
        !has("code_unreadable", Some(5)),
        "encode failures are not double-reported"
    );
    assert!(!has("code_unreadable", Some(6)));
    for i in r["issues"].as_array().unwrap() {
        assert!(!i["hint"].as_str().unwrap().is_empty());
    }
}

#[derive(Debug)]
struct PanicRenderer;

impl Renderer for PanicRenderer {
    fn render(&self, _: &Document, _: &RenderOptions) -> Result<RenderOutput, RenderError> {
        panic!("boom")
    }
    fn decode_codes(&self, _: &Bitmap1) -> Vec<labely_core::services::DecodedCode> {
        Vec::new()
    }
    fn to_png(&self, _: &Bitmap1) -> Result<Vec<u8>, RenderError> {
        Ok(Vec::new())
    }
}

#[test]
fn panic_guard_returns_internal_and_keeps_state() {
    let services = Services {
        renderer: Arc::new(PanicRenderer),
        devices: Arc::new(FakeDeviceHub::new()),
    };
    let mut s = Session::new(services);
    ok(&mut s, "doc.new", json!({"width_mm": 10, "height_mm": 10}));
    add_text(&mut s, "a");
    let before = s.doc(None).unwrap().clone();
    let e = err(&mut s, "render.preview", json!({}));
    assert_eq!(e.code, ErrorCode::Internal);
    assert!(e.message.contains("panicked: boom"), "{}", e.message);
    assert_eq!(*s.doc(None).unwrap(), before);
    let e = err(
        &mut s,
        "command.batch",
        json!({"steps": [
            {"id": "element.add_rect", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 2, "height_mm": 2}},
            {"id": "render.preview", "params": {}}
        ]}),
    );
    assert_eq!(e.code, ErrorCode::Internal);
    assert_eq!(*s.doc(None).unwrap(), before);
    ok(&mut s, "history.undo", json!({}));
}

#[test]
fn doc_set_label_and_get() {
    let mut s = session_with_doc();
    let r = ok(
        &mut s,
        "doc.set_label",
        json!({"width_mm": 45, "media": "continuous", "density": 4}),
    );
    assert_eq!(r["label"]["width_mm"], 45.0);
    assert_eq!(r["label"]["height_mm"], 30.0);
    let e = err(&mut s, "doc.set_label", json!({"height_mm": 0}));
    assert_eq!(e.param.as_deref(), Some("height_mm"));
    let g = ok(&mut s, "doc.get", json!({}));
    assert_eq!(g["document"]["label"]["media"], "continuous");
    assert_eq!(g["dirty"], true);
    assert_eq!(g["can_undo"], true);
    ok(
        &mut s,
        "doc.set_label",
        json!({"printer_model": "", "density": 0}),
    );
    let g = ok(&mut s, "doc.get", json!({}));
    assert!(g["document"]["label"].get("printer_model").is_none());
}
