//! `labely-studio [FILE] [--no-mcp] [--mcp-port N] [--agent-root DIR]… [--lang en|hu]`

use std::path::PathBuf;

use labely_studio::{LabelyApp, StudioOptions};

fn parse_args() -> Result<StudioOptions, String> {
    let mut o = StudioOptions::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--no-mcp" => o.mcp = false,
            "--mcp-port" => {
                o.mcp_port = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--mcp-port needs a number")?;
            }
            "--agent-root" => o.agent_roots.push(
                args.next()
                    .map(PathBuf::from)
                    .ok_or("--agent-root needs a directory")?,
            ),
            "--lang" => {
                o.lang = args
                    .next()
                    .as_deref()
                    .and_then(labely_i18n::Lang::parse)
                    .ok_or("--lang takes en or hu")?;
            }
            "-h" | "--help" => {
                println!(
                    "labely-studio [FILE] [--no-mcp] [--mcp-port N] [--agent-root DIR]... [--lang en|hu]"
                );
                std::process::exit(0);
            }
            f if !f.starts_with('-') => o.open = Some(PathBuf::from(f)),
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(o)
}

fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("LABELY_LOG"))
        .with_writer(std::io::stderr)
        .init();
    let opts = parse_args()?;
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Labely")
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Labely",
        native,
        Box::new(move |cc| Ok(Box::new(LabelyApp::new(Some(&cc.egui_ctx), opts)))),
    )
    .map_err(|e| e.to_string())
}
