//! Open-document tabs, rotating the label paper and pasting elements.

use base64::Engine;
use serde_json::json;

use crate::{add_text, elements, err, ok, session_with_doc};

#[test]
fn list_and_activate_documents() {
    let mut s = session_with_doc();
    for n in 0..10 {
        ok(
            &mut s,
            "doc.new",
            json!({"name": format!("L{n}"), "width_mm": 20, "height_mm": 10}),
        );
    }
    let r = ok(&mut s, "doc.list", json!({}));
    let ids: Vec<_> = r["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["doc_id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 11);
    assert_eq!(
        (ids[1].as_str(), ids[10].as_str()),
        ("doc2", "doc11"),
        "open order"
    );
    assert_eq!(r["docs"][10]["active"], true);

    ok(&mut s, "doc.activate", json!({"doc_id": "doc1"}));
    assert_eq!(s.active_doc_id(), Some("doc1"));
    add_text(&mut s, "first");
    let r = ok(&mut s, "doc.list", json!({}));
    assert_eq!(
        (
            r["docs"][0]["active"].as_bool(),
            r["docs"][0]["dirty"].as_bool()
        ),
        (Some(true), Some(true))
    );
    let e = err(&mut s, "doc.activate", json!({"doc_id": "doc99"}));
    assert_eq!(e.param.as_deref(), Some("doc_id"));
}

#[test]
fn rotate_label_with_and_without_content() {
    let mut s = session_with_doc(); // 40 × 30
    ok(
        &mut s,
        "element.add_rect",
        json!({"x_mm": 0, "y_mm": 0, "width_mm": 10, "height_mm": 4}),
    );
    let r = ok(&mut s, "doc.rotate_label", json!({}));
    assert_eq!(
        (
            r["label"]["width_mm"].as_f64(),
            r["label"]["height_mm"].as_f64()
        ),
        (Some(30.0), Some(40.0))
    );
    // Centre (5, 2) turns clockwise to (30 − 2, 5) = (28, 5).
    let e = &elements(&s)[0];
    assert_eq!(
        (e["x_mm"].as_f64(), e["y_mm"].as_f64()),
        (Some(23.0), Some(3.0))
    );
    assert_eq!(e["rotation_deg"], 90.0);

    // Turning back restores the design.
    ok(&mut s, "doc.rotate_label", json!({"direction": "ccw"}));
    let e = &elements(&s)[0];
    assert_eq!(
        (e["x_mm"].as_f64(), e["y_mm"].as_f64()),
        (Some(0.0), Some(0.0))
    );
    assert_eq!(e["rotation_deg"], 0.0);

    ok(&mut s, "doc.rotate_label", json!({"rotate_content": false}));
    let e = &elements(&s)[0];
    assert_eq!(
        (e["x_mm"].as_f64(), e["rotation_deg"].as_f64()),
        (Some(0.0), Some(0.0))
    );
    assert_eq!(s.doc(None).unwrap().label.width_mm, 30.0);
    ok(&mut s, "history.undo", json!({}));
    assert_eq!(s.doc(None).unwrap().label.width_mm, 40.0);
}

#[test]
fn paste_elements_across_documents() {
    let mut s = session_with_doc();
    let a = add_text(&mut s, "a");
    let b = add_text(&mut s, "b");
    ok(&mut s, "element.group", json!({"ids": [a, b]}));
    let png = [0x89u8, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0];
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    ok(
        &mut s,
        "element.add_image",
        json!({"x_mm": 0, "y_mm": 0, "width_mm": 5, "height_mm": 5, "data_base64": b64}),
    );
    ok(&mut s, "element.lock", json!({"ids": [a], "locked": true}));
    let doc = s.doc(None).unwrap().clone();
    let copied = serde_json::to_value(&doc.pages[0].elements).unwrap();
    let assets = serde_json::to_value(&doc.assets).unwrap();

    ok(&mut s, "doc.new", json!({"width_mm": 40, "height_mm": 30}));
    let e = err(&mut s, "element.paste", json!({"elements": copied}));
    assert_eq!(
        e.param.as_deref(),
        Some("assets"),
        "image without its asset"
    );
    let r = ok(
        &mut s,
        "element.paste",
        json!({"elements": copied, "assets": assets, "dx_mm": 1}),
    );
    assert_eq!(r["ids"].as_array().unwrap().len(), 3);
    let els = elements(&s);
    assert_eq!(els[0]["x_mm"], 2.0);
    assert_eq!(els[0]["locked"], false);
    let g = els[0]["group"].as_u64().unwrap();
    assert_eq!(els[1]["group"].as_u64(), Some(g));
    assert!(
        !r["ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i.as_u64() == Some(g))
    );
    assert_eq!(s.doc(None).unwrap().assets.len(), 1);

    // A lone member of a group is pasted ungrouped.
    let one = json!([copied[0]]);
    ok(&mut s, "element.paste", json!({"elements": one}));
    assert!(elements(&s)[3].get("group").is_none());

    let mut bad = assets.clone();
    let key = bad.as_object().unwrap().keys().next().unwrap().clone();
    bad[&key]["data_base64"] = json!("AAAA");
    let e = err(
        &mut s,
        "element.paste",
        json!({"elements": copied, "assets": bad}),
    );
    assert!(e.message.contains("sha256"));
    let e = err(&mut s, "element.paste", json!({"elements": []}));
    assert_eq!(e.param.as_deref(), Some("elements"));
}
