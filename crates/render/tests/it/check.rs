//! `doc.check` rules that need the real rasteriser: quiet zone and image contrast.

use std::sync::Arc;

use labely_core::{Services, Session};
use labely_render::LabelRenderer;
use serde_json::{Value, json};

fn session() -> Session {
    let mut s = Session::new(Services {
        renderer: Arc::new(LabelRenderer::new()),
        devices: Arc::new(labely_testkit::FakeDeviceHub::new()),
    });
    assert!(
        s.execute("doc.new", json!({"width_mm": 50, "height_mm": 30}))
            .ok
    );
    s
}

fn rules(s: &mut Session) -> Vec<(String, Option<u64>)> {
    let env = s.execute("doc.check", json!({"dpi": 300}));
    assert!(env.ok, "{:?}", env.error);
    env.result.unwrap()["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["rule"].as_str().unwrap().to_owned(),
                i["element_id"].as_u64(),
            )
        })
        .collect()
}

fn add(s: &mut Session, id: &str, p: Value) -> u64 {
    let env = s.execute(id, p);
    assert!(env.ok, "{:?}", env.error);
    env.created[0].0
}

#[test]
fn quiet_zone_needs_space_beside_a_barcode() {
    let mut s = session();
    let tight = add(
        &mut s,
        "element.add_barcode",
        json!({"x_mm": 0, "y_mm": 2, "width_mm": 50, "height_mm": 10, "data": "LABELY-1234"}),
    );
    let roomy = add(
        &mut s,
        "element.add_barcode",
        json!({"x_mm": 10, "y_mm": 16, "width_mm": 30, "height_mm": 10, "data": "12"}),
    );
    let r = rules(&mut s);
    assert!(r.contains(&("quiet_zone".to_owned(), Some(tight))), "{r:?}");
    assert!(
        !r.contains(&("quiet_zone".to_owned(), Some(roomy))),
        "{r:?}"
    );
}

#[test]
fn blank_image_is_low_contrast() {
    let mut s = session();
    let white = labely_render::bitmap_to_png(&labely_core::Bitmap1::new(32, 32)).unwrap();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, white);
    let id = add(
        &mut s,
        "element.add_image",
        json!({"x_mm": 5, "y_mm": 5, "width_mm": 10, "height_mm": 10, "data_base64": data}),
    );
    let r = rules(&mut s);
    assert!(r.contains(&("low_contrast".to_owned(), Some(id))), "{r:?}");
}
