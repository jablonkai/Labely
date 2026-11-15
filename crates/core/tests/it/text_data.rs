//! Linked mirror copies, rich-text spans, image crop, the length guard and the font list.

use serde_json::json;

use crate::{add_text, elements, err, ok, session_with_doc};

fn by_id(s: &labely_core::Session, id: u64) -> serde_json::Value {
    elements(s).into_iter().find(|e| e["id"] == id).unwrap()
}

#[test]
fn mirror_copy_follows_its_source() {
    let mut s = session_with_doc(); // 40 × 30
    let src = add_text(&mut s, "Hello"); // 1,1 30×8
    let m = ok(&mut s, "element.mirror", json!({"id": src}))["id"]
        .as_u64()
        .unwrap();
    let e = by_id(&s, m);
    assert_eq!(
        (e["x_mm"].as_f64(), e["y_mm"].as_f64()),
        (Some(9.0), Some(1.0))
    );
    assert_eq!(e["flip_h"], true);
    assert_eq!(e["mirror"], json!({"of": src, "axis": "horizontal"}));

    ok(
        &mut s,
        "element.set_props",
        json!({"id": src, "props": {"text": "Bye", "x_mm": 2, "rotation_deg": 30}}),
    );
    let e = by_id(&s, m);
    assert_eq!(e["text"], "Bye");
    assert_eq!(e["x_mm"].as_f64(), Some(8.0));
    assert_eq!(e["rotation_deg"].as_f64(), Some(330.0));
    // Undo restores both in one step.
    ok(&mut s, "history.undo", json!({}));
    assert_eq!(by_id(&s, m)["text"], "Hello");

    let v = ok(
        &mut s,
        "element.mirror",
        json!({"id": src, "axis": "vertical"}),
    )["id"]
        .as_u64()
        .unwrap();
    let e = by_id(&s, v);
    assert_eq!(
        (e["y_mm"].as_f64(), e["flip_v"].as_bool()),
        (Some(21.0), Some(true))
    );
    let e = err(&mut s, "element.mirror", json!({"id": v}));
    assert_eq!(e.param.as_deref(), Some("id"));

    // Unlinking keeps the copy as it is; deleting the source unlinks the rest.
    ok(
        &mut s,
        "element.set_props",
        json!({"id": m, "props": {"mirror": null}}),
    );
    ok(&mut s, "element.delete", json!({"ids": [src]}));
    let (em, ev) = (by_id(&s, m), by_id(&s, v));
    assert!(em.get("mirror").is_none() && ev.get("mirror").is_none());
    assert_eq!(em["text"], "Hello");
}

#[test]
fn paste_and_duplicate_keep_mirror_links_consistent() {
    let mut s = session_with_doc();
    let src = add_text(&mut s, "A");
    let m = ok(&mut s, "element.mirror", json!({"id": src}))["id"]
        .as_u64()
        .unwrap();
    let dup = ok(&mut s, "element.duplicate", json!({"id": m}))["id"]
        .as_u64()
        .unwrap();
    assert!(by_id(&s, dup).get("mirror").is_none());

    let both = vec![by_id(&s, src), by_id(&s, m)];
    let ids = ok(&mut s, "element.paste", json!({"elements": both}))["ids"].clone();
    let (ns, nm) = (ids[0].as_u64().unwrap(), ids[1].as_u64().unwrap());
    assert_eq!(by_id(&s, nm)["mirror"]["of"], ns);
    let lone = vec![by_id(&s, m)];
    let id = ok(&mut s, "element.paste", json!({"elements": lone}))["ids"][0]
        .as_u64()
        .unwrap();
    assert!(by_id(&s, id).get("mirror").is_none());
}

#[test]
fn spans_crop_and_length_guard_are_validated() {
    let mut s = session_with_doc();
    let t = add_text(&mut s, "Hello world");
    let set = |s: &mut labely_core::Session, props| {
        s.execute("element.set_props", json!({"id": t, "props": props}))
    };
    assert!(
        set(
            &mut s,
            json!({"spans": [{"start": 0, "end": 5, "bold": true, "font_size_pt": 20}],
                   "max_length_mm": 25, "fit": "fill"})
        )
        .ok
    );
    let e = set(&mut s, json!({"spans": [{"start": 3, "end": 3}]}));
    assert_eq!(e.error.unwrap().param.as_deref(), Some("props.spans.0"));
    let e = set(&mut s, json!({"max_length_mm": 0}));
    assert_eq!(
        e.error.unwrap().param.as_deref(),
        Some("props.max_length_mm")
    );
    // Spans past the end of the text are tolerated (clamped when drawn).
    assert!(set(&mut s, json!({"text": "Hi"})).ok);

    let head = [0x89u8, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0];
    let png = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, head);
    let img = ok(
        &mut s,
        "element.add_image",
        json!({"x_mm": 0, "y_mm": 0, "width_mm": 10, "height_mm": 10, "data_base64": png}),
    )["id"]
        .as_u64()
        .unwrap();
    let e = err(
        &mut s,
        "element.set_props",
        json!({"id": img, "props": {"crop": {"left": 0.5, "top": 0, "right": 0.4, "bottom": 1}}}),
    );
    assert_eq!(e.param.as_deref(), Some("props.crop"));
    ok(
        &mut s,
        "element.set_props",
        json!({"id": img, "props": {"crop": {"left": 0.25, "top": 0, "right": 0.75, "bottom": 1}}}),
    );
}

#[test]
fn font_list_falls_back_to_the_default_font() {
    let mut s = session_with_doc();
    let r = ok(&mut s, "font.list", json!({}));
    assert_eq!(r["fonts"], json!(["Noto Sans"]));
}
