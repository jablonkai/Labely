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

#[test]
fn print_offset_mirror_and_rotate() {
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
    ok(s, "device.connect", json!({"address": FAKE_ADDRESS}));
    let page = |s: &mut labely_core::Session, p| {
        ok(s, "print.submit", p);
        kit.devices.prints().last().unwrap().1.pages[0].clone()
    };
    let plain = page(s, json!({}));
    assert!(plain.count_black() > 0);
    // 203 dpi: 2 mm = 16 dots, −1 mm = −8 dots.
    let moved = page(s, json!({"offset_x_mm": 2, "offset_y_mm": -1}));
    assert_eq!(moved, plain.shifted(16, -8));
    assert_eq!(page(s, json!({"mirror": true})), plain.mirror_h());
    assert_eq!(page(s, json!({"rotate_180": true})), plain.rotate_180());
    assert_eq!(
        page(
            s,
            json!({"mirror": true, "rotate_180": true, "offset_x_mm": 2})
        ),
        plain.shifted(16, 0).mirror_h().rotate_180()
    );
    let e = err(s, "print.submit", json!({"offset_x_mm": 11}));
    assert_eq!(e.param.as_deref(), Some("offset_x_mm"));
    let e = err(s, "print.submit", json!({"offset_y_mm": -10.5}));
    assert_eq!(e.param.as_deref(), Some("offset_y_mm"));
}
