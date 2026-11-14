use labely_core::ErrorCode;
use serde_json::json;

use crate::{add_text, elements, err, ok, session_with_doc};

#[test]
fn undo_redo_roundtrip() {
    let mut s = session_with_doc();
    let id = add_text(&mut s, "a");
    ok(
        &mut s,
        "element.move",
        json!({"id": id, "x_mm": 5, "y_mm": 6}),
    );
    assert_eq!(elements(&s)[0]["x_mm"], 5.0);

    let r = ok(&mut s, "history.undo", json!({}));
    assert_eq!(r["label"], "element.move");
    assert_eq!(elements(&s)[0]["x_mm"], 1.0);
    ok(&mut s, "history.undo", json!({}));
    assert!(elements(&s).is_empty());
    let e = err(&mut s, "history.undo", json!({}));
    assert_eq!(e.code, ErrorCode::Disabled);

    ok(&mut s, "history.redo", json!({}));
    ok(&mut s, "history.redo", json!({}));
    assert_eq!(elements(&s)[0]["x_mm"], 5.0);
    assert!(!s.can_redo(None));

    // A new edit clears redo.
    ok(&mut s, "history.undo", json!({}));
    ok(
        &mut s,
        "element.rotate",
        json!({"id": id, "rotation_deg": -90}),
    );
    assert_eq!(elements(&s)[0]["rotation_deg"], 270.0);
    assert!(!s.can_redo(None));
}

#[test]
fn queries_and_failures_push_nothing() {
    let mut s = session_with_doc();
    let id = add_text(&mut s, "a");
    ok(&mut s, "doc.get", json!({}));
    ok(&mut s, "render.preview", json!({}));
    let before = s.doc(None).unwrap().clone();
    let e = err(
        &mut s,
        "element.resize",
        json!({"id": id, "width_mm": -1, "height_mm": 2}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);
    assert_eq!(*s.doc(None).unwrap(), before);
    ok(&mut s, "history.undo", json!({}));
    assert!(!s.can_undo(None), "only add_text was an undo step");
}

#[test]
fn coalesce_merges_consecutive_same_key_edits() {
    let mut s = session_with_doc();
    let id = add_text(&mut s, "a");
    for x in 2..6 {
        ok(
            &mut s,
            "element.move",
            json!({"id": id, "x_mm": x, "y_mm": 1, "coalesce": "drag"}),
        );
    }
    ok(&mut s, "history.undo", json!({}));
    assert_eq!(elements(&s)[0]["x_mm"], 1.0, "four moves were one step");

    // A different key (or no key) breaks the chain.
    ok(&mut s, "history.redo", json!({}));
    ok(
        &mut s,
        "element.move",
        json!({"id": id, "x_mm": 8, "y_mm": 1, "coalesce": "drag"}),
    );
    ok(
        &mut s,
        "element.move",
        json!({"id": id, "x_mm": 9, "y_mm": 1}),
    );
    ok(&mut s, "history.undo", json!({}));
    assert_eq!(elements(&s)[0]["x_mm"], 8.0);
    ok(&mut s, "history.undo", json!({}));
    assert_eq!(elements(&s)[0]["x_mm"], 5.0);
}

#[test]
fn undo_cap_is_200() {
    let mut s = session_with_doc();
    let id = add_text(&mut s, "a");
    for i in 0..250 {
        ok(
            &mut s,
            "element.move",
            json!({"id": id, "x_mm": i, "y_mm": 0}),
        );
    }
    let mut n = 0;
    while s.execute("history.undo", json!({})).ok {
        n += 1;
    }
    assert_eq!(n, 200);
}

#[test]
fn per_document_history() {
    let mut s = session_with_doc();
    let first = s.active_doc_id().unwrap().to_owned();
    add_text(&mut s, "a");
    let second = ok(&mut s, "doc.new", json!({"width_mm": 20, "height_mm": 10}))["doc_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(first, second);
    assert!(!s.can_undo(Some(&second)));
    ok(&mut s, "history.undo", json!({"doc_id": first}));
    assert_eq!(s.doc(Some(&first)).unwrap().elements().count(), 0);
    let e = err(&mut s, "history.undo", json!({"doc_id": "nope"}));
    assert_eq!(e.code, ErrorCode::NotFound);
    assert!(e.accepted.unwrap().contains(&first));
}
