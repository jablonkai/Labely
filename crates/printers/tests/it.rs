//! Driver ↔ simulator integration tests (no hardware).

use labely_core::Bitmap1;
use labely_core::document::MediaType;
use labely_core::services::{DeviceHub, PrintRequest};
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
    let mut events = Vec::new();
    let report = drv.print(&job, &mut |p| events.push(p)).unwrap();
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
    assert!(!events.is_empty());
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
    let err = drv.print(&job, &mut |_| {}).unwrap_err();
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
    hub.print(&addr, job).unwrap();
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
    assert_eq!(hub.connected().len(), 1);
    assert!(hub.status("sim:niimbot.b21_pro").unwrap().paper.is_some());
    hub.disconnect("sim:niimbot.b21_pro").unwrap();
    assert!(hub.connect("nope:x").is_err());
    assert!(hub.connect("fake:/tmp/x?model=does.not.exist").is_err());
}
