//! Paper library, RFID paper detection, calibration and printer settings (ADR-0020).

use labely_core::ErrorCode;
use labely_core::document::MediaType;
use labely_core::services::PaperInfo;
use labely_testkit::{FAKE_ADDRESS, test_kit};
use serde_json::json;

use crate::{err, ok, session_with_doc};

fn rfid(barcode: &str, media: MediaType) -> Option<PaperInfo> {
    Some(PaperInfo {
        barcode: barcode.into(),
        serial: "S1".into(),
        total: 200,
        used: 20,
        media: Some(media),
    })
}

#[test]
fn bundled_papers_list_and_filter() {
    let mut s = session_with_doc();
    let all = ok(&mut s, "paper.list", json!({}));
    let n = all["papers"].as_array().unwrap().len();
    assert!(n > 30);
    let r = ok(&mut s, "paper.list", json!({"query": "50x30"}));
    let ids: Vec<_> = r["papers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_owned())
        .collect();
    assert!(ids.contains(&"std:50x30".to_owned()), "{ids:?}");
    let fits = ok(&mut s, "paper.list", json!({"model": "test.fake"}));
    assert!(
        fits["papers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["width_mm"].as_f64().unwrap() <= 48.0)
    );
    let rolls = ok(&mut s, "paper.list", json!({"media": "continuous"}));
    assert!(
        rolls["papers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["media"] == "continuous")
    );
    let e = err(&mut s, "paper.list", json!({"model": "nope"}));
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn apply_is_undoable() {
    let mut s = session_with_doc();
    let r = ok(&mut s, "paper.apply", json!({"id": "std:25x25-round"}));
    assert_eq!(r["label"]["width_mm"], 25.0);
    assert_eq!(r["label"]["corner_radius_mm"], 3.0);
    assert_eq!(
        r["label"]["printer_model"], "test.fake",
        "other fields stay"
    );
    ok(&mut s, "history.undo", json!({}));
    assert_eq!(s.doc(None).unwrap().label.width_mm, 40.0);
    let e = err(&mut s, "paper.apply", json!({"id": "std:nope"}));
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn library_needs_a_file() {
    let mut s = session_with_doc();
    let e = err(
        &mut s,
        "paper.add",
        json!({"name": "Jar", "width_mm": 38, "height_mm": 38}),
    );
    assert_eq!(e.code, ErrorCode::Disabled);
}

#[test]
fn custom_papers_and_links_persist() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("papers.json");
    let mut s = session_with_doc();
    s.set_paper_file(Some(file.clone()));

    let r = ok(
        &mut s,
        "paper.add",
        json!({"name": "Jar lid", "width_mm": 38, "height_mm": 38,
               "corner_radius_mm": 19, "barcode": "111"}),
    );
    assert_eq!(r["id"], "my:jar-lid");
    assert_eq!(r["custom"], true);
    assert_eq!(r["barcodes"], json!(["111"]));
    let r = ok(
        &mut s,
        "paper.add",
        json!({"name": "Jar lid", "width_mm": 38, "height_mm": 38}),
    );
    assert_eq!(r["id"], "my:jar-lid-2");
    let e = err(
        &mut s,
        "paper.add",
        json!({"name": "", "width_mm": 38, "height_mm": 38}),
    );
    assert_eq!(e.param.as_deref(), Some("name"));
    let e = err(
        &mut s,
        "paper.add",
        json!({"name": "x", "width_mm": -1, "height_mm": 38}),
    );
    assert_eq!(e.param.as_deref(), Some("width_mm"));

    let r = ok(
        &mut s,
        "paper.link",
        json!({"barcode": "222", "id": "std:50x30"}),
    );
    assert!(r.get("previous").is_none());
    let r = ok(
        &mut s,
        "paper.link",
        json!({"barcode": "222", "id": "std:40x30"}),
    );
    assert_eq!(r["previous"], "std:50x30");
    let e = err(
        &mut s,
        "paper.link",
        json!({"barcode": "333", "id": "std:nope"}),
    );
    assert_eq!(e.code, ErrorCode::NotFound);

    // A new session reading the same file sees everything.
    let mut s2 = session_with_doc();
    s2.set_paper_file(Some(file.clone()));
    let mine = ok(&mut s2, "paper.list", json!({"custom": true}));
    assert_eq!(mine["papers"].as_array().unwrap().len(), 2);
    let by_code = ok(&mut s2, "paper.list", json!({"query": "222"}));
    assert_eq!(by_code["papers"][0]["id"], "std:40x30");

    let e = err(&mut s2, "paper.delete", json!({"id": "std:40x30"}));
    assert_eq!(e.param.as_deref(), Some("id"));
    let r = ok(&mut s2, "paper.delete", json!({"id": "my:jar-lid"}));
    assert_eq!(r["unlinked"], json!(["111"]));
    let r = ok(&mut s2, "paper.unlink", json!({"barcode": "222"}));
    assert_eq!(r["previous"], "std:40x30");
    let e = err(&mut s2, "paper.unlink", json!({"barcode": "222"}));
    assert_eq!(e.code, ErrorCode::NotFound);
}

#[test]
fn detect_and_check_use_linked_paper() {
    let dir = tempfile::tempdir().unwrap();
    let mut kit = test_kit();
    kit.session
        .set_paper_file(Some(dir.path().join("papers.json")));
    let s = &mut kit.session;
    ok(
        s,
        "doc.new",
        json!({"width_mm": 40, "height_mm": 30, "printer_model": "test.fake"}),
    );
    ok(s, "device.connect", json!({"address": FAKE_ADDRESS}));

    let r = ok(s, "paper.detect", json!({}));
    assert_eq!(r["rfid"], false);

    kit.devices.set_paper(rfid("6900001", MediaType::Gap));
    let s = &mut kit.session;
    let r = ok(s, "paper.detect", json!({}));
    assert_eq!(r["rfid"], true);
    assert_eq!(r["barcode"], "6900001");
    assert_eq!(r["remaining"], 180);
    assert!(r.get("paper").is_none(), "not linked yet");
    assert_eq!(r["matches_label"], true, "media matches");

    ok(
        s,
        "paper.link",
        json!({"barcode": "6900001", "id": "std:50x30"}),
    );
    let r = ok(s, "paper.detect", json!({}));
    assert_eq!(r["paper"]["id"], "std:50x30");
    assert_eq!(r["matches_label"], false, "40 mm label, 50 mm paper");

    let c = ok(s, "doc.check", json!({}));
    let issue = c["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["rule"] == "paper_mismatch")
        .expect("size mismatch reported");
    assert!(issue["hint"].as_str().unwrap().contains("std:50x30"));

    ok(s, "paper.apply", json!({"id": "std:50x30"}));
    let r = ok(s, "paper.detect", json!({}));
    assert_eq!(r["matches_label"], true);
    let c = ok(s, "doc.check", json!({}));
    assert!(
        c["issues"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["rule"] != "paper_mismatch")
    );
}

#[test]
fn calibrate_and_settings() {
    let mut kit = test_kit();
    let s = &mut kit.session;
    ok(
        s,
        "doc.new",
        json!({"width_mm": 40, "height_mm": 30, "media": "black_mark"}),
    );
    let e = err(s, "device.calibrate", json!({}));
    assert_eq!(e.code, ErrorCode::DeviceUnavailable);
    ok(s, "device.connect", json!({"address": FAKE_ADDRESS}));
    let r = ok(s, "device.calibrate", json!({}));
    assert_eq!(r["media"], "black_mark", "defaults to the label media");
    ok(s, "device.calibrate", json!({"media": "gap"}));
    let e = err(s, "device.calibrate", json!({"media": "continuous"}));
    assert_eq!(e.param.as_deref(), Some("media"));
    assert_eq!(
        kit.devices.calibrations(),
        vec![MediaType::BlackMark, MediaType::Gap]
    );

    let s = &mut kit.session;
    let r = ok(
        s,
        "device.set_settings",
        json!({"auto_off_level": 3, "sound_power": false}),
    );
    assert_eq!(r["settings"]["auto_off_level"], 3);
    assert_eq!(r["settings"]["sound_power"], false);
    let r = ok(s, "device.settings", json!({}));
    assert_eq!(r["settings"]["auto_off_level"], 3);
    let e = err(s, "device.set_settings", json!({"auto_off_level": 5}));
    assert_eq!(e.param.as_deref(), Some("auto_off_level"));
    let e = err(s, "device.set_settings", json!({}));
    assert_eq!(e.code, ErrorCode::BadParams);
}

#[test]
fn calibrate_needs_print_permission() {
    let mut kit = test_kit();
    let s = &mut kit.session;
    ok(s, "device.connect", json!({"address": FAKE_ADDRESS}));
    s.set_policy(labely_core::Policy::restricted(vec![std::env::temp_dir()]));
    let e = err(s, "device.calibrate", json!({}));
    assert_eq!(e.code, ErrorCode::Forbidden);
    ok(s, "device.settings", json!({}));
}
