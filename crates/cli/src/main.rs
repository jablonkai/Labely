//! `labely`: scriptable front end over the command registry.
//!
//! Every subcommand is a thin wrapper around `Session::execute`; `labely run` exposes the
//! registry directly.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use clap::{Parser, Subcommand};
use labely_core::session::Session;
use serde_json::{Value, json};

#[derive(Parser)]
#[command(name = "labely", version, about = "Free, open-source label printing")]
struct Cli {
    /// Print raw JSON envelopes.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List all commands with their JSON schemas.
    Commands,
    /// Run one command (`--cmd`/`--params`) or a script of steps, optionally on a document.
    Run {
        /// Document to open first.
        file: Option<PathBuf>,
        #[arg(long)]
        cmd: Option<String>,
        /// JSON params for `--cmd`.
        #[arg(long, default_value = "{}")]
        params: String,
        /// JSON file: `[{"id": …, "params": {…}}, …]`, run as one atomic batch.
        #[arg(long, conflicts_with = "cmd")]
        script: Option<PathBuf>,
        /// Save the document after running (to FILE, or this path).
        #[arg(long)]
        save: Option<Option<PathBuf>>,
    },
    /// Render a document to PNG.
    Render {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dpi: Option<u32>,
    },
    /// Check a document for printing problems.
    Check {
        file: PathBuf,
        #[arg(long)]
        dpi: Option<u32>,
    },
    /// Print a document.
    Print {
        file: PathBuf,
        /// `ble:<id>`, `usb:<port>`, `fake:<dir>[?model=<id>]` or `sim:<model id>`.
        #[arg(short, long)]
        printer: String,
        #[arg(long, default_value_t = 1)]
        copies: u16,
        #[arg(long)]
        density: Option<u8>,
    },
    /// Show a printer's status (battery, lid, loaded labels).
    Status {
        #[arg(short, long)]
        printer: String,
    },
    /// Discover printers (Bluetooth and USB).
    Devices {
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
    },
    /// List known printer models.
    Models,
}

struct Ctx {
    session: Session,
    json: bool,
}

impl Ctx {
    /// Run a command; return its result or fail with the envelope's error.
    fn exec(&mut self, id: &str, params: Value) -> Result<Value> {
        let env = serde_json::to_value(self.session.execute(id, params))?;
        if env["ok"].as_bool() == Some(true) {
            Ok(env)
        } else {
            let e = &env["error"];
            let mut msg = format!(
                "{} ({id}): {}",
                e["code"].as_str().unwrap_or("error"),
                e["message"].as_str().unwrap_or("")
            );
            if let Some(h) = e["hint"].as_str() {
                msg.push_str(&format!("\nhint: {h}"));
            }
            if self.json {
                println!("{}", serde_json::to_string_pretty(&env)?);
            }
            bail!(msg)
        }
    }

    fn open(&mut self, file: &Path) -> Result<()> {
        self.exec("doc.open", json!({ "path": file }))
            .with_context(|| format!("opening {}", file.display()))?;
        Ok(())
    }

    fn show(&self, env: &Value) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(env)?);
        } else {
            let r = env.get("result").unwrap_or(env);
            println!("{}", serde_json::to_string_pretty(r)?);
            for w in env["warnings"].as_array().into_iter().flatten() {
                eprintln!(
                    "warning: {}",
                    w.as_str().map_or_else(|| w.to_string(), str::to_owned)
                );
            }
        }
        Ok(())
    }
}

fn run(cli: Cli) -> Result<()> {
    let mut ctx = Ctx {
        session: Session::new(labely_printers::default_services()),
        json: cli.json,
    };
    match cli.cmd {
        Cmd::Commands => {
            let env = ctx.exec("command.list", json!({}))?;
            if ctx.json {
                ctx.show(&env)?;
            } else {
                for c in env["result"]["commands"].as_array().into_iter().flatten() {
                    println!(
                        "{:<24} {}",
                        c["id"].as_str().unwrap_or(""),
                        c["description"]
                            .as_str()
                            .or(c["label"].as_str())
                            .unwrap_or("")
                    );
                }
            }
        }
        Cmd::Run {
            file,
            cmd,
            params,
            script,
            save,
        } => {
            if let Some(f) = &file {
                ctx.open(f)?;
            }
            let env = if let Some(script) = script {
                let steps: Value = serde_json::from_slice(&std::fs::read(&script)?)
                    .with_context(|| format!("parsing {}", script.display()))?;
                ctx.exec("command.batch", json!({ "steps": steps }))?
            } else if let Some(id) = cmd {
                let p: Value =
                    serde_json::from_str(&params).context("--params is not valid JSON")?;
                ctx.exec(&id, p)?
            } else {
                bail!("give --cmd <id> or --script <file>")
            };
            ctx.show(&env)?;
            if let Some(target) = save {
                let path = target
                    .or(file)
                    .context("--save needs a path when no FILE is open")?;
                ctx.exec("doc.save", json!({ "path": path }))?;
            }
        }
        Cmd::Render { file, output, dpi } => {
            ctx.open(&file)?;
            let mut p = json!({});
            if let Some(d) = dpi {
                p["dpi"] = json!(d);
            }
            let env = ctx.exec("render.preview", p)?;
            let r = &env["result"];
            let png = base64::engine::general_purpose::STANDARD.decode(
                r["png_base64"]
                    .as_str()
                    .context("render.preview returned no PNG")?,
            )?;
            std::fs::write(&output, png)?;
            if ctx.json {
                println!(
                    "{}",
                    json!({ "ok": true, "output": output, "width_dots": r["width_dots"], "height_dots": r["height_dots"], "dpi": r["dpi"] })
                );
            } else {
                eprintln!(
                    "wrote {} ({}×{} dots at {} dpi)",
                    output.display(),
                    r["width_dots"],
                    r["height_dots"],
                    r["dpi"]
                );
            }
        }
        Cmd::Check { file, dpi } => {
            ctx.open(&file)?;
            let mut p = json!({});
            if let Some(d) = dpi {
                p["dpi"] = json!(d);
            }
            let env = ctx.exec("doc.check", p)?;
            ctx.show(&env)?;
        }
        Cmd::Print {
            file,
            printer,
            copies,
            density,
        } => {
            ctx.open(&file)?;
            ctx.exec("device.connect", json!({ "address": printer }))?;
            let mut p = json!({ "address": printer, "copies": copies });
            if let Some(d) = density {
                p["density"] = json!(d);
            }
            let env = ctx.exec("print.submit", p)?;
            ctx.show(&env)?;
        }
        Cmd::Status { printer } => {
            ctx.exec("device.connect", json!({ "address": printer }))?;
            let env = ctx.exec("device.status", json!({ "address": printer }))?;
            ctx.show(&env)?;
        }
        Cmd::Devices { timeout_ms } => {
            let env = ctx.exec("device.scan", json!({ "timeout_ms": timeout_ms }))?;
            ctx.show(&env)?;
        }
        Cmd::Models => {
            for m in labely_printers::Hub::new().registry().models() {
                println!(
                    "{:<24} {:<12} {} dpi  {} dots  {}",
                    m.id, m.name, m.dpi, m.head_dots, m.task
                );
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("LABELY_LOG"))
        .with_writer(std::io::stderr)
        .init();
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
