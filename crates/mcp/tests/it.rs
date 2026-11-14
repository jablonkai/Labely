//! MCP end to end: an agent builds, previews, checks and prints a label (Phase 2 exit
//! criterion), over an in-memory transport and over HTTP.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use labely_core::{Policy, Session};
use labely_mcp::{LabelyMcp, LocalExecutor};
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock};
use serde_json::{Value, json};

fn server(root: &std::path::Path) -> (LabelyMcp, Arc<LocalExecutor>) {
    let session = Session::with_policy(
        labely_printers::default_services(),
        Policy::restricted(vec![root.to_path_buf()]),
    );
    let exec = Arc::new(LocalExecutor::new(session));
    (LabelyMcp::new(exec.clone()), exec)
}

fn args(v: Value) -> serde_json::Map<String, Value> {
    v.as_object().cloned().unwrap()
}

fn envelope(r: &CallToolResult) -> Value {
    r.structured_content.clone().unwrap()
}

async fn call<S>(
    client: &rmcp::service::RunningService<rmcp::RoleClient, S>,
    name: &str,
    a: Value,
) -> CallToolResult
where
    S: rmcp::Service<rmcp::RoleClient>,
{
    client
        .call_tool(CallToolRequestParams::new(name.to_owned()).with_arguments(args(a)))
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_builds_previews_checks_and_prints() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let (srv, exec) = server(&root);
    let (a, b) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let s = srv.serve(a).await.unwrap();
        s.waiting().await.ok();
    });
    let client = ().serve(b).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    let names: Vec<_> = tools.iter().map(|t| t.name.to_string()).collect();
    for n in [
        "command_list",
        "command_run",
        "command_batch",
        "render_preview",
        "check",
        "print",
    ] {
        assert!(names.contains(&n.to_owned()), "{n} missing in {names:?}");
    }
    let info = client.peer_info().unwrap();
    assert!(
        info.instructions
            .as_deref()
            .unwrap()
            .contains("command_batch")
    );

    let r = call(
        &client,
        "document_new",
        json!({"width_mm": 50, "height_mm": 30, "printer_model": "niimbot.b21_pro"}),
    )
    .await;
    assert_eq!(envelope(&r)["ok"], true, "{:?}", envelope(&r));

    let r = call(
        &client,
        "command_batch",
        json!({"label": "agent label", "steps": [
            {"id": "element.add_text", "params": {"x_mm": 2, "y_mm": 2, "width_mm": 46, "height_mm": 7, "text": "Hello MCP", "font_size_pt": 14}},
            {"id": "element.add_barcode", "params": {"x_mm": 2, "y_mm": 11, "width_mm": 30, "height_mm": 12, "data": "LBL-0042"}},
            {"id": "element.add_code2d", "params": {"x_mm": 36, "y_mm": 11, "width_mm": 12, "height_mm": 12, "data": "https://example.org"}},
            {"id": "element.set_props", "params": {"id": "$0.id", "props": {"bold": true}}},
            {"id": "selection.set", "params": {"ids": ["$0.id", "$1.id"]}}
        ]}),
    )
    .await;
    let env = envelope(&r);
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["created"].as_array().unwrap().len(), 3);

    let r = call(&client, "render_preview", json!({})).await;
    assert!(r.content.iter().any(
        |c| matches!(c, ContentBlock::Image(i) if i.mime_type == "image/png" && !i.data.is_empty())
    ));
    let env = envelope(&r);
    assert_eq!(env["result"]["dpi"], 300);
    assert!(
        env["result"].get("png_base64").is_none(),
        "png is only in the image block"
    );

    let r = call(&client, "check", json!({})).await;
    let env = envelope(&r);
    assert_eq!(env["result"]["ok"], true, "{env}");

    let out = root.join("printed");
    let address = format!("fake:{}?model=niimbot.b21_pro", out.display());
    let r = call(&client, "print", json!({"address": address})).await;
    assert_eq!(r.is_error, Some(true));
    assert_eq!(envelope(&r)["error"]["code"], "forbidden");

    exec.with_session(|s| {
        let mut p = s.policy().clone();
        p.allow_print = true;
        s.set_policy(p);
    });
    let r = call(
        &client,
        "command_run",
        json!({"id": "device.connect", "params": {"address": "fake:/"}}),
    )
    .await;
    assert_eq!(
        envelope(&r)["error"]["code"],
        "forbidden",
        "fake printers only write below the roots"
    );
    let r = call(
        &client,
        "command_run",
        json!({"id": "device.connect", "params": {"address": address}}),
    )
    .await;
    assert_eq!(envelope(&r)["ok"], true, "{:?}", envelope(&r));
    let r = call(&client, "print", json!({"address": address})).await;
    assert_eq!(envelope(&r)["ok"], true, "{:?}", envelope(&r));
    assert!(
        out.join("job-0001/page-01.png").is_file(),
        "FakePrinter wrote the page"
    );

    // File roots.
    let r = call(
        &client,
        "document_save",
        json!({"path": "/tmp/../etc/labely-escape.labely"}),
    )
    .await;
    assert_eq!(envelope(&r)["error"]["code"], "forbidden");
    let r = call(
        &client,
        "document_save",
        json!({"path": root.join("a.labely")}),
    )
    .await;
    assert_eq!(envelope(&r)["ok"], true, "{:?}", envelope(&r));

    // Generic tool, errors with hints, undo.
    let r = call(
        &client,
        "command_run",
        json!({"id": "element.add_txt", "params": {}}),
    )
    .await;
    let env = envelope(&r);
    assert_eq!(env["error"]["code"], "unknown_command");
    assert_eq!(env["error"]["accepted"][0], "element.add_text");
    let r = call(&client, "undo", json!({})).await;
    assert_eq!(envelope(&r)["result"]["label"], "agent label");

    client.cancel().await.ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn http_requires_token_and_local_origin() {
    let dir = tempfile::tempdir().unwrap();
    let (srv, _) = server(dir.path());
    let token = labely_mcp::connection::new_token().unwrap();
    let http = labely_mcp::http::serve(srv, 0, token.clone())
        .await
        .unwrap();

    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#;
    let post = |auth: Option<String>, origin: Option<&'static str>| {
        let c = reqwest::Client::new();
        let mut rq = c
            .post(&http.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(body);
        if let Some(a) = auth {
            rq = rq.header("authorization", format!("Bearer {a}"));
        }
        if let Some(o) = origin {
            rq = rq.header("origin", o);
        }
        rq.send()
    };
    assert_eq!(post(None, None).await.unwrap().status(), 401);
    assert_eq!(
        post(Some("wrong".into()), None).await.unwrap().status(),
        401
    );
    assert_eq!(
        post(Some(token.clone()), Some("https://evil.example"))
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        post(Some(token.clone()), Some("http://localhost:5173"))
            .await
            .unwrap()
            .status(),
        200
    );

    // A real client through the bridge's transport.
    let info = labely_mcp::connection::ConnectionInfo {
        url: http.url.clone(),
        token,
        pid: 0,
    };
    let bridge = labely_mcp::bridge::connect(&info).await.unwrap();
    let (a, b) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let s = bridge.serve(a).await.unwrap();
        s.waiting().await.ok();
    });
    let client = ().serve(b).await.unwrap();
    let r = call(
        &client,
        "document_new",
        json!({"width_mm": 40, "height_mm": 20}),
    )
    .await;
    assert_eq!(envelope(&r)["ok"], true);
    let r = call(&client, "document_get", json!({})).await;
    assert_eq!(
        envelope(&r)["result"]["document"]["label"]["width_mm"],
        40.0
    );
    client.cancel().await.ok();
    http.shutdown();
}
