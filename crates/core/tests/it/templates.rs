//! Slice 3.4: templates, export, image records (ADR-0018).

use labely_core::ErrorCode;
use labely_core::commands::template::STARTERS;
use serde_json::{Value, json};

use crate::{err, ok, session_with_doc};

fn library() -> (labely_core::Session, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut s = labely_testkit::test_session();
    s.set_template_dir(Some(dir.path().join("templates")));
    (s, dir)
}

fn refs(s: &mut labely_core::Session, params: Value) -> Vec<String> {
    ok(s, "template.list", params)["templates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["ref"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn starters_load_and_pass_check() {
    let mut s = labely_testkit::test_session();
    let all = refs(&mut s, json!({}));
    assert_eq!(all.len(), STARTERS.len(), "no library: starters only");
    for r in all {
        let out = ok(&mut s, "template.new", json!({ "ref": r }));
        assert!(out["doc_id"].as_str().is_some());
        let check = ok(&mut s, "doc.check", json!({}));
        assert_eq!(check["ok"], true, "{r}: {check}");
        assert!(!s.is_dirty(None) || s.doc_path(None).is_none());
    }
    let e = err(&mut s, "template.new", json!({"ref": "starter:nope"}));
    assert_eq!(e.code, ErrorCode::NotFound);
    assert!(e.accepted.unwrap().contains(&"starter:jar".to_owned()));
    // The library needs a directory.
    let e = err(&mut s, "template.save", json!({"name": "x"}));
    assert_eq!(e.code, ErrorCode::Disabled);
}

#[test]
fn library_save_list_move_duplicate_delete() {
    let (mut s, _dir) = library();
    ok(
        &mut s,
        "template.new",
        json!({"ref": "starter:price-tag", "name": "Shelf"}),
    );
    assert_eq!(s.doc(None).unwrap().name, "Shelf");
    assert_eq!(
        ok(&mut s, "template.save", json!({"folder": "Shop"}))["ref"],
        "my:Shop/Shelf"
    );
    let e = err(&mut s, "template.save", json!({"folder": "Shop"}));
    assert_eq!(e.code, ErrorCode::BadParams);
    assert!(e.message.contains("already exists"), "{}", e.message);
    ok(
        &mut s,
        "template.save",
        json!({"folder": "Shop", "overwrite": true}),
    );

    let list = ok(&mut s, "template.list", json!({"folder": "Shop"}));
    assert_eq!(list["folders"], json!(["Shop"]));
    assert_eq!(list["templates"][0]["name"], "Shelf");
    assert_eq!(list["templates"][0]["starter"], false);
    assert_eq!(list["templates"].as_array().unwrap().len(), 1);
    assert_eq!(refs(&mut s, json!({"query": "SHEL"})), ["my:Shop/Shelf"]);

    // Duplicate numbers the copy; starters can be copied into the library.
    let r = ok(
        &mut s,
        "template.duplicate",
        json!({"ref": "my:Shop/Shelf"}),
    );
    assert_eq!(r["ref"], "my:Shop/Shelf 2");
    let r = ok(&mut s, "template.duplicate", json!({"ref": "starter:jar"}));
    assert_eq!(r["ref"], "my:Jar");

    // Rename and move to the default folder.
    let r = ok(
        &mut s,
        "template.move",
        json!({"ref": "my:Shop/Shelf 2", "name": "Bin", "folder": ""}),
    );
    assert_eq!(r["ref"], "my:Bin");
    let e = err(
        &mut s,
        "template.move",
        json!({"ref": "my:Bin", "name": "Jar"}),
    );
    assert!(e.message.contains("already exists"));

    // template.new from the library takes the file name.
    let out = ok(&mut s, "template.new", json!({"ref": "my:Bin"}));
    assert_eq!(out["name"], "Bin");

    // Starters are read-only; bad names are refused.
    let e = err(&mut s, "template.delete", json!({"ref": "starter:jar"}));
    assert!(e.hint.unwrap().contains("template.duplicate"));
    for bad in ["../evil", "a/b/c", ".hidden", "x:y", ""] {
        let e = err(&mut s, "template.save", json!({ "name": bad }));
        assert_eq!(e.code, ErrorCode::BadParams, "{bad}");
    }
    let e = err(
        &mut s,
        "template.new",
        json!({"ref": "my:../../etc/passwd"}),
    );
    assert_eq!(e.code, ErrorCode::BadParams);

    ok(&mut s, "template.delete", json!({"ref": "my:Bin"}));
    let e = err(&mut s, "template.delete", json!({"ref": "my:Bin"}));
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn folders() {
    let (mut s, _dir) = library();
    let r = ok(
        &mut s,
        "template.folder",
        json!({"op": "create", "folder": "Kitchen"}),
    );
    assert_eq!(r["folders"], json!(["Kitchen"]));
    err(
        &mut s,
        "template.folder",
        json!({"op": "create", "folder": "Kitchen"}),
    );
    ok(&mut s, "template.new", json!({"ref": "starter:jar"}));
    ok(&mut s, "template.save", json!({"folder": "Kitchen"}));
    let r = ok(
        &mut s,
        "template.folder",
        json!({"op": "rename", "folder": "Kitchen", "to": "Pantry"}),
    );
    assert_eq!(r["folders"], json!(["Pantry"]));
    assert_eq!(refs(&mut s, json!({"folder": "Pantry"})), ["my:Pantry/Jar"]);
    let e = err(
        &mut s,
        "template.folder",
        json!({"op": "delete", "folder": "Pantry"}),
    );
    assert!(e.message.contains("1 template"), "{}", e.message);
    let r = ok(
        &mut s,
        "template.folder",
        json!({"op": "delete", "folder": "Pantry", "with_templates": true}),
    );
    assert_eq!(r["folders"], json!([]));
    let e = err(
        &mut s,
        "template.folder",
        json!({"op": "delete", "folder": "Pantry"}),
    );
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn library_writes_are_not_journaled() {
    let (mut s, _dir) = library();
    ok(&mut s, "template.new", json!({"ref": "starter:jar"}));
    ok(&mut s, "template.save", json!({}));
    ok(
        &mut s,
        "template.folder",
        json!({"op": "create", "folder": "A"}),
    );
    let ids: Vec<_> = s.journal().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["template.new"]);
}

#[test]
fn export_png_and_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = session_with_doc();
    ok(&mut s, "data.set_serial", json!({"name": "n"}));
    crate::add_text(&mut s, "No. {{n}}");
    let png = dir.path().join("a.png");
    let r = ok(&mut s, "export.png", json!({ "path": png }));
    assert_eq!(r["dpi"], 203);
    assert!(std::fs::read(&png).unwrap().starts_with(b"\x89PNG"));

    let pdf = dir.path().join("a.pdf");
    let r = ok(&mut s, "export.pdf", json!({ "path": pdf, "count": 3 }));
    assert_eq!(r["pages"], 3);
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF-1.4"));
    assert_eq!(r["bytes"], bytes.len());
    let e = err(&mut s, "export.pdf", json!({ "path": pdf, "records": "1" }));
    assert_eq!(e.param.as_deref(), Some("records"));

    // With data: one page per selected record.
    ok(&mut s, "data.attach_csv", json!({"text": "a\n1\n2\n3\n"}));
    let r = ok(
        &mut s,
        "export.pdf",
        json!({ "path": pdf, "records": "1,3" }),
    );
    assert_eq!(r["pages"], 2);
    err(&mut s, "export.pdf", json!({ "path": pdf, "count": 2 }));

    // Agents stay inside their roots.
    s.set_policy(labely_core::Policy::restricted(vec![dir.path().into()]));
    let e = err(&mut s, "export.png", json!({"path": "/tmp/../etc/x.png"}));
    assert_eq!(e.code, ErrorCode::Forbidden);
}

/// A tiny valid PNG (1×1, grey) with a distinguishing byte in an ancillary chunk.
fn png(tag: u8) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
    v.extend([0, 0, 0, 1, b't', b'E', b'X', b't', tag, 0, 0, 0, 0]);
    v
}

#[test]
fn attach_images_as_records() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..3)
        .map(|i| {
            let p = dir.path().join(format!("img{i}.png"));
            std::fs::write(&p, png(i)).unwrap();
            p
        })
        .collect();
    let mut s = session_with_doc();
    let r = ok(&mut s, "data.attach_images", json!({ "paths": paths }));
    assert_eq!(r["columns"], json!(["image", "file"]));
    assert_eq!(r["row_count"], 3);
    assert_eq!(r["origin"], "3 images");
    assert_eq!(r["preview"][1][1], "img1.png");
    let id = r["element_id"].as_u64().unwrap();
    let doc = s.doc(None).unwrap();
    assert_eq!(doc.assets.len(), 3);
    let e = doc.element(labely_core::ElementId(id)).unwrap();
    assert_eq!((e.width_mm, e.height_mm), (40.0, 30.0));
    let v = serde_json::to_value(&e.kind).unwrap();
    assert_eq!(v["asset"], "{{image}}");

    // Re-attaching keeps the bound element, and stale assets are pruned.
    let r = ok(&mut s, "data.attach_images", json!({ "paths": [paths[0]] }));
    assert!(r.get("element_id").is_none());
    assert_eq!(s.doc(None).unwrap().assets.len(), 1);
    ok(&mut s, "data.detach", json!({}));
    assert!(s.doc(None).unwrap().assets.is_empty());

    let e = err(&mut s, "data.attach_images", json!({"paths": []}));
    assert_eq!(e.param.as_deref(), Some("paths"));
    let txt = dir.path().join("x.txt");
    std::fs::write(&txt, "hello").unwrap();
    let e = err(
        &mut s,
        "data.attach_images",
        json!({ "paths": [paths[0], txt] }),
    );
    assert_eq!(e.param.as_deref(), Some("paths.1"));
    let e = err(
        &mut s,
        "data.attach_images",
        json!({ "paths": [paths[0]], "column": "file" }),
    );
    assert_eq!(e.param.as_deref(), Some("column"));
}
