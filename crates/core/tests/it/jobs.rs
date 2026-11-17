//! Print queue and print history (ADR-0019).

use labely_core::ErrorCode;
use labely_core::services::DeviceError;
use labely_testkit::{FAKE_ADDRESS, TestKit, test_kit};
use serde_json::{Value, json};

use crate::{err, ok};

fn kit_with_label() -> TestKit {
    let mut kit = test_kit();
    let s = &mut kit.session;
    ok(
        s,
        "doc.new",
        json!({"name": "Shelf", "width_mm": 40, "height_mm": 20, "printer_model": "test.fake"}),
    );
    ok(
        s,
        "element.add_rect",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 5, "height_mm": 5}),
    );
    ok(s, "device.connect", json!({"address": FAKE_ADDRESS}));
    kit
}

fn state(v: &Value) -> &str {
    v["state"].as_str().unwrap()
}

#[test]
fn waited_print_reports_job() {
    let mut kit = kit_with_label();
    let r = ok(&mut kit.session, "print.submit", json!({"copies": 2}));
    assert_eq!(state(&r), "done");
    assert_eq!(r["job_id"], "job-1");
    assert_eq!(r["pages_printed"], 2);
    let jobs = ok(&mut kit.session, "job.list", json!({}));
    assert_eq!(jobs["paused"], false);
    assert_eq!(jobs["jobs"][0]["doc_name"], "Shelf");
    assert_eq!(jobs["jobs"][0]["pages_total"], 2);
    assert!(jobs["jobs"][0]["finished_at"].is_string());

    kit.devices.fail_next_print(DeviceError::Printer {
        message: "lid open".into(),
        hint: "close the lid".into(),
    });
    let e = err(&mut kit.session, "print.submit", json!({}));
    assert_eq!(e.code, ErrorCode::DeviceError);
    assert_eq!(e.hint.as_deref(), Some("close the lid"));
    let j = ok(&mut kit.session, "job.get", json!({"id": "job-2"}));
    assert_eq!(state(&j), "failed");
    assert_eq!(j["error"]["code"], "device_error");
}

#[test]
fn queue_pause_resume_and_cancel() {
    let mut kit = kit_with_label();
    kit.devices.hold_prints(true);
    let s = &mut kit.session;
    let a = ok(s, "print.submit", json!({"wait": false, "copies": 3}));
    assert_eq!(a["job_id"], "job-1");
    assert_eq!(a["pages_printed"], 0);
    assert!(kit.devices.wait_held(), "job-1 reached the printer");
    let j = ok(s, "job.get", json!({"id": "job-1"}));
    assert_eq!(state(&j), "printing");
    assert_eq!(j["pages_done"], 1, "live progress");

    assert_eq!(ok(s, "job.pause", json!({}))["paused"], true);
    let e = err(s, "print.submit", json!({}));
    assert_eq!(
        e.param.as_deref(),
        Some("wait"),
        "a waited print on a paused queue"
    );
    ok(s, "print.submit", json!({"wait": false}));
    ok(s, "print.submit", json!({"wait": false}));

    // Cancel a queued job, then the running one.
    let c = ok(s, "job.cancel", json!({"id": "job-2"}));
    assert_eq!(state(&c), "cancelled");
    assert_eq!(
        err(s, "job.cancel", json!({"id": "job-2"})).code,
        ErrorCode::BadParams
    );
    ok(s, "job.cancel", json!({"id": "job-1"}));
    let j = ok(s, "job.wait", json!({"id": "job-1", "timeout_ms": 5000}));
    assert_eq!(state(&j), "cancelled");
    assert_eq!(j["error"]["code"], "cancelled");

    // Paused: job-3 does not start.
    let j = ok(s, "job.wait", json!({"id": "job-3", "timeout_ms": 50}));
    assert_eq!(state(&j), "queued");
    kit.devices.hold_prints(false);
    ok(s, "job.resume", json!({}));
    let j = ok(s, "job.wait", json!({"id": "job-3", "timeout_ms": 5000}));
    assert_eq!(state(&j), "done");
    assert_eq!(kit.devices.prints().len(), 1, "only job-3 printed");

    assert_eq!(ok(s, "job.clear", json!({}))["removed"], 3);
    assert!(
        ok(s, "job.list", json!({}))["jobs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        err(s, "job.get", json!({"id": "job-9"})).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        err(s, "job.wait", json!({"id": "job-3", "timeout_ms": 700_000}))
            .param
            .as_deref(),
        Some("timeout_ms")
    );
}

#[test]
fn history_records_and_reopens() {
    let mut kit = kit_with_label();
    let s = &mut kit.session;
    assert_eq!(err(s, "job.history", json!({})).code, ErrorCode::Disabled);
    let dir = tempfile::tempdir().unwrap();
    s.set_history_dir(Some(dir.path().join("history")));

    ok(s, "print.submit", json!({"copies": 2}));
    ok(s, "doc.set_label", json!({"name": "Second"}));
    ok(s, "print.submit", json!({}));
    // A job cancelled before it started never reached a printer: no entry.
    ok(s, "job.pause", json!({}));
    ok(s, "print.submit", json!({"wait": false}));
    ok(s, "job.cancel", json!({"id": "job-3"}));
    ok(s, "job.resume", json!({}));

    let h = ok(s, "job.history", json!({}));
    assert_eq!(h["total"], 2);
    let entries = h["entries"].as_array().unwrap();
    assert_eq!(entries[0]["doc_name"], "Second", "newest first");
    assert_eq!(entries[1]["pages_printed"], 2);
    assert_eq!(entries[1]["state"], "done");
    assert_eq!(entries[1]["model"], "test.fake");
    let first = entries[1]["id"].as_str().unwrap().to_owned();
    let second = entries[0]["id"].as_str().unwrap().to_owned();
    assert_ne!(first, second);
    assert_eq!(
        ok(s, "job.history", json!({"limit": 1, "offset": 1}))["entries"][0]["id"],
        first
    );

    let o = ok(s, "job.history_open", json!({"id": first}));
    assert_eq!(o["name"], "Shelf");
    assert_eq!(s.doc(None).unwrap().name, "Shelf");
    assert_eq!(
        err(s, "job.history_open", json!({"id": "nope"})).code,
        ErrorCode::NotFound
    );

    assert_eq!(
        err(s, "job.history_delete", json!({"ids": ["nope"]})).code,
        ErrorCode::NotFound
    );
    let d = ok(s, "job.history_delete", json!({"ids": [first]}));
    assert_eq!(d["deleted"].as_array().unwrap().len(), 1);
    assert!(
        !dir.path()
            .join("history")
            .join(format!("{first}.labely"))
            .exists()
    );
    assert_eq!(ok(s, "job.history", json!({}))["total"], 1);
    ok(s, "job.history_clear", json!({}));
    assert_eq!(ok(s, "job.history", json!({}))["total"], 0);
}

#[test]
fn observer_sees_every_change_in_order() {
    use std::sync::{Arc, Mutex};

    use labely_core::jobs::{JobInfo, JobState};

    let mut kit = kit_with_label();
    let seen: Arc<Mutex<Vec<JobInfo>>> = Arc::default();
    let sink = Arc::clone(&seen);
    kit.session
        .jobs()
        .set_observer(Some(Arc::new(move |j: &JobInfo| {
            sink.lock().unwrap().push(j.clone());
        })));
    ok(&mut kit.session, "print.submit", json!({"copies": 2}));

    let seen = seen.lock().unwrap().clone();
    let mut states: Vec<JobState> = seen.iter().map(|j| j.state).collect();
    states.dedup();
    assert_eq!(
        states,
        [JobState::Queued, JobState::Printing, JobState::Done],
        "{seen:?}"
    );
    assert!(seen.iter().all(|j| j.id == "job-1"));
    assert!(
        seen.iter()
            .any(|j| j.state == JobState::Printing && j.pages_done == 2),
        "{seen:?}"
    );
    assert_eq!(seen.last().map(|j| j.pages_done), Some(2));

    kit.session.jobs().set_observer(None);
    ok(&mut kit.session, "print.submit", json!({}));
}
