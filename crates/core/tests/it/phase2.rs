//! Selection, journal replay, session policy and the `doc.check` rules added in Phase 2.

use labely_core::services::PaperInfo;
use labely_core::{ElementId, ErrorCode, Policy};
use serde_json::json;

use crate::{add_text, err, ok, session_with_doc};

#[test]
fn selection_modes_and_view_state() {
    let mut s = session_with_doc();
    let a = add_text(&mut s, "a");
    let b = add_text(&mut s, "b");
    let sel = |s: &mut labely_core::Session| ok(s, "selection.get", json!({}))["ids"].clone();
    ok(&mut s, "selection.set", json!({"ids": [a]}));
    ok(&mut s, "selection.set", json!({"ids": [b], "mode": "add"}));
    assert_eq!(sel(&mut s), json!([a, b]));
    ok(
        &mut s,
        "selection.set",
        json!({"ids": [a], "mode": "toggle"}),
    );
    assert_eq!(sel(&mut s), json!([b]));
    ok(
        &mut s,
        "selection.set",
        json!({"ids": [b], "mode": "remove"}),
    );
    assert_eq!(sel(&mut s), json!([]));
    let e = err(&mut s, "selection.set", json!({"ids": [999]}));
    assert_eq!(e.code, ErrorCode::NotFound);

    // Not undoable, not journaled; deleted elements drop out.
    ok(&mut s, "selection.set", json!({"ids": [a, b]}));
    let undo = ok(&mut s, "history.undo", json!({}));
    assert_eq!(
        undo["label"], "element.add_text",
        "selection pushed no undo step"
    );
    assert_eq!(s.selection(None), vec![ElementId(a)]);
    let ids: Vec<_> = s.journal().iter().map(|j| j.id.as_str()).collect();
    assert!(!ids.contains(&"selection.set"));
}

#[test]
fn journal_is_a_replayable_script() {
    let mut s = session_with_doc();
    add_text(&mut s, "one");
    ok(
        &mut s,
        "element.move",
        json!({"id": 1, "x_mm": 5, "y_mm": 6}),
    );
    ok(
        &mut s,
        "variable.set",
        json!({"name": "who", "value": "Ada"}),
    );
    let j = ok(&mut s, "history.journal", json!({}));
    assert_eq!(j["total"], 4);
    let since = ok(&mut s, "history.journal", json!({"since": 3}));
    assert_eq!(since["entries"].as_array().unwrap().len(), 1);

    let mut replay = labely_testkit::test_session();
    ok(&mut replay, "command.batch", json!({"steps": j["entries"]}));
    assert_eq!(replay.doc(None), s.doc(None));
}

#[test]
fn policy_limits_files_and_printing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut s = session_with_doc();
    ok(
        &mut s,
        "device.connect",
        json!({"address": labely_testkit::FAKE_ADDRESS}),
    );
    s.set_policy(Policy::restricted(vec![root.clone()]));
    let outside = std::env::temp_dir().join("labely-outside-root.labely");
    let e = err(&mut s, "doc.save", json!({"path": outside}));
    assert_eq!(e.code, ErrorCode::Forbidden);
    assert_eq!(e.param.as_deref(), Some("path"));
    ok(&mut s, "doc.save", json!({"path": root.join("in.labely")}));
    let e = err(&mut s, "doc.open", json!({"path": "/etc/hosts"}));
    assert_eq!(e.code, ErrorCode::Forbidden);

    let e = err(
        &mut s,
        "print.submit",
        json!({"address": labely_testkit::FAKE_ADDRESS}),
    );
    assert_eq!(e.code, ErrorCode::Forbidden);
    // Also inside a non-atomic batch.
    let r = ok(
        &mut s,
        "command.batch",
        json!({"atomic": false, "steps": [{"id": "print.submit", "params": {"address": labely_testkit::FAKE_ADDRESS}}]}),
    );
    assert_eq!(r["results"][0]["error"]["code"], "forbidden", "{r}");
}

fn check_rules(s: &mut labely_core::Session, params: serde_json::Value) -> Vec<String> {
    ok(s, "doc.check", params)["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["rule"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn check_bindings_margins_dpi_and_paper() {
    let mut kit = labely_testkit::test_kit();
    let s = &mut kit.session;
    ok(
        s,
        "doc.new",
        json!({"width_mm": 40, "height_mm": 30, "printer_model": "test.fake"}),
    );
    ok(
        s,
        "element.add_text",
        json!({"x_mm": 0.5, "y_mm": 1, "width_mm": 20, "height_mm": 6, "text": "Hi {{who}} {{empty}}"}),
    );
    ok(s, "variable.set", json!({"name": "empty", "value": ""}));
    let r = check_rules(s, json!({}));
    assert!(r.contains(&"binding_missing".to_owned()), "{r:?}");
    assert!(r.contains(&"binding_empty".to_owned()), "{r:?}");
    assert!(!r.contains(&"in_margin".to_owned()));
    let r = check_rules(s, json!({"margin_mm": 1.5}));
    assert!(r.contains(&"in_margin".to_owned()), "{r:?}");
    let r = check_rules(s, json!({"dpi": 300}));
    assert!(r.contains(&"dpi_mismatch".to_owned()), "{r:?}");

    ok(
        s,
        "device.connect",
        json!({"address": labely_testkit::FAKE_ADDRESS}),
    );
    kit.devices.set_paper(Some(PaperInfo {
        barcode: "6972842743589".into(),
        serial: "x".into(),
        total: 100,
        used: 1,
        media: Some(labely_core::document::MediaType::BlackMark),
    }));
    let r = check_rules(&mut kit.session, json!({}));
    assert!(r.contains(&"paper_mismatch".to_owned()), "{r:?}");
}

#[test]
fn menus_and_kinds_are_listed() {
    let mut s = labely_testkit::test_session();
    let r = ok(&mut s, "command.list", json!({"prefix": "selection."}));
    assert_eq!(r["commands"][0]["kind"], "ui");
    assert_eq!(r["commands"][0]["menu"], "edit");
    let r = ok(&mut s, "command.list", json!({"prefix": "history.undo"}));
    assert_eq!(r["commands"][0]["shortcut"], "Ctrl+Z");
}
