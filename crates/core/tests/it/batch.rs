use labely_core::ErrorCode;
use serde_json::json;

use crate::{add_text, elements, err, ok, session_with_doc};

#[test]
fn refs_and_single_undo_step() {
    let mut s = session_with_doc();
    let r = ok(
        &mut s,
        "command.batch",
        json!({"label": "make label", "steps": [
            {"id": "element.add_text", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 20, "height_mm": 5, "text": "$$5.00"}},
            {"id": "element.move", "params": {"id": "$0.id", "x_mm": 3, "y_mm": 4}},
            {"id": "element.duplicate", "params": {"id": "$last.element.id"}},
            {"id": "element.set_props", "params": {"id": "$2.id", "props": {"text": "copy"}}},
            {"id": "element.rotate", "params": {"id": "$0.created.0", "rotation_deg": 90}}
        ]}),
    );
    assert_eq!(r["completed"], 5);
    assert_eq!(r["failed"], 0);
    assert_eq!(r["created"], json!([1, 2]));
    let els = elements(&s);
    assert_eq!(els[0]["text"], "$5.00");
    assert_eq!(els[0]["x_mm"], 3.0);
    assert_eq!(els[0]["rotation_deg"], 90.0);
    assert_eq!(els[1]["text"], "copy");

    let u = ok(&mut s, "history.undo", json!({}));
    assert_eq!(u["label"], "make label");
    assert!(elements(&s).is_empty());
    assert!(!s.can_undo(None));
    ok(&mut s, "history.redo", json!({}));
    assert_eq!(elements(&s).len(), 2);
}

#[test]
fn atomic_failure_rolls_back_everything() {
    let mut s = session_with_doc();
    add_text(&mut s, "keep");
    let before = s.doc(None).unwrap().clone();
    let journal = s.journal().len();
    let e = err(
        &mut s,
        "command.batch",
        json!({"steps": [
            {"id": "element.add_text", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 20, "height_mm": 5, "text": "x"}},
            {"id": "doc.new", "params": {"width_mm": 10, "height_mm": 10}},
            {"id": "element.move", "params": {"id": 999, "x_mm": 0, "y_mm": 0}}
        ]}),
    );
    assert_eq!(e.code, ErrorCode::NotFound);
    assert!(
        e.message.starts_with("step 2 (element.move) failed:"),
        "{}",
        e.message
    );
    assert!(e.message.ends_with("nothing was changed"), "{}", e.message);
    assert_eq!(*s.doc(None).unwrap(), before);
    assert_eq!(s.doc_ids().count(), 1, "doc.new was rolled back");
    assert_eq!(s.journal().len(), journal);
    ok(&mut s, "history.undo", json!({}));
    assert!(!s.can_undo(None));
}

#[test]
fn params_are_validated_before_running() {
    let mut s = session_with_doc();
    let e = err(
        &mut s,
        "command.batch",
        json!({"steps": [
            {"id": "element.add_text", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 20, "height_mm": 5, "text": "x"}},
            {"id": "element.add_rect", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 20}}
        ]}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);
    assert!(
        e.message.starts_with("step 1 (element.add_rect) failed"),
        "{}",
        e.message
    );
    assert_eq!(e.param.as_deref(), Some("height_mm"));
    assert!(elements(&s).is_empty());
}

#[test]
fn unresolved_ref_fails_before_step() {
    let mut s = session_with_doc();
    let e = err(
        &mut s,
        "command.batch",
        json!({"steps": [
            {"id": "element.add_text", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 20, "height_mm": 5, "text": "x"}},
            {"id": "element.move", "params": {"id": "$0.nope", "x_mm": 0, "y_mm": 0}}
        ]}),
    );
    assert!(
        e.message.contains("unresolved reference `$0.nope`"),
        "{}",
        e.message
    );
    assert_eq!(e.param.as_deref(), Some("steps.1.params"));
    assert!(elements(&s).is_empty());
}

#[test]
fn refused_commands_in_atomic_batch() {
    let mut s = session_with_doc();
    for id in [
        "print.submit",
        "device.scan",
        "doc.open",
        "doc.close",
        "command.batch",
        "history.undo",
    ] {
        let e = err(
            &mut s,
            "command.batch",
            json!({"steps": [{"id": id, "params": {}}]}),
        );
        assert_eq!(e.code, ErrorCode::BadParams, "{id}");
        assert!(
            e.message.contains("cannot run inside an atomic batch"),
            "{id}: {}",
            e.message
        );
        assert_eq!(e.param.as_deref(), Some("steps.0.id"));
    }
    let e = err(
        &mut s,
        "command.batch",
        json!({"steps": [{"id": "element.ad_text", "params": {}}]}),
    );
    assert_eq!(e.code, ErrorCode::UnknownCommand);
    assert!(e.accepted.unwrap().contains(&"element.add_text".to_owned()));
}

#[test]
fn non_atomic_keeps_successes_and_reports_failures() {
    let mut s = session_with_doc();
    let r = ok(
        &mut s,
        "command.batch",
        json!({"atomic": false, "stop_on_error": false, "steps": [
            {"id": "element.add_text", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 20, "height_mm": 5, "text": "x"}},
            {"id": "element.move", "params": {"id": 42, "x_mm": 0, "y_mm": 0}},
            {"id": "element.move", "params": {"id": "$1.id", "x_mm": 0, "y_mm": 0}},
            {"id": "doc.get", "params": {"terse": true}}
        ]}),
    );
    assert_eq!(r["completed"], 2);
    assert_eq!(r["failed"], 2);
    assert_eq!(r["results"][1]["error"]["code"], "not_found");
    assert!(
        r["results"][2]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("step 1 failed")
    );
    assert!(r["results"][3].get("result").is_none() || r["results"][3]["result"].is_null());
    assert_eq!(elements(&s).len(), 1);
    // Each successful edit is its own undo step.
    ok(&mut s, "history.undo", json!({}));
    assert!(!s.can_undo(None));
}

#[test]
fn terse_batch_and_limits() {
    let mut s = session_with_doc();
    let env = s.execute(
        "command.batch",
        json!({"terse": true, "steps": [
            {"id": "element.add_rect", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 2, "height_mm": 2}}
        ]}),
    );
    assert!(env.ok);
    assert_eq!(env.created.len(), 1);
    let r = env.result.unwrap();
    assert!(r["results"][0].get("result").is_none());

    let steps: Vec<_> = (0..129)
        .map(|_| json!({"id": "doc.get", "params": {}}))
        .collect();
    let e = err(&mut s, "command.batch", json!({"steps": steps}));
    assert!(e.message.contains("limit of 128"));
    let e = err(&mut s, "command.batch", json!({"steps": []}));
    assert_eq!(e.param.as_deref(), Some("steps"));
}

#[test]
fn batch_steps_are_journaled_individually() {
    let mut s = session_with_doc();
    let n = s.journal().len();
    ok(
        &mut s,
        "command.batch",
        json!({"steps": [
            {"id": "element.add_rect", "params": {"x_mm": 1, "y_mm": 1, "width_mm": 2, "height_mm": 2}},
            {"id": "element.move", "params": {"id": "$0.id", "x_mm": 3, "y_mm": 3}}
        ]}),
    );
    let j = &s.journal()[n..];
    assert_eq!(j.len(), 2);
    assert_eq!(j[1].id, "element.move");
    assert_eq!(j[1].params["id"], 1, "refs are journaled resolved");
}
