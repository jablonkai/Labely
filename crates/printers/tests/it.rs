//! Driver ↔ simulator integration tests (no hardware).

use labely_core::document::MediaType;
use labely_core::services::{DeviceError, DeviceHub, JobControl, PagePlanes, PrintRequest};
use labely_core::{Bitmap1, Gray4};
use labely_printers::Hub;
use labely_printers::driver::{Driver, prepare_for_head, prepare_gray_for_head};
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
        planes: PagePlanes::None,
        half_cut: None,
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
        planes: PagePlanes::None,
        half_cut: None,
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
        planes: PagePlanes::None,
        half_cut: None,
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
        planes: PagePlanes::None,
        half_cut: None,
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
                planes: PagePlanes::None,
                half_cut: None,
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

#[test]
fn native_ble_host_prints_through_simulator() {
    use labely_printers::sim::SimBleHost;
    let host = std::sync::Arc::new(SimBleHost::b21_pro());
    let hub = Hub::new().with_native_ble(host.clone());
    let found = hub.scan(100).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].address, "ble:SIM-B21");
    assert_eq!(
        found[0].model.as_ref().map(|m| m.id.as_str()),
        Some("niimbot.b21_pro")
    );

    let info = hub.connect("ble:SIM-B21").unwrap();
    assert!(info.connected);
    assert_eq!(info.serial.as_deref(), Some("SIM0000001"));
    assert!(hub.scan(100).unwrap()[0].connected);

    // Small page: the BLE packet gap (10 ms) applies per row.
    let mut page = Bitmap1::new(96, 24);
    for x in 0..96 {
        page.set(x, x % 24, true);
    }
    let job = PrintRequest {
        pages: vec![page],
        copies: 2,
        density: 3,
        media: MediaType::Gap,
        planes: PagePlanes::None,
        half_cut: None,
    };
    let report = hub.print("ble:SIM-B21", job, &JobControl::new()).unwrap();
    assert_eq!(report.pages_printed, 2); // copies count
    let log = host.log().unwrap();
    assert_eq!(log.pages.len(), 1);
    assert_eq!(log.pages[0].1, 2);
    hub.disconnect("ble:SIM-B21").unwrap();
    assert!(hub.connect("ble:OTHER").is_err());
}

/// Every NIIMBOT model in the registry prints on a simulator set up for it: the task
/// resolves, the job completes and the pages arrive as prepared for the head.
#[test]
fn every_registered_model_prints_on_the_simulator() {
    let registry = Registry::builtin();
    let models: Vec<_> = registry
        .models()
        .iter()
        .filter(|m| m.driver == "niimbot")
        .collect();
    assert!(models.len() > 40);
    for model in models {
        assert!(!model.model_codes.is_empty(), "{} has no code", model.id);
        let (sim, transport) = Simulator::start(SimConfig::for_model(model));
        // The model guessed from the BLE name decides between models that share a code.
        let by_name = registry.guess_by_name(&model.name).cloned();
        let mut drv = NiimbotDriver::new(Box::new(transport)).with_fallback_model(by_name);
        let id = drv.connect().unwrap();
        let resolved = registry.by_id(&id.model.unwrap().id).unwrap();
        assert_eq!(resolved.task, model.task, "{}", model.id);
        assert_eq!(resolved.head_dots, model.head_dots, "{}", model.id);

        // Taller than one check-line interval, so the B21 tasks send a check-line.
        let (w, h) = (model.head_dots.min(96), 230);
        let mut label = Bitmap1::new(w, h);
        for y in 0..h {
            label.set(y % w, y, true);
        }
        let job = PrintRequest {
            pages: vec![label.clone()],
            copies: 2,
            density: model.density.default,
            media: MediaType::Gap,
            planes: PagePlanes::None,
            half_cut: None,
        };
        let control = JobControl::new();
        let report = drv
            .print(&job, &control)
            .unwrap_or_else(|e| panic!("{} ({}): {e}", model.id, model.task));
        assert_eq!(report.pages_printed, 2, "{}", model.id);
        assert_eq!(control.progress(), (2, 2), "{}", model.id);

        let log = sim.log();
        let expected = prepare_for_head(&label, resolved);
        let copies: u32 = log.pages.iter().map(|p| u32::from(p.1)).sum();
        assert_eq!(copies, 2, "{}", model.id);
        for (page, _) in &log.pages {
            assert_eq!(page, &expected, "{} ({})", model.id, model.task);
        }
        let per_copy = matches!(model.task.as_str(), "b21_v1" | "b21_l2b");
        assert_eq!(
            log.pages.len(),
            if per_copy { 2 } else { 1 },
            "{}",
            model.id
        );
        assert_eq!(
            log.commands.contains(&0x86),
            per_copy,
            "{}: check-lines",
            model.id
        );
    }
}

#[test]
fn ble_name_decides_between_models_that_share_a_code() {
    let registry = Registry::builtin();
    let d11 = registry.by_id("niimbot.d11").unwrap();
    let hi = registry.by_id("niimbot.hi_nb_d11").unwrap();
    assert_eq!(d11.model_codes, hi.model_codes);
    for (name, expected) in [("D11-A1B2C3", d11), ("Hi-NB-D11-0001", hi)] {
        let (_sim, transport) = Simulator::start(SimConfig::for_model(expected));
        let mut drv = NiimbotDriver::new(Box::new(transport))
            .with_fallback_model(registry.guess_by_name(name).cloned());
        assert_eq!(drv.connect().unwrap().model.unwrap().id, expected.id);
    }
    // Without a name the first model registered for the code is used.
    let (_sim, transport) = Simulator::start(SimConfig::for_model(hi));
    let mut drv = NiimbotDriver::new(Box::new(transport));
    assert_eq!(drv.connect().unwrap().model.unwrap().id, d11.id);
}

/// A 96 x 40 label with a black frame row and a red bar.
fn two_color_label() -> (Bitmap1, Bitmap1) {
    let (w, h) = (96, 40);
    let (mut black, mut red) = (Bitmap1::new(w, h), Bitmap1::new(w, h));
    for x in 0..w {
        black.set(x, 2, true);
    }
    for y in 10..20 {
        for x in 8..40 {
            red.set(x, y, true);
        }
        // Rows with both colours.
        black.set(60, y, true);
    }
    (black, red)
}

fn connect_sim(config: SimConfig) -> (Simulator, NiimbotDriver) {
    let (sim, transport) = Simulator::start(config);
    let mut drv = NiimbotDriver::new(Box::new(transport));
    assert!(drv.connect().is_ok(), "the simulator answers the handshake");
    (sim, drv)
}

#[test]
fn two_colour_job_reaches_a_colour_printer_as_two_planes() {
    let (sim, mut drv) = connect_sim(SimConfig {
        color: true,
        ..SimConfig::b21_pro()
    });
    let (black, red) = two_color_label();
    let job = PrintRequest {
        pages: vec![black.clone()],
        copies: 1,
        density: 3,
        media: MediaType::Gap,
        planes: PagePlanes::Red(vec![red.clone()]),
        half_cut: None,
    };
    let report = drv.print(&job, &JobControl::new()).unwrap();
    assert_eq!(report.pages_printed, 1);

    let log = sim.log();
    let model = Registry::builtin().by_id("niimbot.b21_pro").unwrap();
    assert_eq!(log.color_mode, Some(1));
    assert!(log.commands.contains(&0x8A));
    assert!(!log.commands.iter().any(|c| matches!(c, 0x84 | 0x85)));
    assert_eq!(log.pages[0].0, prepare_for_head(&black, model));
    assert_eq!(log.red_pages, vec![prepare_for_head(&red, model)]);
}

#[test]
fn red_prints_black_on_a_printer_without_a_second_colour() {
    let (sim, mut drv) = connect_sim(SimConfig::b21_pro());
    let (black, red) = two_color_label();
    let job = PrintRequest {
        pages: vec![black.clone()],
        copies: 1,
        density: 3,
        media: MediaType::Gap,
        planes: PagePlanes::Red(vec![red.clone()]),
        half_cut: None,
    };
    drv.print(&job, &JobControl::new()).unwrap();

    let log = sim.log();
    assert_eq!(log.color_mode, Some(0));
    assert!(!log.commands.contains(&0x8A));
    assert!(log.red_pages.is_empty());
    let page = &log.pages[0].0;
    assert_eq!(page.count_black(), black.count_black() + red.count_black());
    assert!(page.get(8, 10) && page.get(60, 10) && page.get(0, 2));

    // A plane count that does not match the pages is refused.
    let bad = PrintRequest {
        planes: PagePlanes::Red(Vec::new()),
        ..job
    };
    assert!(matches!(
        drv.print(&bad, &JobControl::new()),
        Err(DeviceError::Other(_))
    ));
}

#[test]
fn gray_job_reaches_the_printer_with_its_levels() {
    let (sim, mut drv) = connect_sim(SimConfig::b21_pro());
    let (w, h) = (96u32, 300u32);
    let mut gray = Gray4::new(w, h);
    for y in 20..h {
        for x in 0..w {
            // A ramp, flat areas (runs) and noise-like columns.
            let level = match x {
                0..=31 => (x / 2) as u8,
                32..=79 => 9,
                _ => ((x + y) % 16) as u8,
            };
            gray.set(x, y, level);
        }
    }
    let job = PrintRequest {
        pages: vec![Bitmap1::new(w, h)],
        copies: 2,
        density: 3,
        media: MediaType::Gap,
        planes: PagePlanes::Gray(vec![gray.clone()]),
        half_cut: None,
    };
    let report = drv.print(&job, &JobControl::new()).unwrap();
    assert_eq!(report.pages_printed, 2);

    let log = sim.log();
    let model = Registry::builtin().by_id("niimbot.b21_pro").unwrap();
    assert_eq!(log.color_mode, Some(3));
    assert!(log.commands.contains(&0x8D));
    assert_eq!(log.gray_pages, vec![prepare_gray_for_head(&gray, model)]);
    assert_eq!(log.pages[0].1, 2);
}

#[test]
fn gray_falls_back_to_the_bitmap_when_the_task_has_no_gray_rows() {
    let registry = Registry::builtin();
    let model = registry.by_id("niimbot.b1").unwrap();
    let (sim, mut drv) = connect_sim(SimConfig::for_model(model));
    let mut page = Bitmap1::new(96, 10);
    page.set(5, 5, true);
    let job = PrintRequest {
        pages: vec![page.clone()],
        copies: 1,
        density: 3,
        media: MediaType::Gap,
        planes: PagePlanes::Gray(vec![Gray4::new(96, 10)]),
        half_cut: None,
    };
    drv.print(&job, &JobControl::new()).unwrap();
    let log = sim.log();
    assert!(log.gray_pages.is_empty());
    assert_eq!(log.pages[0].0, prepare_for_head(&page, model));
}

#[test]
fn half_cut_is_set_before_the_job() {
    let registry = Registry::builtin();
    let model = registry.by_id("niimbot.h1s").unwrap();
    let (sim, mut drv) = connect_sim(SimConfig::for_model(model));
    let mut page = Bitmap1::new(96, 10);
    page.set(5, 5, true);
    let job = PrintRequest {
        pages: vec![page],
        copies: 1,
        density: 2,
        media: MediaType::Gap,
        planes: PagePlanes::None,
        half_cut: Some(true),
    };
    drv.print(&job, &JobControl::new()).unwrap();
    let log = sim.log();
    assert_eq!(log.half_cut, Some(true));
    let at = |c: u8| log.commands.iter().position(|x| *x == c).unwrap();
    assert!(at(0x5C) < at(0x01));
    // The H1S page size carries the cut fields; they stay zero.
    assert_eq!((log.page_size_len, log.cut), (Some(9), Some((0, 0))));
}

#[test]
fn oss_only_models_are_registered() {
    let registry = Registry::builtin();
    for (id, code, name) in [
        ("niimbot.a1_pro", 7424, "A1_PRO-1234"),
        ("niimbot.a8", 256, "A8-1234"),
        ("niimbot.d11_h", 528, "D11_H-1234"),
        ("niimbot.d11_pro", 531, "D11_PRO-1234"),
        ("niimbot.d110_m", 2320, "D110_M-1234"),
        ("niimbot.d11s", 514, "D11S-1234"),
        ("niimbot.et10", 5376, "ET10-1234"),
        ("niimbot.h1", 3840, "H1-1234"),
        ("niimbot.hi_d110", 2305, "Hi-D110-1234"),
    ] {
        let model = registry.by_id(id).unwrap();
        assert!(model.model_codes.contains(&code), "{id}");
        assert_eq!(registry.guess_by_name(name).unwrap().id, id);
    }
    // A shared code still resolves to the vendor's model when the name does not say.
    assert_eq!(registry.by_code("niimbot", 256).unwrap().id, "niimbot.b3s");
    assert_eq!(
        registry.by_code("niimbot", 2305).unwrap().id,
        "niimbot.d110"
    );
    assert_eq!(
        registry.guess_by_name("H1S-1234").unwrap().id,
        "niimbot.h1s"
    );
}
