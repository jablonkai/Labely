use labely_core::{ElementId, ErrorCode, Session};
use serde_json::json;

use crate::{err, ok, session_with_doc};

fn rect(s: &mut Session, x: f64, y: f64, w: f64, h: f64) -> u64 {
    ok(
        s,
        "element.add_rect",
        json!({"x_mm": x, "y_mm": y, "width_mm": w, "height_mm": h}),
    )["id"]
        .as_u64()
        .unwrap()
}

fn pos(s: &Session, id: u64) -> (f64, f64) {
    let e = s.doc(None).unwrap().element(ElementId(id)).unwrap();
    (e.x_mm, e.y_mm)
}

#[test]
fn align_to_selection_and_label() {
    let mut s = session_with_doc(); // 40 × 30 mm
    let a = rect(&mut s, 2.0, 1.0, 4.0, 4.0);
    let b = rect(&mut s, 10.0, 8.0, 6.0, 2.0);
    let r = ok(
        &mut s,
        "element.align",
        json!({"ids": [a, b], "edge": "left"}),
    );
    assert_eq!(r["moved"], json!([b]));
    assert_eq!(pos(&s, b), (2.0, 8.0));
    ok(
        &mut s,
        "element.align",
        json!({"ids": [a, b], "edge": "bottom"}),
    );
    assert_eq!(pos(&s, a).1, 6.0);
    ok(
        &mut s,
        "element.align",
        json!({"ids": [a, b], "edge": "center"}),
    );
    assert_eq!(pos(&s, a).0, 3.0, "centred on the 2–8 mm box");
    // One id aligns to the label.
    ok(
        &mut s,
        "element.align",
        json!({"ids": [a], "edge": "right"}),
    );
    assert_eq!(pos(&s, a).0, 36.0);
    ok(
        &mut s,
        "element.align",
        json!({"ids": [a, b], "edge": "middle", "to": "label"}),
    );
    assert_eq!((pos(&s, a).1, pos(&s, b).1), (13.0, 14.0));

    // Rotated elements align by their bounding box.
    let c = rect(&mut s, 10.0, 10.0, 10.0, 2.0);
    ok(
        &mut s,
        "element.rotate",
        json!({"id": c, "rotation_deg": 90}),
    );
    ok(&mut s, "element.align", json!({"ids": [c], "edge": "top"}));
    assert_eq!(
        pos(&s, c).1,
        4.0,
        "a 10 mm tall rotated box touches the top"
    );

    // Locked elements stay and are reported.
    ok(&mut s, "element.lock", json!({"ids": [b]}));
    let e = s.execute("element.align", json!({"ids": [a, b], "edge": "top"}));
    assert!(e.ok && e.warnings[0].contains("locked"));
    assert_eq!(pos(&s, b).1, 14.0);
    let e = err(&mut s, "element.align", json!({"ids": [], "edge": "top"}));
    assert_eq!(e.param.as_deref(), Some("ids"));
}

#[test]
fn distribute_equal_gaps() {
    let mut s = session_with_doc();
    let a = rect(&mut s, 0.0, 0.0, 2.0, 2.0);
    let b = rect(&mut s, 3.0, 0.0, 4.0, 2.0);
    let c = rect(&mut s, 20.0, 0.0, 2.0, 2.0);
    let d = rect(&mut s, 10.0, 0.0, 2.0, 2.0);
    // Boxes span 0–22 with 10 mm of content: three gaps of 4 mm.
    let r = ok(
        &mut s,
        "element.distribute",
        json!({"ids": [c, a, b, d], "axis": "horizontal"}),
    );
    assert_eq!(r["moved"], json!([b, d]));
    assert_eq!(pos(&s, b).0, 6.0);
    assert_eq!(pos(&s, d).0, 14.0);
    assert_eq!((pos(&s, a).0, pos(&s, c).0), (0.0, 20.0));
    let e = err(
        &mut s,
        "element.distribute",
        json!({"ids": [a, b], "axis": "vertical"}),
    );
    assert!(e.message.contains("three"));
}

#[test]
fn group_ungroup_and_duplicate() {
    let mut s = session_with_doc();
    let a = rect(&mut s, 0.0, 0.0, 2.0, 2.0);
    let b = rect(&mut s, 3.0, 0.0, 2.0, 2.0);
    let c = rect(&mut s, 6.0, 0.0, 2.0, 2.0);
    let g = ok(&mut s, "element.group", json!({"ids": [a, b]}));
    assert_eq!(g["members"], json!([a, b]));
    // Grouping a member with c brings the whole group in.
    let g2 = ok(&mut s, "element.group", json!({"ids": [b, c]}));
    assert_eq!(g2["members"], json!([a, b, c]));
    assert_ne!(g["group"], g2["group"]);
    let gid = g2["group"].as_u64();
    let doc = s.doc(None).unwrap();
    assert!(doc.elements().all(|e| e.group == gid));
    // A duplicate is not part of the group.
    let d = ok(&mut s, "element.duplicate", json!({"id": a}))["id"]
        .as_u64()
        .unwrap();
    assert_eq!(
        s.doc(None).unwrap().element(ElementId(d)).unwrap().group,
        None
    );
    let r = ok(&mut s, "element.ungroup", json!({"ids": [c]}));
    assert_eq!(r["ungrouped"], json!([a, b, c]));
    assert!(s.doc(None).unwrap().elements().all(|e| e.group.is_none()));
    let e = s.execute("element.ungroup", json!({"ids": [a]}));
    assert!(e.ok && !e.warnings.is_empty());
    let e = err(&mut s, "element.group", json!({"ids": [a]}));
    assert_eq!(e.code, ErrorCode::BadParams);
    let e = err(&mut s, "element.group", json!({"ids": [a, 999]}));
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn lock_and_unlock() {
    let mut s = session_with_doc();
    let a = rect(&mut s, 0.0, 0.0, 2.0, 2.0);
    ok(&mut s, "element.lock", json!({"ids": [a]}));
    let e = err(
        &mut s,
        "element.move",
        json!({"id": a, "x_mm": 5, "y_mm": 5}),
    );
    assert_eq!(e.code, ErrorCode::Disabled);
    ok(&mut s, "element.lock", json!({"ids": [a], "locked": false}));
    ok(
        &mut s,
        "element.move",
        json!({"id": a, "x_mm": 5, "y_mm": 5}),
    );
}
