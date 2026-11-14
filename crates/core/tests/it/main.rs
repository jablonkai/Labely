//! Integration tests for labely-core, driven through `Session::execute` with testkit fakes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod batch;
mod commands;
mod data;
mod device;
mod elements;
mod file;
mod history;
mod phase2;
mod schema;

use labely_core::{CommandError, Session};
use serde_json::{Value, json};

/// Execute and return `result`, failing the test on `ok:false`.
pub fn ok(s: &mut Session, id: &str, params: Value) -> Value {
    let e = s.execute(id, params);
    assert!(e.ok, "{id} failed: {:?}", e.error);
    e.result.unwrap_or(Value::Null)
}

/// Execute and return the error, failing the test on `ok:true`.
pub fn err(s: &mut Session, id: &str, params: Value) -> CommandError {
    let e = s.execute(id, params);
    assert!(!e.ok, "{id} unexpectedly succeeded: {:?}", e.result);
    e.error.unwrap()
}

/// Session with one 40×30 mm document on the fake model.
pub fn session_with_doc() -> Session {
    let mut s = labely_testkit::test_session();
    ok(
        &mut s,
        "doc.new",
        json!({"width_mm": 40, "height_mm": 30, "printer_model": "test.fake"}),
    );
    s
}

pub fn add_text(s: &mut Session, text: &str) -> u64 {
    let r = ok(
        s,
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 30, "height_mm": 8, "text": text}),
    );
    r["id"].as_u64().unwrap()
}

pub fn elements(s: &Session) -> Vec<Value> {
    let d = serde_json::to_value(s.doc(None).unwrap()).unwrap();
    d["pages"][0]["elements"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}
