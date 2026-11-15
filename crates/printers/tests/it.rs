//! Driver ↔ simulator integration tests (no hardware).

use labely_core::Bitmap1;
use labely_core::document::MediaType;
use labely_core::services::{DeviceError, DeviceHub, JobControl, PrintRequest};
use labely_printers::Hub;
use labely_printers::driver::{Driver, prepare_for_head};
use labely_printers::niimbot::NiimbotDriver;
use labely_printers::registry::Registry;
use labely_printers::sim::{SimConfig, Simulator};

fn test_label() -> Bitmap1 {
    // 50 x 30 mm at 300 dpi, with a border and a diagonal.
    let (w, h) = (591, 354);
    let mut b = Bitmap1::new(w, h);
    for x in 0..w {
        b.set(x, 0, true);
        b.set(x, h - 1, true);
    }
    for y in 0..h {
        b.set(10, y, true);
        b.set(y + 100, y, true);
    }
    b
}

#[test]
fn b21_pro_job_roundtrips_through_simulator() {
    let (sim, transport) = Simulator::start(SimConfig::b21_pro());
    let mut drv = NiimbotDriver::new(Box::new(transport));
    let id = drv.connect().unwrap();
    assert_eq!(id.model.as_ref().unwrap().id, "niimbot.b21_pro");
    assert_eq!(id.protocol, Some(4));

    let st = drv.status().unwrap();
    assert_eq!(st.battery_percent, Some(100));
    assert_eq!(st.paper.as_ref().unwrap().barcode, "6972842743589");
    assert_eq!(st.paper.as_ref().unwrap().media, Some(MediaType::Gap));

    let label = test_label();
    let job = PrintRequest {
        pages: vec![label.clone()],
        copies: 2,
        density: 4,
        media: MediaType::Gap,
    };
    let control = JobControl::new();
    let report = drv.print(&job, &control).unwrap();
    assert_eq!(report.pages_printed, 2);

    let log = sim.log();
    assert_eq!(log.density, Some(4));
    assert_eq!(log.label_type, Some(1));
    assert_eq!(log.print_start_len, Some(9));
    assert_eq!(log.page_size_len, Some(13));
    assert_eq!(log.pages.len(), 1);
    let model = Registry::builtin().by_id("niimbot.b21_pro").unwrap();
    let expected = prepare_for_head(&label, model);
    assert_eq!(
        log.pages[0].0, expected,
        "simulator reassembled the exact raster"
    );
    assert_eq!(log.pages[0].1, 2);
    assert_eq!(control.progress(), (2, 2));
}

#[test]
fn cancelled_job_sends_no_page() {
    let (sim, transport) = Simulator::start(SimConfig::b21_pro());
    let mut drv = NiimbotDriver::new(Box::new(transport));
    drv.connect().unwrap();
    let job = PrintRequest {
        pages: vec![test_label()],
        copies: 1,
        density: 3,
        media: MediaType::Gap,
    };
    let control = JobControl::new();
    control.cancel();
    let err = drv.print(&job, &control).unwrap_err();
    assert_eq!(err, DeviceError::Cancelled);
    assert!(sim.log().pages.is_empty());
}

#[test]
fn hub_fake_printer_options() {
    let dir = tempfile::tempdir().unwrap();
    let hub = Hub::new();
    let bad = format!("fake:{}?colour=red", dir.path().display());
    assert!(
        hub.connect(&bad)
            .unwrap_err()
            .to_string()
            .contains("unknown option")
    );
    let addr = format!(
        "fake:{}?model=niimbot.b21_pro&delay_ms=1",
        dir.path().display()
    );
    assert_eq!(
        hub.connect(&addr).unwrap().model.unwrap().id,
        "niimbot.b21_pro"
    );
}

#[test]
fn printer_error_is_reported() {
    let mut cfg = SimConfig::b21_pro();
    cfg.fail_page_end = Some(1);
    let (_sim, transport) = Simulator::start(cfg);
    let mut drv = NiimbotDriver::new(Box::new(transport));
    drv.connect().unwrap();
    let job = PrintRequest {
        pages: vec![Bitmap1::new(64, 8)],
        copies: 1,
        density: 3,
        media: MediaType::Gap,
    };
    let err = drv.print(&job, &JobControl::new()).unwrap_err();
    assert!(err.to_string().contains("CoverOpen"), "{err}");
}

#[test]
fn hub_fake_printer_writes_png() {
    let dir = tempfile::tempdir().unwrap();
    let hub = Hub::new();
    let addr = format!("fake:{}", dir.path().display());
    let info = hub.connect(&addr).unwrap();
    assert_eq!(info.model.unwrap().dpi, 300);
    let job = PrintRequest {
        pages: vec![test_label()],
        copies: 1,
        density: 3,
        media: MediaType::Gap,
    };
    hub.print(&addr, job, &JobControl::new()).unwrap();
    let png = dir.path().join("job-0001/page-01.png");
    assert!(png.exists());
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("job-0001/job.json")).unwrap())
            .unwrap();
    assert_eq!(meta["pages"][0]["width_dots"], 584);
}

#[test]
fn hub_sim_and_errors() {
    let hub = Hub::new();
    let info = hub.connect("sim:niimbot.b21_pro").unwrap();
    assert!(info.connected);
    // Read at connect, as a B21 Pro reports them.
    assert_eq!(info.firmware.as_deref(), Some("3.12"));
    assert_eq!(info.hardware.as_deref(), Some("3.01"));
    assert_eq!(info.serial.as_deref(), Some("SIM0000001"));
    assert_eq!(hub.connected().len(), 1);
    assert_eq!(hub.connected()[0].firmware.as_deref(), Some("3.12"));
    assert!(hub.status("sim:niimbot.b21_pro").unwrap().paper.is_some());
    hub.disconnect("sim:niimbot.b21_pro").unwrap();
    assert!(hub.connect("nope:x").is_err());
    assert!(hub.connect("fake:/tmp/x?model=does.not.exist").is_err());
}

#[test]
fn hub_stays_responsive_while_printing() {
    let dir = tempfile::tempdir().unwrap();
    let hub = std::sync::Arc::new(Hub::new());
    let addr = format!("fake:{}?delay_ms=300", dir.path().display());
    hub.connect(&addr).unwrap();
    let control = std::sync::Arc::new(JobControl::new());
    let worker = {
        let (hub, addr, control) = (hub.clone(), addr.clone(), control.clone());
        std::thread::spawn(move || {
            let job = PrintRequest {
                pages: vec![test_label(), test_label()],
                copies: 1,
                density: 3,
                media: MediaType::Gap,
            };
            hub.print(&addr, job, &control)
        })
    };
    while control.progress().1 == 0 {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // Listing never waits; status and disconnect report busy instead of blocking.
    assert_eq!(hub.connected().len(), 1);
    let busy = hub.status(&addr).unwrap_err();
    assert!(busy.to_string().contains("busy printing"), "{busy}");
    assert!(hub.disconnect(&addr).is_err());
    control.cancel();
    assert_eq!(worker.join().unwrap().unwrap_err(), DeviceError::Cancelled);
    assert!(hub.status(&addr).is_ok());
    hub.disconnect(&addr).unwrap();
}

#[test]
fn settings_and_calibration_through_simulator() {
    use labely_core::services::DeviceSettings;
    let (sim, transport) = Simulator::start(SimConfig::b21_pro());
    let mut drv = NiimbotDriver::new(Box::new(transport));
    drv.connect().unwrap();

    let s = drv.settings().unwrap();
    assert_eq!(s.auto_off_level, Some(2));
    assert_eq!(s.sound_power, Some(true));
    assert_eq!(s.sound_bluetooth, Some(true));

    drv.set_settings(&DeviceSettings {
        auto_off_level: Some(4),
        sound_power: Some(false),
        sound_bluetooth: None,
    })
    .unwrap();
    let s = drv.settings().unwrap();
    assert_eq!(s.auto_off_level, Some(4));
    assert_eq!(s.sound_power, Some(false));
    assert_eq!(s.sound_bluetooth, Some(true));

    let bad = drv.set_settings(&DeviceSettings {
        auto_off_level: Some(9),
        ..DeviceSettings::default()
    });
    assert!(bad.is_err());

    drv.calibrate(MediaType::BlackMark).unwrap();
    assert_eq!(sim.log().calibrations, vec![2]);
}

#[test]
fn hub_settings_on_fake_printer() {
    use labely_core::services::DeviceSettings;
    let dir = tempfile::tempdir().unwrap();
    let hub = Hub::new();
    let address = format!("fake:{}", dir.path().display());
    hub.connect(&address).unwrap();
    let s = hub
        .set_settings(
            &address,
            &DeviceSettings {
                auto_off_level: Some(3),
                ..DeviceSettings::default()
            },
        )
        .unwrap();
    assert_eq!(s.auto_off_level, Some(3));
    hub.calibrate(&address, MediaType::Gap).unwrap();
    assert!(dir.path().join("calibrate-0001.json").exists());
}

#[test]
fn font_add_loads_a_font_file_into_the_renderer() {
    let mut s = labely_core::Session::new(labely_printers::default_services());
    let dir = tempfile::tempdir().unwrap();
    let font = dir.path().join("bold.ttf");
    let src =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../render/fonts/NotoSans-Bold.ttf");
    std::fs::copy(src, &font).unwrap();
    let e = s.execute("font.add", serde_json::json!({ "path": font }));
    assert!(e.ok, "{:?}", e.error);
    let r = e.result.unwrap();
    assert_eq!(r["added"], serde_json::json!(["Noto Sans"]));
    assert!(
        r["fonts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "Noto Sans")
    );

    let junk = dir.path().join("junk.ttf");
    std::fs::write(&junk, b"hello").unwrap();
    let e = s.execute("font.add", serde_json::json!({ "path": junk }));
    let err = e.error.unwrap();
    assert_eq!(err.param.as_deref(), Some("path"));
    assert!(err.message.contains("not a TrueType"), "{}", err.message);

    let missing = dir.path().join("missing.otf");
    let e = s.execute("font.add", serde_json::json!({ "path": missing }));
    assert_eq!(e.error.unwrap().code, labely_core::ErrorCode::NotFound);
}
