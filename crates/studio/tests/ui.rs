//! Studio UI tests with egui_kittest: menus, canvas gestures, undo, and an agent editing the
//! open document over MCP (Phase 2 exit criterion, "live in the studio").
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use egui::{Key, Modifiers, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use labely_i18n::Lang;
use labely_studio::{LabelyApp, StudioOptions};
use serde_json::json;

fn options() -> StudioOptions {
    StudioOptions {
        mcp: false,
        native_dialogs: false,
        lang: Lang::En,
        // Never write the real app-data history from tests.
        history_dir: Some(std::env::temp_dir().join("labely-studio-test-history")),
        paper_file: Some(std::env::temp_dir().join("labely-studio-test-papers.json")),
        ..StudioOptions::default()
    }
}

fn harness(app: LabelyApp) -> Harness<'static, LabelyApp> {
    Harness::builder()
        .with_size(vec2(1100.0, 720.0))
        .build_ui_state(|ui, app: &mut LabelyApp| app.show(ui), app)
}

fn element_count(h: &Harness<'_, LabelyApp>) -> usize {
    h.state()
        .session
        .doc(None)
        .map_or(0, |d| d.elements().count())
}

#[test]
fn new_document_add_text_and_undo() {
    let mut h = harness(LabelyApp::new(None, options()));
    h.run();
    h.get_by_label("New label…").click();
    h.run();
    h.get_by_label("Create").click();
    h.run();
    let doc = h.state().session.doc(None).expect("document created");
    assert_eq!(doc.label.width_mm, 50.0);
    assert_eq!(doc.label.printer_model.as_deref(), Some("niimbot.b21_pro"));

    h.key_press(Key::T);
    h.run();
    assert_eq!(element_count(&h), 1);
    assert_eq!(h.state().selection().len(), 1, "a new element is selected");
    // The properties panel shows the text kind.
    assert!(h.query_by_label("Content").is_some());

    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(element_count(&h), 0);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    h.run();
    assert_eq!(element_count(&h), 1);
}

#[test]
fn menus_are_generated_and_localised() {
    let mut app = LabelyApp::new(
        None,
        StudioOptions {
            lang: Lang::Hu,
            ..options()
        },
    );
    app.run("doc.new", json!({"width_mm": 40, "height_mm": 20}));
    let mut h = harness(app);
    h.run();
    h.get_by_label("Beszúrás").click();
    h.run();
    h.get_by_label_contains("Vonalkód").click();
    h.run();
    let doc = h.state().session.doc(None).unwrap();
    assert_eq!(doc.elements().next().unwrap().kind.type_name(), "barcode");
}

#[test]
fn dragging_moves_and_resizes_in_one_undo_step_each() {
    let mut app = LabelyApp::new(None, options());
    app.run("doc.new", json!({"width_mm": 50, "height_mm": 30}));
    let env = app.run(
        "element.add_rect",
        json!({"x_mm": 10, "y_mm": 10, "width_mm": 10, "height_mm": 6}),
    );
    let id = env.created[0];
    let mut h = harness(app);
    h.run();
    let view = h.state().ui.last_view.expect("canvas drawn");
    let start = view.to_screen(15.0, 13.0);
    let end = start + vec2(5.0 * view.zoom, 2.0 * view.zoom);
    drag(&mut h, start, end);
    let e = h
        .state()
        .session
        .doc(None)
        .unwrap()
        .element(id)
        .unwrap()
        .clone();
    assert!(
        (e.x_mm - 15.0).abs() < 0.2 && (e.y_mm - 12.0).abs() < 0.2,
        "moved to {e:?}"
    );
    assert_eq!(h.state().selection(), vec![id]);

    // Bottom-right handle: grow by 4 × 2 mm. (Panels may have re-laid out the canvas.)
    let view = h.state().ui.last_view.unwrap();
    let corner = view.to_screen(e.x_mm + e.width_mm, e.y_mm + e.height_mm);
    drag(
        &mut h,
        corner,
        corner + vec2(4.0 * view.zoom, 2.0 * view.zoom),
    );
    let r = h
        .state()
        .session
        .doc(None)
        .unwrap()
        .element(id)
        .unwrap()
        .clone();
    assert!(
        (r.width_mm - 14.0).abs() < 0.2 && (r.height_mm - 8.0).abs() < 0.2,
        "{r:?}"
    );
    assert!((r.x_mm - e.x_mm).abs() < 1e-9, "the opposite corner stays");

    let s = &mut h.state_mut().session;
    assert_eq!(
        s.execute("history.undo", json!({})).result.unwrap()["label"],
        "element.set_props"
    );
    assert_eq!(
        s.execute("history.undo", json!({})).result.unwrap()["label"],
        "element.move"
    );
    let back = s.doc(None).unwrap().element(id).unwrap();
    assert_eq!((back.x_mm, back.y_mm, back.width_mm), (10.0, 10.0, 10.0));
}

fn drag(h: &mut Harness<'_, LabelyApp>, from: Pos2, to: Pos2) {
    h.hover_at(from);
    h.run();
    h.drag_at(from);
    h.run();
    for i in 1..=4 {
        h.hover_at(from + (to - from) * (i as f32 / 4.0));
        h.run();
    }
    h.drop_at(to);
    h.run();
}

#[test]
fn agent_edits_the_open_document_live() {
    let dir = tempfile::tempdir().unwrap();
    let conn = dir.path().join("mcp-connection.json");
    let root = dir.path().canonicalize().unwrap();
    let mut h = harness(LabelyApp::new(
        None,
        StudioOptions {
            mcp: true,
            mcp_port: 0,
            agent_roots: vec![root.clone()],
            connection_file: Some(conn.clone()),
            ..options()
        },
    ));
    let ctx = h.ctx.clone();
    h.state_mut().start_mcp(&ctx);
    assert!(h.state().mcp_url().is_some(), "{:?}", h.state().ui.status);
    let info = labely_mcp::connection::read(&conn)
        .unwrap()
        .expect("connection file");

    let printed = root.join("out");
    let address = format!("fake:{}", printed.display());
    let agent = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            use rmcp::ServiceExt;
            use rmcp::model::CallToolRequestParams;
            use rmcp::transport::StreamableHttpClientTransport;
            use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
            let t = StreamableHttpClientTransport::from_config(
                StreamableHttpClientTransportConfig::with_uri(info.url.clone())
                    .auth_header(info.token.clone()),
            );
            let client = ().serve(t).await.unwrap();
            let call = |name: &str, a: serde_json::Value| {
                let p = CallToolRequestParams::new(name.to_owned())
                    .with_arguments(a.as_object().cloned().unwrap());
                let c = &client;
                async move { c.call_tool(p).await.unwrap().structured_content.unwrap() }
            };
            let mut out = Vec::new();
            out.push(call("document_new", json!({"width_mm": 40, "height_mm": 25})).await);
            out.push(
                call(
                    "command_batch",
                    json!({"steps": [
                        {"id": "element.add_text", "params": {"x_mm": 2, "y_mm": 2, "width_mm": 36, "height_mm": 8, "text": "From the agent"}},
                        {"id": "element.add_code2d", "params": {"x_mm": 2, "y_mm": 11, "width_mm": 12, "height_mm": 12, "data": "42"}},
                        {"id": "selection.set", "params": {"ids": ["$1.id"]}}
                    ]}),
                )
                .await,
            );
            out.push(call("render_preview", json!({})).await);
            out.push(call("check", json!({})).await);
            out.push(call("command_run", json!({"id": "device.connect", "params": {"address": address}})).await);
            out.push(call("print", json!({"address": address})).await);
            // The user ticks "Allow agents to print"; then the same call goes through.
            let mut last = json!(null);
            for _ in 0..200 {
                last = call("print", json!({"address": address})).await;
                if last["ok"] == true {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            out.push(last);
            client.cancel().await.ok();
            out
        })
    });
    while !agent.is_finished() {
        if h.state()
            .ui
            .agent_log
            .iter()
            .any(|l| l.contains("print.submit"))
        {
            h.state_mut().ui.allow_agent_print = true;
        }
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let out = agent.join().unwrap();
    for (i, env) in out.iter().take(5).enumerate() {
        assert_eq!(env["ok"], true, "call {i}: {env}");
    }
    assert_eq!(
        out[5]["error"]["code"], "forbidden",
        "printing is off until the user allows it"
    );
    assert_eq!(out[6]["ok"], true, "after the user allowed it: {}", out[6]);
    assert!(printed.join("job-0001/page-01.png").is_file());
    h.run();
    let app = h.state();
    let doc = app.session.doc(None).unwrap();
    assert_eq!(doc.elements().count(), 2, "the user sees the agent's label");
    assert_eq!(app.selection().len(), 1);
    assert!(app.ui.agent_log.iter().any(|l| l.contains("command.batch")));
    assert!(
        h.query_by_label_contains("From the agent").is_some(),
        "layers list shows it"
    );
}

#[test]
fn data_panel_binds_and_insert_date() {
    let mut app = LabelyApp::new(None, options());
    app.run("doc.new", json!({"width_mm": 50, "height_mm": 30}));
    app.run(
        "data.attach_csv",
        json!({"text": "name,sku\nApple,A1\nPear,P2\n"}),
    );
    app.run(
        "element.add_text",
        json!({"x_mm": 1, "y_mm": 1, "width_mm": 40, "height_mm": 8, "text": "x"}),
    );
    let id = app.session.doc(None).unwrap().elements().next().unwrap().id;
    app.select(&[id]);
    app.ui.show_data = true;
    let mut h = harness(app);
    h.run();
    assert!(h.query_by_label_contains("2 records").is_some());

    // Bind the selected text to the first column, then step to record 2.
    h.get_all_by_label("Bind").next().unwrap().click();
    h.run();
    let doc = h.state().session.doc(None).unwrap();
    match &doc.element(id).unwrap().kind {
        labely_core::ElementKind::Text(t) => assert_eq!(t.text, "{{name}}"),
        k => panic!("{k:?}"),
    }
    h.get_by_label("▶").click();
    h.run();
    assert_eq!(h.state().ui.record, 2);
    assert!(h.query_by_label("Pear").is_some(), "panel shows record 2");

    // Insert → Serial number creates the counter and a bound text.
    h.get_by_label("Insert").click();
    h.run();
    h.get_by_label("Serial number").click();
    h.run();
    let doc = h.state().session.doc(None).unwrap();
    assert!(doc.serials.contains_key("serial"));
    assert_eq!(doc.elements().count(), 2);
}

#[test]
fn table_editing_and_code_from_image() {
    let mut app = LabelyApp::new(None, options());
    app.run("doc.new", json!({"width_mm": 50, "height_mm": 30}));
    let mut h = harness(app);
    h.run();
    h.get_by_label("Insert").click();
    h.run();
    h.get_by_label("Table").click();
    h.run();
    let id = h.state().selection()[0];
    h.get_by_label("Merge right").click();
    h.run();
    h.get_by_label("+ Row").click();
    h.run();
    let t = match &h
        .state()
        .session
        .doc(None)
        .unwrap()
        .element(id)
        .unwrap()
        .kind
    {
        labely_core::ElementKind::Table(t) => t.clone(),
        k => panic!("{k:?}"),
    };
    assert_eq!(t.row_heights.len(), 4);
    assert_eq!(t.merges.len(), 1);

    // A QR rendered by Labely is read back and recreated.
    let app = h.state_mut();
    app.run("doc.new", json!({"width_mm": 30, "height_mm": 30}));
    app.run(
        "element.add_code2d",
        json!({"x_mm": 5, "y_mm": 5, "width_mm": 20, "height_mm": 20, "data": "from image"}),
    );
    let png = app.run("render.preview", json!({})).result.unwrap()["png_base64"].clone();
    app.run("doc.new", json!({"width_mm": 30, "height_mm": 30}));
    labely_studio::menu::insert_code_from_image(app, &json!({ "data_base64": png }));
    let doc = app.session.doc(None).unwrap();
    let e = doc.elements().next().expect("code added");
    match &e.kind {
        labely_core::ElementKind::Code2d(c) => assert_eq!(c.data, "from image"),
        k => panic!("{k:?}"),
    }
}

#[test]
fn group_select_and_align() {
    let mut app = LabelyApp::new(None, options());
    app.run("doc.new", json!({"width_mm": 50, "height_mm": 30}));
    for x in [5, 20, 35] {
        app.run(
            "element.add_rect",
            json!({"x_mm": x, "y_mm": x / 5, "width_mm": 6, "height_mm": 6}),
        );
    }
    let ids: Vec<_> = app
        .session
        .doc(None)
        .unwrap()
        .elements()
        .map(|e| e.id)
        .collect();
    app.select(&ids[..2]);
    let mut h = harness(app);
    h.run();
    h.get_by_label("Arrange").click();
    h.run();
    h.get_by_label_contains("Group").click();
    h.run();
    // Selecting one member selects the group.
    h.state_mut().select(&ids[..1]);
    assert_eq!(h.state().selection(), ids[..2].to_vec());
    h.get_by_label("Arrange").click();
    h.run();
    h.get_by_label("Align").click();
    h.run();
    h.get_by_label("Top edges").click();
    h.run();
    let doc = h.state().session.doc(None).unwrap();
    let ys: Vec<f64> = doc.elements().map(|e| e.y_mm).collect();
    assert_eq!(ys, vec![1.0, 1.0, 7.0]);
}

#[test]
fn template_gallery_open_save_and_manage() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(LabelyApp::new(
        None,
        StudioOptions {
            template_dir: Some(dir.path().to_path_buf()),
            ..options()
        },
    ));
    h.run();
    // Welcome screen → gallery → open a starter by double-click.
    h.get_by_label("New from template…").click();
    h.run();
    assert!(h.query_by_label("Starter templates").is_some());
    h.get_by_label("Price tag").click();
    h.run();
    h.get_by_label("Open").click();
    h.run();
    let doc = h.state().session.doc(None).expect("opened from template");
    assert_eq!(doc.name, "Price tag");
    assert_eq!(doc.elements().count(), 3);
    assert!(h.state().ui.dialog.is_none());

    // File → Save as template… into a new folder.
    h.get_by_label("File").click();
    h.run();
    h.get_by_label("Save as template…").click();
    h.run();
    let folder = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Folder");
    folder.focus();
    folder.type_text("Shop");
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert!(dir.path().join("Shop/Price tag.labely").is_file());
    // Saving again asks before replacing.
    h.get_by_label("File").click();
    h.run();
    h.get_by_label("Save as template…").click();
    h.run();
    let folder = h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Folder");
    folder.focus();
    folder.type_text("Shop");
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert!(h.query_by_label("Replace").is_some());
    h.get_by_label("Replace").click();
    h.run();
    assert!(h.state().ui.dialog.is_none());

    // The gallery lists the folder; delete the template there.
    h.get_by_label("File").click();
    h.run();
    h.get_by_label_contains("New from template…").click();
    h.run();
    h.get_by_label("  🗀 Shop").click();
    h.run();
    h.get_by_label("Price tag").click();
    h.run();
    h.get_by_label("Delete").click();
    h.run();
    assert!(!dir.path().join("Shop/Price tag.labely").exists());
}

#[test]
fn print_queue_panel_and_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = LabelyApp::new(
        None,
        StudioOptions {
            history_dir: Some(dir.path().join("history")),
            ..options()
        },
    );
    assert!(
        app.run(
            "doc.new",
            json!({"name": "Queue test", "width_mm": 30, "height_mm": 20})
        )
        .ok
    );
    app.ui.print.address = format!("fake:{}", dir.path().join("out").display());
    app.ui.dialog = Some(labely_studio::app::Dialog::Print);
    let mut h = harness(app);
    h.run();
    // The dialog's Print button queues the job and opens the printers panel.
    h.get_all_by_label("Print").last().unwrap().click();
    // Fixed steps: the UI keeps repainting while the job is busy.
    h.run_steps(3);
    assert!(h.state().ui.dialog.is_none());
    assert!(h.state().ui.show_printers);
    let job = h.state().session.jobs().list()[0].id.clone();
    h.state().session.jobs().wait(&job, None).unwrap();
    h.run();
    assert!(h.query_by_label("Print jobs").is_some());
    assert!(
        h.state()
            .ui
            .status
            .as_ref()
            .is_some_and(|s| s.text == "Printed: Queue test"),
        "{:?}",
        h.state().ui.status
    );
    assert!(dir.path().join("out/job-0001/page-01.png").is_file());

    // History dialog: the entry reopens the printed document.
    h.get_by_label("Print history…").click();
    h.run();
    assert!(h.query_by_label("Queue test").is_some());
    h.get_by_label("Open").click();
    h.run();
    assert!(h.state().ui.dialog.is_none());
    assert_eq!(h.state().session.doc(None).unwrap().name, "Queue test");
    assert_eq!(h.state().session.doc_ids().count(), 2);
}

#[test]
fn paper_library_and_printer_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = LabelyApp::new(
        None,
        StudioOptions {
            paper_file: Some(dir.path().join("papers.json")),
            ..options()
        },
    );
    assert!(
        app.run(
            "doc.new",
            json!({"width_mm": 40, "height_mm": 30, "printer_model": "niimbot.b21_pro"})
        )
        .ok
    );
    let mut h = harness(app);
    h.run();
    h.get_all_by_label("Label").next().unwrap().click();
    h.run();
    h.get_by_label("Label paper…").click();
    h.run();
    assert!(h.query_by_label("Label paper").is_some());
    let search = h.get_by_label("Search");
    search.focus();
    search.type_text("25x25-round");
    h.run();
    h.get_by_label("Use").click();
    h.run();
    assert!(
        h.state().ui.dialog.is_none(),
        "using a paper closes the dialog"
    );
    let label = h.state().session.doc(None).unwrap().label.clone();
    assert_eq!((label.width_mm, label.corner_radius_mm), (25.0, 3.0));

    // Add a custom paper.
    h.get_all_by_label("Label").next().unwrap().click();
    h.run();
    h.get_by_label("Label paper…").click();
    h.run();
    let name = h
        .get_all_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
        .last()
        .unwrap();
    name.focus();
    name.type_text("Jar lid");
    h.run();
    h.get_by_label("Add paper").click();
    h.run();
    let saved = std::fs::read_to_string(dir.path().join("papers.json")).unwrap();
    assert!(saved.contains("my:jar-lid"), "{saved}");
    h.get_by_label("Close").click();
    h.run();

    // Printer settings dialog on a fake printer: calibrate.
    let out = dir.path().join("out");
    let address = format!("fake:{}", out.display());
    assert!(
        h.state_mut()
            .run("device.connect", json!({"address": address}))
            .ok
    );
    h.get_all_by_label("Print").next().unwrap().click();
    h.run();
    h.get_by_label("Printer settings and calibration…").click();
    h.run();
    assert!(h.query_by_label("Printer settings").is_some());
    h.get_by_label("Calibrate").click();
    h.run();
    assert!(out.join("calibrate-0001.json").is_file());
    assert_eq!(
        h.state().ui.status.as_ref().map(|s| s.text.as_str()),
        Some("Calibration done.")
    );
}
