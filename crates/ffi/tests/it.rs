//! `LabelyEngine` end to end, with a Rust stand-in for the shell's BLE stack that talks to the
//! printer simulator.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use labely_ffi::{
    BleDevice, BleError, BleNotifySink, DeviceEvents, EngineConfig, LabelyEngine, NativeBle,
    NativeBleLink,
};
use labely_printers::sim::{SimConfig, Simulator};
use labely_transport::Transport;
use labely_transport::memory::MemoryTransport;
use serde_json::{Value, json};

/// Plays the Kotlin/Swift side: one B21 Pro at `AA:BB`, a fresh simulator per connect.
#[derive(Default)]
struct ShellBle {
    sims: Mutex<Vec<Simulator>>,
    connects: Mutex<Vec<String>>,
    /// `open` flag of each link; clearing one drops the link as walking out of range does.
    links: Mutex<Vec<Arc<AtomicBool>>>,
}

struct ShellLink {
    t: Arc<Mutex<MemoryTransport>>,
    open: Arc<AtomicBool>,
}

impl NativeBleLink for ShellLink {
    fn write(&self, bytes: Vec<u8>) -> Result<(), BleError> {
        assert!(
            bytes.len() <= 20,
            "chunk of {} bytes over the MTU",
            bytes.len()
        );
        self.t
            .lock()
            .unwrap()
            .send(&bytes)
            .map_err(|e| BleError::Failed {
                detail: e.to_string(),
            })
    }
    fn mtu(&self) -> u32 {
        20
    }
    fn name(&self) -> String {
        "B21_Pro-1234".into()
    }
    fn disconnect(&self) {
        self.open.store(false, Ordering::Release);
    }
}

impl NativeBle for ShellBle {
    fn scan(
        &self,
        _timeout_ms: u64,
        name_prefixes: Vec<String>,
    ) -> Result<Vec<BleDevice>, BleError> {
        assert!(
            name_prefixes.iter().any(|p| p == "B21"),
            "{name_prefixes:?}"
        );
        Ok(vec![BleDevice {
            id: "AA:BB".into(),
            name: "B21_Pro-1234".into(),
            rssi: Some(-60),
        }])
    }

    fn connect(
        &self,
        id: String,
        sink: Arc<BleNotifySink>,
    ) -> Result<Arc<dyn NativeBleLink>, BleError> {
        self.connects.lock().unwrap().push(id.clone());
        if id != "AA:BB" {
            return Err(BleError::NotFound { detail: id });
        }
        let (sim, t) = Simulator::start(SimConfig::b21_pro());
        self.sims.lock().unwrap().push(sim);
        let t = Arc::new(Mutex::new(t));
        let open = Arc::new(AtomicBool::new(true));
        self.links.lock().unwrap().push(Arc::clone(&open));
        let (t2, open2) = (Arc::clone(&t), Arc::clone(&open));
        std::thread::spawn(move || {
            while open2.load(Ordering::Acquire) {
                let got = t2.lock().unwrap().recv(Duration::from_millis(5));
                match got {
                    Ok(Some(b)) => sink.push(b),
                    Ok(None) => std::thread::yield_now(),
                    Err(_) => break,
                }
            }
            sink.disconnected();
        });
        Ok(Arc::new(ShellLink { t, open }))
    }
}

fn engine(ble: Option<Arc<dyn NativeBle>>, dir: &std::path::Path) -> Arc<LabelyEngine> {
    LabelyEngine::new(
        EngineConfig {
            data_dir: dir.display().to_string(),
            file_roots: vec![],
            allow_print: true,
        },
        ble,
    )
}

fn run(e: &LabelyEngine, id: &str, params: Value) -> Value {
    let out: Value = serde_json::from_str(&e.execute(id.into(), params.to_string())).unwrap();
    assert_eq!(out["ok"], true, "{id}: {out}");
    out["result"].clone()
}

#[test]
fn prints_over_native_ble() {
    let dir = tempfile::tempdir().unwrap();
    let ble = Arc::new(ShellBle::default());
    let e = engine(Some(ble.clone()), dir.path());

    run(
        &e,
        "doc.new",
        json!({"width_mm": 20, "height_mm": 8, "printer_model": "niimbot.b21_pro"}),
    );
    run(
        &e,
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 18, "height_mm": 6, "text": "Hi"}),
    );
    let png = e.render_png(None, 203, 0).unwrap();
    assert_eq!(&png[1..4], b"PNG");

    let found = run(&e, "device.scan", json!({"timeout_ms": 50}));
    assert_eq!(found["devices"][0]["address"], "ble:AA:BB", "{found}");
    assert_eq!(found["devices"][0]["model"]["id"], "niimbot.b21_pro");

    let dev = run(&e, "device.connect", json!({"address": "ble:AA:BB"}));
    assert_eq!(dev["serial"], "SIM0000001", "{dev}");
    let job = run(&e, "print.submit", json!({"address": "ble:AA:BB"}));
    assert_eq!(job["state"], "done", "{job}");

    let sims = ble.sims.lock().unwrap();
    assert_eq!(sims[0].log().pages.len(), 1);
    drop(sims);
    run(&e, "device.disconnect", json!({"address": "ble:AA:BB"}));
}

#[test]
fn errors_come_back_as_envelopes() {
    let dir = tempfile::tempdir().unwrap();
    let ble = Arc::new(ShellBle::default());
    let e = engine(Some(ble.clone()), dir.path());

    let bad: Value = serde_json::from_str(&e.execute("doc.new".into(), "[1]".into())).unwrap();
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["error"]["code"], "bad_params", "{bad}");
    let bad: Value = serde_json::from_str(&e.execute("doc.new".into(), "{".into())).unwrap();
    assert_eq!(bad["error"]["code"], "bad_params");
    let unknown: Value = serde_json::from_str(&e.execute("no.such".into(), "".into())).unwrap();
    assert_eq!(unknown["ok"], false);

    let fail: Value = serde_json::from_str(&e.execute(
        "device.connect".into(),
        json!({"address": "ble:CC:DD"}).to_string(),
    ))
    .unwrap();
    assert_eq!(fail["ok"], false, "{fail}");
    assert!(
        fail["error"]["message"].as_str().unwrap().contains("CC:DD"),
        "{fail}"
    );
    assert_eq!(*ble.connects.lock().unwrap(), vec!["CC:DD".to_string()]);

    assert!(e.render_png(None, 203, 0).is_err());
}

#[test]
fn policy_limits_files_and_printing() {
    let dir = tempfile::tempdir().unwrap();
    let e = LabelyEngine::new(
        EngineConfig {
            data_dir: dir.path().display().to_string(),
            file_roots: vec![],
            allow_print: false,
        },
        None,
    );
    run(&e, "doc.new", json!({"width_mm": 20, "height_mm": 8}));
    let inside = dir.path().join("a.labely");
    run(
        &e,
        "doc.save",
        json!({"path": inside.display().to_string()}),
    );
    assert!(inside.exists());

    let outside: Value = serde_json::from_str(&e.execute(
        "doc.save".into(),
        json!({"path": "/etc/labely.labely"}).to_string(),
    ))
    .unwrap();
    assert_eq!(outside["error"]["code"], "forbidden", "{outside}");

    let addr = format!("fake:{}", dir.path().join("out").display());
    run(&e, "device.connect", json!({"address": addr}));
    let print: Value = serde_json::from_str(
        &e.execute("print.submit".into(), json!({"address": addr}).to_string()),
    )
    .unwrap();
    assert_eq!(print["error"]["code"], "forbidden", "{print}");
    e.set_allow_print(true);
    run(&e, "print.submit", json!({"address": addr}));

    let cat: Value = serde_json::from_str(&e.commands_json()).unwrap();
    assert!(
        cat["result"]["commands"].as_array().unwrap().len() > 50,
        "{cat}"
    );
    assert_eq!(e.api_version(), labely_core::API_VERSION);
}

/// Plays the shell's event listener. It calls back into the engine, as a UI refresh would.
struct Recorder {
    engine: Mutex<std::sync::Weak<LabelyEngine>>,
    tx: Mutex<std::sync::mpsc::Sender<(String, Value)>>,
}

impl DeviceEvents for Recorder {
    fn job_changed(&self, job_json: String) {
        if let Some(e) = self.engine.lock().unwrap().upgrade() {
            let id: Value = serde_json::from_str(&job_json).unwrap();
            run(&e, "job.get", json!({"id": id["id"]}));
        }
        let v = serde_json::from_str(&job_json).unwrap();
        let _ = self.tx.lock().unwrap().send(("job".into(), v));
    }
    fn device_changed(&self, event_json: String) {
        let v = serde_json::from_str(&event_json).unwrap();
        let _ = self.tx.lock().unwrap().send(("device".into(), v));
    }
}

fn listen(e: &Arc<LabelyEngine>) -> std::sync::mpsc::Receiver<(String, Value)> {
    let (tx, rx) = std::sync::mpsc::channel();
    e.set_events(Some(Arc::new(Recorder {
        engine: Mutex::new(Arc::downgrade(e)),
        tx: Mutex::new(tx),
    })));
    rx
}

/// Next event of `kind`, skipping the others.
fn next(rx: &std::sync::mpsc::Receiver<(String, Value)>, kind: &str) -> Value {
    loop {
        let (k, v) = rx.recv_timeout(Duration::from_secs(10)).expect("no event");
        if k == kind {
            return v;
        }
    }
}

#[test]
fn pushes_job_and_device_events() {
    let dir = tempfile::tempdir().unwrap();
    let ble = Arc::new(ShellBle::default());
    let e = engine(Some(ble.clone()), dir.path());
    let rx = listen(&e);

    run(
        &e,
        "doc.new",
        json!({"width_mm": 20, "height_mm": 8, "printer_model": "niimbot.b21_pro"}),
    );
    run(
        &e,
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 18, "height_mm": 6, "text": "Hi"}),
    );
    run(&e, "device.connect", json!({"address": "ble:AA:BB"}));
    let dev = next(&rx, "device");
    assert_eq!(dev["address"], "ble:AA:BB", "{dev}");
    assert_eq!(dev["connected"], true);
    assert_eq!(dev["device"]["serial"], "SIM0000001", "{dev}");

    run(&e, "device.status", json!({"address": "ble:AA:BB"}));
    let st = next(&rx, "device");
    assert!(st["status"].is_object(), "{st}");

    let job = run(
        &e,
        "print.submit",
        json!({"address": "ble:AA:BB", "copies": 2, "wait": false}),
    );
    let mut states = Vec::new();
    let last = loop {
        let j = next(&rx, "job");
        assert_eq!(j["id"], job["job_id"], "{j}");
        let s = j["state"].as_str().unwrap().to_owned();
        if states.last() != Some(&s) {
            states.push(s.clone());
        }
        if s != "queued" && s != "printing" {
            break j;
        }
    };
    assert_eq!(states, ["queued", "printing", "done"]);
    assert_eq!(last["pages_done"], 2, "{last}");

    run(&e, "device.disconnect", json!({"address": "ble:AA:BB"}));
    let gone = next(&rx, "device");
    assert_eq!(gone["connected"], false, "{gone}");
    assert!(gone.get("lost").is_none(), "{gone}");
    // The shell's link reports the close too; that is not a lost link.
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn reports_a_lost_link_and_reconnects() {
    let dir = tempfile::tempdir().unwrap();
    let ble = Arc::new(ShellBle::default());
    let e = engine(Some(ble.clone()), dir.path());
    let rx = listen(&e);

    run(&e, "device.connect", json!({"address": "ble:AA:BB"}));
    next(&rx, "device");
    ble.links.lock().unwrap()[0].store(false, Ordering::Release);
    let lost = next(&rx, "device");
    assert_eq!(lost["address"], "ble:AA:BB", "{lost}");
    assert_eq!(lost["connected"], false);
    assert_eq!(lost["lost"], true);

    let devices = run(&e, "device.list", json!({}));
    assert_eq!(devices["devices"], json!([]), "{devices}");
    run(&e, "device.connect", json!({"address": "ble:AA:BB"}));
    assert_eq!(ble.connects.lock().unwrap().len(), 2);
    e.set_events(None);
}
