use labely_core::ErrorCode;
use labely_core::services::DeviceError;
use labely_testkit::{FAKE_ADDRESS, test_kit};
use serde_json::json;

use crate::{err, ok};

#[test]
fn scan_connect_status_print() {
    let mut kit = test_kit();
    let s = &mut kit.session;
    ok(
        s,
        "doc.new",
        json!({"width_mm": 40, "height_mm": 20, "printer_model": "test.fake"}),
    );
    ok(
        s,
        "element.add_rect",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 5, "height_mm": 5}),
    );

    let e = err(s, "print.submit", json!({}));
    assert_eq!(e.code, ErrorCode::DeviceUnavailable);
    assert!(e.hint.unwrap().contains("device.scan"));

    let r = ok(s, "device.scan", json!({"timeout_ms": 10}));
    assert_eq!(r["devices"][0]["address"], FAKE_ADDRESS);
    let e = err(s, "device.connect", json!({"address": "ble:nope"}));
    assert_eq!(e.code, ErrorCode::DeviceUnavailable);
    let r = ok(s, "device.connect", json!({"address": FAKE_ADDRESS}));
    assert_eq!(r["model"]["id"], "test.fake");
    assert_eq!(
        ok(s, "device.list", json!({}))["devices"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let r = ok(s, "device.status", json!({}));
    assert_eq!(r["status"]["battery_percent"], 100);

    let journal = s.journal().len();
    let r = ok(s, "print.submit", json!({"copies": 2}));
    assert_eq!(r["pages_printed"], 2);
    assert_eq!(r["density"], 3, "model default");
    assert_eq!(r["dpi"], 203);
    assert_eq!(
        s.journal().len(),
        journal,
        "device commands are not journaled"
    );
    let e = err(s, "print.submit", json!({"density": 9}));
    assert_eq!(e.param.as_deref(), Some("density"));

    kit.devices.fail_next_print(DeviceError::Printer {
        message: "lid open".into(),
        hint: "close the lid".into(),
    });
    let e = err(&mut kit.session, "print.submit", json!({}));
    assert_eq!(e.code, ErrorCode::DeviceError);
    assert_eq!(e.hint.as_deref(), Some("close the lid"));

    let prints = kit.devices.prints();
    assert_eq!(prints.len(), 1);
    let job = &prints[0].1;
    assert_eq!(job.copies, 2);
    assert_eq!((job.pages[0].width(), job.pages[0].height()), (320, 160));
    assert!(job.pages[0].count_black() > 0);

    ok(&mut kit.session, "device.disconnect", json!({}));
}
