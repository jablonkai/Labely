use labely_core::ErrorCode;
use serde_json::json;

use crate::{add_text, err, ok, session_with_doc};

#[test]
fn save_open_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.labely");
    let path_s = path.to_str().unwrap();

    let mut s = session_with_doc();
    add_text(&mut s, "hello");
    ok(&mut s, "variable.set", json!({"name": "n", "value": "1"}));
    let e = err(&mut s, "doc.save", json!({}));
    assert_eq!(e.param.as_deref(), Some("path"));
    let r = ok(&mut s, "doc.save", json!({"path": path_s}));
    assert!(r["bytes"].as_u64().unwrap() > 10);
    assert_eq!(ok(&mut s, "doc.get", json!({}))["dirty"], false);
    let original = s.doc(None).unwrap().clone();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.starts_with("{\n  \"format_version\": 2,"),
        "pretty JSON"
    );

    let opened = ok(&mut s, "doc.open", json!({"path": path_s}));
    let id = opened["doc_id"].as_str().unwrap().to_owned();
    assert_eq!(s.active_doc_id(), Some(id.as_str()));
    assert_eq!(*s.doc(Some(&id)).unwrap(), original);
    let g = ok(&mut s, "doc.get", json!({"doc_id": id}));
    assert_eq!(g["dirty"], false);
    assert_eq!(g["path"], path_s);

    // Save again without a path goes to the same file.
    add_text(&mut s, "more");
    assert_eq!(ok(&mut s, "doc.save", json!({}))["path"], path_s);

    let r = ok(&mut s, "doc.close", json!({}));
    assert_eq!(r["closed"], id.as_str());
    assert!(r["active"].is_string());
}

#[test]
fn open_rejects_newer_format_and_garbage() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = labely_testkit::test_session();
    let newer = dir.path().join("new.labely");
    std::fs::write(&newer, r#"{"format_version": 3, "name": "x"}"#).unwrap();
    let e = err(&mut s, "doc.open", json!({"path": newer.to_str().unwrap()}));
    assert_eq!(e.code, ErrorCode::BadParams);
    assert!(
        e.message.contains("format_version 3 is newer"),
        "{}",
        e.message
    );

    let bad = dir.path().join("bad.labely");
    std::fs::write(&bad, "not json").unwrap();
    assert_eq!(
        err(&mut s, "doc.open", json!({"path": bad.to_str().unwrap()})).code,
        ErrorCode::BadParams
    );
    let e = err(
        &mut s,
        "doc.open",
        json!({"path": dir.path().join("missing.labely").to_str().unwrap()}),
    );
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(s.doc_ids().count(), 0);
}
