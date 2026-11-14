use labely_core::ErrorCode;
use serde_json::json;

use crate::{err, ok, session_with_doc};

fn table(s: &mut labely_core::Session) -> u64 {
    ok(
        s,
        "element.add_table",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 30, "height_mm": 20,
               "cells": [["a", "b", "c"], ["d"]]}),
    )["id"]
        .as_u64()
        .unwrap()
}

fn edit(s: &mut labely_core::Session, id: u64, e: serde_json::Value) -> serde_json::Value {
    ok(s, "element.table_edit", json!({"id": id, "edit": e}))["element"].clone()
}

#[test]
fn add_table_shapes_and_cells() {
    let mut s = session_with_doc();
    let id = table(&mut s);
    let e = &s
        .doc(None)
        .unwrap()
        .element(labely_core::ElementId(id))
        .unwrap()
        .clone();
    let v = serde_json::to_value(e).unwrap();
    assert_eq!(v["type"], "table");
    assert_eq!(v["column_widths"], json!([1.0, 1.0, 1.0]));
    assert_eq!(v["cells"][1][0]["text"], "d");
    assert_eq!(v["cells"][1][2], json!({}), "empty cells serialise as {{}}");
    let r = ok(
        &mut s,
        "element.add_table",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 30, "height_mm": 20, "row_count": 2, "column_count": 4}),
    );
    assert!(r["id"].is_u64());
    let e = err(
        &mut s,
        "element.add_table",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 30, "height_mm": 20, "row_count": 0}),
    );
    assert_eq!(e.param.as_deref(), Some("cells"));
}

#[test]
fn table_edits_rows_columns_and_merges() {
    let mut s = session_with_doc();
    let id = table(&mut s);
    let v = edit(
        &mut s,
        id,
        json!({"op": "merge", "row": 0, "col": 1, "rows": 2, "cols": 2}),
    );
    assert_eq!(
        v["merges"],
        json!([{"row": 0, "col": 1, "rows": 2, "cols": 2}])
    );
    // Inserting a column inside the merge widens it; before it shifts it.
    let v = edit(&mut s, id, json!({"op": "insert_column", "at": 2}));
    assert_eq!(v["merges"][0]["cols"], 3);
    let v = edit(
        &mut s,
        id,
        json!({"op": "insert_row", "at": 0, "height": 2.0}),
    );
    assert_eq!(v["merges"][0]["row"], 1);
    assert_eq!(v["row_heights"], json!([2.0, 1.0, 1.0]));
    // Deleting rows shrinks the merge until it is gone.
    edit(&mut s, id, json!({"op": "delete_row", "at": 1}));
    let v = edit(&mut s, id, json!({"op": "delete_column", "at": 1}));
    assert_eq!(
        v["merges"],
        json!([{"row": 1, "col": 1, "rows": 1, "cols": 2}])
    );
    let v = edit(&mut s, id, json!({"op": "split", "row": 1, "col": 2}));
    assert!(v.get("merges").is_none());

    let v = edit(
        &mut s,
        id,
        json!({"op": "set_cell", "row": 0, "col": 0, "text": "{{x}}", "bold": true, "align": "right"}),
    );
    assert_eq!(
        v["cells"][0][0],
        json!({"text": "{{x}}", "bold": true, "align": "right"})
    );
    let v = edit(
        &mut s,
        id,
        json!({"op": "resize_column", "at": 0, "width": 3}),
    );
    assert_eq!(v["column_widths"][0], 3.0);
    let v = edit(&mut s, id, json!({"op": "clear"}));
    assert!(v["cells"].to_string().find("text").is_none());

    // Errors.
    let e = err(
        &mut s,
        "element.table_edit",
        json!({"id": id, "edit": {"op": "delete_row", "at": 9}}),
    );
    assert!(e.message.contains("out of range"));
    let e = err(
        &mut s,
        "element.table_edit",
        json!({"id": id, "edit": {"op": "resize_row", "at": 0, "height": 0}}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);
    edit(
        &mut s,
        id,
        json!({"op": "merge", "row": 0, "col": 0, "rows": 2, "cols": 2}),
    );
    let e = err(
        &mut s,
        "element.table_edit",
        json!({"id": id, "edit": {"op": "merge", "row": 1, "col": 1, "rows": 1, "cols": 2}}),
    );
    assert!(e.message.contains("overlaps"));
    let e = err(
        &mut s,
        "element.table_edit",
        json!({"id": id, "edit": {"op": "split", "row": 2, "col": 2}}),
    );
    assert_eq!(e.code, ErrorCode::NotFound);
    let t = crate::add_text(&mut s, "x");
    let e = err(
        &mut s,
        "element.table_edit",
        json!({"id": t, "edit": {"op": "clear"}}),
    );
    assert!(e.message.contains("not a table"));
    // A table edit is one undo step.
    let before = s.doc(None).unwrap().clone();
    edit(&mut s, id, json!({"op": "insert_row", "at": 0}));
    ok(&mut s, "history.undo", json!({}));
    assert_eq!(*s.doc(None).unwrap(), before);
}

#[test]
fn table_cells_take_bindings() {
    let mut kit = labely_testkit::test_kit();
    let s = &mut kit.session;
    ok(s, "doc.new", json!({"width_mm": 40, "height_mm": 30}));
    ok(
        s,
        "variable.set",
        json!({"name": "now", "value": "2026-05-06"}),
    );
    ok(
        s,
        "element.add_table",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 30, "height_mm": 20, "cells": [["{{now:%d/%m}}", "{{nope}}"]]}),
    );
    ok(s, "render.preview", json!({}));
    assert_eq!(
        kit.renderer.rendered_values().pop().unwrap()["now:%d/%m"],
        "06/05"
    );
    let issues = ok(s, "doc.check", json!({}))["issues"].clone();
    assert!(issues.to_string().contains("binding_missing"), "{issues}");
}

#[test]
fn icons_borders_and_decode() {
    let mut s = session_with_doc();
    let r = ok(&mut s, "icon.list", json!({"category": "handling"}));
    assert_eq!(
        r["icons"],
        json!([{"name": "fragile", "category": "handling"}])
    );
    ok(
        &mut s,
        "element.add_icon",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 8, "height_mm": 8, "icon": "check"}),
    );
    let e = err(
        &mut s,
        "element.add_icon",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 8, "height_mm": 8, "icon": "nope"}),
    );
    assert_eq!(e.code, ErrorCode::NotFound);
    assert!(e.accepted.unwrap().contains(&"fragile".to_owned()));
    let b = ok(
        &mut s,
        "element.add_border",
        json!({"x_mm": 0, "y_mm": 0, "width_mm": 40, "height_mm": 30, "style": "ticket"}),
    )["id"]
        .as_u64()
        .unwrap();
    let r = ok(
        &mut s,
        "element.set_props",
        json!({"id": b, "props": {"style": "double", "stroke_mm": 0.8}}),
    );
    assert_eq!(r["element"]["style"], "double");

    use base64::Engine;
    let img =
        base64::engine::general_purpose::STANDARD.encode("qr:https://x.y\nean13:4006381333931");
    let r = ok(&mut s, "code.decode", json!({"data_base64": img}));
    assert_eq!(
        r["codes"],
        json!([
            {"kind": "code2d", "symbology": "qr", "text": "https://x.y"},
            {"kind": "barcode", "symbology": "ean13", "text": "4006381333931"}
        ])
    );
    let e = err(&mut s, "code.decode", json!({}));
    assert_eq!(e.code, ErrorCode::BadParams);
}
