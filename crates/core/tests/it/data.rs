use labely_core::ErrorCode;
use labely_testkit::{FAKE_ADDRESS, test_kit};
use serde_json::json;

use crate::{add_text, err, ok, session_with_doc};

const CSV: &str = "name;sku;qty\nAda;A-1;2\nBob;B-2;0\nCy;;1\n";

#[test]
fn attach_csv_get_detach() {
    let mut s = session_with_doc();
    let r = ok(&mut s, "data.attach_csv", json!({"text": CSV}));
    assert_eq!(r["columns"], json!(["name", "sku", "qty"]));
    assert_eq!(r["row_count"], 3);
    assert_eq!(r["preview"][0], json!(["Ada", "A-1", "2"]));

    let g = ok(&mut s, "data.get", json!({"from": 2, "limit": 1}));
    assert_eq!(g["attached"], true);
    assert_eq!(g["rows"], json!([["Bob", "B-2", "0"]]));

    // Replacing warns; undo restores the previous source.
    let e = s.execute(
        "data.attach_csv",
        json!({"text": "a,b\n1,2\n", "header": true}),
    );
    assert!(e.warnings.iter().any(|w| w.contains("replaced")));
    ok(&mut s, "history.undo", json!({}));
    assert_eq!(s.doc(None).unwrap().data.as_ref().unwrap().columns.len(), 3);

    assert_eq!(ok(&mut s, "data.detach", json!({}))["removed"], true);
    assert_eq!(ok(&mut s, "data.get", json!({}))["attached"], false);

    let e = err(&mut s, "data.attach_csv", json!({}));
    assert_eq!(e.code, ErrorCode::BadParams);
    let e = err(
        &mut s,
        "data.attach_csv",
        json!({"text": "a", "delimiter": "ab"}),
    );
    assert_eq!(e.param.as_deref(), Some("delimiter"));
}

#[test]
fn attach_csv_from_file_respects_roots() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d.csv");
    std::fs::write(&path, "x,y\n1,2\n").unwrap();
    let mut s = session_with_doc();
    let r = ok(
        &mut s,
        "data.attach_csv",
        json!({"path": path.to_str().unwrap()}),
    );
    assert_eq!(r["origin"], "d.csv");
    let e = err(
        &mut s,
        "data.attach_csv",
        json!({"path": dir.path().join("nope.csv").to_str().unwrap()}),
    );
    assert_eq!(e.code, ErrorCode::NotFound);
    let e = err(
        &mut s,
        "data.attach_xlsx",
        json!({"path": path.to_str().unwrap()}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);
}

#[test]
fn bind_and_preview_record() {
    let mut s = session_with_doc();
    let t = add_text(&mut s, "x");
    let e = err(&mut s, "data.bind", json!({"id": t, "column": "name"}));
    assert_eq!(e.code, ErrorCode::NotFound);
    ok(&mut s, "data.attach_csv", json!({"text": CSV}));
    let r = ok(&mut s, "data.bind", json!({"id": t, "column": "name"}));
    assert_eq!(r["property"], "text");
    assert_eq!(r["value"], "{{name}}");
    let q = ok(
        &mut s,
        "element.add_code2d",
        json!({"x_mm": 1, "y_mm": 10, "width_mm": 15, "height_mm": 15, "data": "x"}),
    )["id"]
        .as_u64()
        .unwrap();
    assert_eq!(
        ok(&mut s, "data.bind", json!({"id": q, "column": "sku"}))["property"],
        "data"
    );
    let r = ok(
        &mut s,
        "element.add_rect",
        json!({"x_mm": 0, "y_mm": 0, "width_mm": 2, "height_mm": 2}),
    )["id"]
        .as_u64()
        .unwrap();
    let e = err(&mut s, "data.bind", json!({"id": r, "column": "name"}));
    assert_eq!(e.param.as_deref(), Some("id"));

    ok(
        &mut s,
        "data.set_serial",
        json!({"start": 100, "digits": 5}),
    );
    let p = ok(&mut s, "data.preview_record", json!({"record": 2}));
    assert_eq!(p["values"]["name"], "Bob");
    assert_eq!(p["values"]["serial"], "00101");
    let e = err(&mut s, "data.preview_record", json!({"record": 4}));
    assert_eq!(e.param.as_deref(), Some("record"));

    // render.preview fills the record.
    let mut kit = test_kit();
    let s2 = &mut kit.session;
    ok(s2, "doc.new", json!({"width_mm": 40, "height_mm": 30}));
    ok(s2, "data.attach_csv", json!({"text": CSV}));
    ok(s2, "render.preview", json!({"record": 3}));
    assert_eq!(kit.renderer.rendered_values().last().unwrap()["name"], "Cy");
    let e = err(s2, "render.preview", json!({"record": 9}));
    assert_eq!(e.param.as_deref(), Some("record"));
}

#[test]
fn find_record_by_value() {
    let mut s = session_with_doc();
    let e = err(&mut s, "data.find_record", json!({"value": "B-2"}));
    assert_eq!(e.code, ErrorCode::BadParams);
    ok(
        &mut s,
        "data.attach_csv",
        json!({"text": "name;sku;qty\nAda;A-1;2\nBob;B-2;0\nCy; B-2 ;1\nDi;D-4;B-2\n"}),
    );
    // Any column: Bob, Cy (trimmed) and Di (in qty) match; Bob is first.
    let r = ok(&mut s, "data.find_record", json!({"value": " B-2\n"}));
    assert_eq!(r["record"], 2);
    assert_eq!(r["column"], "sku");
    assert_eq!(r["match_count"], 3);
    assert_eq!(r["matches"], json!([2, 3, 4]));
    let r = ok(
        &mut s,
        "data.find_record",
        json!({"value": "B-2", "column": "qty"}),
    );
    assert_eq!(r["record"], 4);
    assert_eq!(r["matches"], json!([4]));
    // Exact, case-sensitive.
    let e = err(&mut s, "data.find_record", json!({"value": "b-2"}));
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(e.param.as_deref(), Some("value"));
    let e = err(
        &mut s,
        "data.find_record",
        json!({"value": "B-2", "column": "nope"}),
    );
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(e.param.as_deref(), Some("column"));
    let e = err(&mut s, "data.find_record", json!({"value": "  "}));
    assert_eq!(e.param.as_deref(), Some("value"));
}

#[test]
fn serial_set_update_remove() {
    let mut s = session_with_doc();
    let r = ok(
        &mut s,
        "data.set_serial",
        json!({"name": "no", "start": 9, "step": 2, "digits": 3, "prefix": "#"}),
    );
    assert_eq!(r["sample"], json!(["#009", "#011", "#013"]));
    // Partial update keeps the other fields.
    let r = ok(&mut s, "data.set_serial", json!({"name": "no", "step": 1}));
    assert_eq!(r["sample"], json!(["#009", "#010", "#011"]));
    let e = err(&mut s, "data.set_serial", json!({"name": "{x}"}));
    assert_eq!(e.param.as_deref(), Some("name"));
    let e = err(&mut s, "data.remove_serial", json!({"name": "nope"}));
    assert_eq!(e.code, ErrorCode::NotFound);
    ok(&mut s, "data.remove_serial", json!({"name": "no"}));
    assert!(s.doc(None).unwrap().serials.is_empty());
}

#[test]
fn check_reports_data_bindings() {
    let mut s = session_with_doc();
    let t = add_text(&mut s, "{{sku}} {{nope}} {{serial}}");
    ok(&mut s, "data.attach_csv", json!({"text": CSV}));
    ok(&mut s, "data.set_serial", json!({}));
    let issues = ok(&mut s, "doc.check", json!({}))["issues"].clone();
    let rules: Vec<_> = issues
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["element_id"] == t)
        .map(|i| (i["rule"].as_str().unwrap(), i["message"].as_str().unwrap()))
        .collect();
    assert!(
        rules
            .iter()
            .any(|(r, m)| *r == "binding_empty" && m.contains("1 of 3") && m.contains("record 3")),
        "{rules:?}"
    );
    assert!(
        rules
            .iter()
            .any(|(r, m)| *r == "binding_missing" && m.contains("nope"))
    );
    assert!(
        !rules
            .iter()
            .any(|(r, m)| r.starts_with("binding") && m.contains("{{serial}}"))
    );
}

#[test]
fn print_records_copies_and_serials() {
    let mut kit = test_kit();
    let s = &mut kit.session;
    ok(
        s,
        "doc.new",
        json!({"width_mm": 40, "height_mm": 20, "printer_model": "test.fake"}),
    );
    add_text(s, "{{name}} {{serial}}");
    ok(s, "device.connect", json!({"address": FAKE_ADDRESS}));

    // Without data: count labels, serial advances.
    ok(s, "data.set_serial", json!({"start": 1}));
    kit.renderer.clear_values();
    let r = ok(s, "print.submit", json!({"count": 3, "copies": 2}));
    assert_eq!(r["labels"], 3);
    let serials: Vec<_> = kit
        .renderer
        .rendered_values()
        .iter()
        .map(|v| v["serial"].clone())
        .collect();
    assert_eq!(serials, ["1", "2", "3"]);
    let (_, job) = kit.devices.prints().last().unwrap().clone();
    assert_eq!((job.pages.len(), job.copies), (3, 2));
    let e = err(s, "print.submit", json!({"records": "1"}));
    assert!(e.message.contains("data source"));

    // With data: a range, in order; serial is the position in the job.
    ok(s, "data.attach_csv", json!({"text": CSV}));
    kit.renderer.clear_values();
    let r = ok(s, "print.submit", json!({"records": "3,1"}));
    assert_eq!(r["labels"], 2);
    let v = kit.renderer.rendered_values();
    assert_eq!(
        (v[0]["name"].as_str(), v[0]["serial"].as_str()),
        ("Cy", "1")
    );
    assert_eq!(
        (v[1]["name"].as_str(), v[1]["serial"].as_str()),
        ("Ada", "2")
    );
    let e = err(s, "print.submit", json!({"count": 2}));
    assert_eq!(e.param.as_deref(), Some("count"));
    let e = err(s, "print.submit", json!({"records": "1-9"}));
    assert_eq!(e.param.as_deref(), Some("records"));

    // Copies from a column: Bob (0) is skipped, pages are expanded.
    let r = ok(s, "print.submit", json!({"copies_column": "qty"}));
    assert_eq!(r["labels"], 2);
    let (_, job) = kit.devices.prints().last().unwrap().clone();
    assert_eq!((job.pages.len(), job.copies), (3, 1));
    let e = err(s, "print.submit", json!({"copies_column": "name"}));
    assert_eq!(e.param.as_deref(), Some("copies_column"));
}

#[test]
fn attach_xlsx_sheets_and_cells() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/items.xlsx");
    let mut s = labely_testkit::test_session();
    ok(&mut s, "doc.new", json!({"width_mm": 40, "height_mm": 30}));
    let r = ok(&mut s, "data.attach_xlsx", json!({"path": path}));
    assert_eq!(r["sheet"], "Items");
    assert_eq!(r["sheets"], json!(["Items", "Other"]));
    assert_eq!(r["columns"], json!(["name", "price", "best before"]));
    assert_eq!(r["preview"][0], json!(["Apple", "3", "2025-01-01"]));
    assert_eq!(r["preview"][1], json!(["Pear", "2.5", "2025-02-01"]));
    let r = ok(
        &mut s,
        "data.attach_xlsx",
        json!({"path": path, "sheet": "Other", "header": false}),
    );
    assert_eq!(r["columns"], json!(["A"]));
    assert_eq!(r["row_count"], 2);
    let e = err(
        &mut s,
        "data.attach_xlsx",
        json!({"path": path, "sheet": "Nope"}),
    );
    assert_eq!(
        e.accepted,
        Some(vec!["Items".to_owned(), "Other".to_owned()])
    );
}

#[test]
fn date_bindings_use_the_pinned_clock() {
    let mut kit = test_kit();
    let s = &mut kit.session;
    ok(s, "doc.new", json!({"width_mm": 40, "height_mm": 20}));
    let t = add_text(s, "{{now:%d.%m.%Y}} / {{now+1M}} / {{ now }}");
    ok(
        s,
        "variable.set",
        json!({"name": "now", "value": "2026-01-31 10:00"}),
    );
    ok(s, "render.preview", json!({}));
    let v = kit.renderer.rendered_values().pop().unwrap();
    assert_eq!(v["now:%d.%m.%Y"], "31.01.2026");
    assert_eq!(v["now+1M"], "2026-02-28");
    assert_eq!(v["now"], "2026-01-31");
    let issues = ok(s, "doc.check", json!({}))["issues"].clone();
    assert!(
        !issues
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule"].as_str().unwrap().starts_with("binding")),
        "{issues}"
    );

    ok(
        s,
        "element.set_props",
        json!({"id": t, "props": {"text": "{{now+3q}}"}}),
    );
    let issues = ok(s, "doc.check", json!({}))["issues"].clone();
    assert!(
        issues
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule"] == "binding_invalid" && i["severity"] == "error"),
        "{issues}"
    );
}

#[test]
fn flip_and_direction_props() {
    let mut s = session_with_doc();
    let t = add_text(&mut s, "x");
    let r = ok(
        &mut s,
        "element.set_props",
        json!({"id": t, "props": {"flip_h": true, "direction": "vertical"}}),
    );
    assert_eq!(r["element"]["flip_h"], true);
    assert_eq!(r["element"]["direction"], "vertical");
    assert!(
        r["element"].get("flip_v").is_none(),
        "false flips are omitted"
    );
}
