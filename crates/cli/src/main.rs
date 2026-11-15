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
        /// Write the session journal (a replayable script) to this file.
        #[arg(long)]
        journal: Option<PathBuf>,
    },
    /// MCP server on stdio. Forwards to a running studio when one is listening (bridge),
    /// otherwise serves an in-process session (headless).
    Mcp {
        /// Always serve in-process, even if a studio is running.
        #[arg(long, conflicts_with = "connect")]
        headless: bool,
        /// Only bridge to a running studio; fail if none is listening.
        #[arg(long)]
        connect: bool,
        /// Headless: let agents print (otherwise print.* is refused).
        #[arg(long)]
        allow_print: bool,
        /// Headless: directory agents may read and write (repeatable; default: current dir).
        #[arg(long = "root")]
        roots: Vec<PathBuf>,
        /// Studio connection file (default: <app-data>/mcp-connection.json).
        #[arg(long)]
        connection: Option<PathBuf>,
    },
    /// Render a document to PNG.
    Render {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dpi: Option<u32>,
        /// Data record to render, 1-based.
        #[arg(long)]
        record: Option<usize>,
        /// Attach this CSV or spreadsheet first (.csv, .tsv, .xlsx, .xls, .ods).
        #[arg(long)]
        data: Option<PathBuf>,
    },
    /// Export labels to a PDF (one page per label) or a PNG, by the output extension.
    Export {
        file: PathBuf,
        /// `.pdf` or `.png` file to write.
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dpi: Option<u32>,
        /// Attach this CSV or spreadsheet first (.csv, .tsv, .xlsx, .xls, .ods).
        #[arg(long)]
        data: Option<PathBuf>,
        /// PDF: records to export, 1-based (`all` by default). PNG: the one record (`3`).
        #[arg(long)]
        records: Option<String>,
        /// PDF without data: number of labels (serials advance per label).
        #[arg(long)]
        count: Option<u32>,
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
        /// Attach this CSV or spreadsheet first (.csv, .tsv, .xlsx, .xls, .ods).
        #[arg(long)]
        data: Option<PathBuf>,
        /// Records to print, 1-based: `all` (default), `3`, `1,4,6-9`.
        #[arg(long)]
        records: Option<String>,
        /// Labels to print when there is no data (serials advance per label).
        #[arg(long)]
        count: Option<u32>,
        /// Column holding each record's copy count.
        #[arg(long)]
        copies_column: Option<String>,
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
    i18n: labely_i18n::I18n,
}

impl Ctx {
    /// Run a command; return its result or fail with the envelope's error.
    fn exec(&mut self, id: &str, params: Value) -> Result<Value> {
        let env = serde_json::to_value(self.session.execute(id, params))?;
        if env["ok"].as_bool() == Some(true) {
            Ok(env)
        } else {
            let e = &env["error"];
            let code = e["code"].as_str().unwrap_or("internal");
            let mut msg = format!(
                "{} [{code}] ({id}): {}",
                self.i18n.error_title(code),
                e["message"].as_str().unwrap_or("")
            );
            if let Some(h) = e["hint"].as_str() {
                msg.push_str(&format!("\n{}: {h}", self.i18n.t("cli-hint")));
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

    /// Attach a CSV (by extension) or spreadsheet as the document's data.
    fn attach(&mut self, path: &Path) -> Result<()> {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let id = if matches!(ext.as_str(), "xlsx" | "xlsm" | "xls" | "ods") {
            "data.attach_xlsx"
        } else {
            "data.attach_csv"
        };
        self.exec(id, json!({ "path": path }))
            .with_context(|| format!("attaching {}", path.display()))?;
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
                    "{}: {}",
                    self.i18n.t("cli-warning"),
                    w.as_str().map_or_else(|| w.to_string(), str::to_owned)
                );
            }
        }
        Ok(())
    }
}

fn run(cli: Cli) -> Result<()> {
    let mut session = Session::new(labely_printers::default_services());
    session.set_template_dir(Some(labely_mcp::connection::data_dir().join("templates")));
    session.set_history_dir(Some(labely_mcp::connection::data_dir().join("history")));
    session.set_paper_file(Some(labely_mcp::connection::data_dir().join("papers.json")));
    let mut ctx = Ctx {
        session,
        json: cli.json,
        i18n: labely_i18n::I18n::new(labely_i18n::Lang::detect()),
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
            journal,
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
            if let Some(out) = journal {
                let env = ctx.exec("history.journal", json!({}))?;
                let entries = &env["result"]["entries"];
                std::fs::write(&out, serde_json::to_vec_pretty(entries)?)
                    .with_context(|| format!("writing {}", out.display()))?;
            }
        }
        Cmd::Mcp {
            headless,
            connect,
            allow_print,
            roots,
            connection,
        } => mcp(headless, connect, allow_print, roots, connection)?,
        Cmd::Render {
            file,
            output,
            dpi,
            record,
            data,
        } => {
            ctx.open(&file)?;
            if let Some(d) = &data {
                ctx.attach(d)?;
            }
            let mut p = json!({});
            if let Some(d) = dpi {
                p["dpi"] = json!(d);
            }
            if let Some(r) = record {
                p["record"] = json!(r);
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
        Cmd::Export {
            file,
            output,
            dpi,
            data,
            records,
            count,
        } => {
            ctx.open(&file)?;
            if let Some(d) = &data {
                ctx.attach(d)?;
            }
            let pdf = output
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
            let mut p = json!({ "path": output });
            if let Some(d) = dpi {
                p["dpi"] = json!(d);
            }
            if pdf {
                if let Some(r) = records {
                    p["records"] = json!(r);
                }
                if let Some(c) = count {
                    p["count"] = json!(c);
                }
            } else if let Some(r) = records {
                let r: usize = r
                    .trim()
                    .parse()
                    .context("PNG export takes one record number in --records")?;
                p["record"] = json!(r);
            }
            let env = ctx.exec(if pdf { "export.pdf" } else { "export.png" }, p)?;
            ctx.show(&env)?;
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
            data,
            records,
            count,
            copies_column,
        } => {
            ctx.open(&file)?;
            if let Some(d) = &data {
                ctx.attach(d)?;
            }
            ctx.exec("device.connect", json!({ "address": printer }))?;
            let mut p = json!({ "address": printer, "copies": copies });
            if let Some(d) = density {
                p["density"] = json!(d);
            }
            if let Some(r) = records {
                p["records"] = json!(r);
            }
            if let Some(c) = count {
                p["count"] = json!(c);
            }
            if let Some(c) = copies_column {
                p["copies_column"] = json!(c);
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

fn mcp(
    headless: bool,
    connect: bool,
    allow_print: bool,
    roots: Vec<PathBuf>,
    connection: Option<PathBuf>,
) -> Result<()> {
    use labely_mcp::{LabelyMcp, LocalExecutor, connection as conn};
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        if !headless {
            let path = connection.unwrap_or_else(conn::default_path);
            match conn::read(&path)? {
                Some(info) => match labely_mcp::bridge::run(&info).await {
                    Ok(()) => return Ok(()),
                    Err(e) if connect => return Err(e.into()),
                    Err(e) => {
                        let i18n = labely_i18n::I18n::new(labely_i18n::Lang::detect());
                        let err = e.to_string();
                        let text = i18n.t_args(
                            "cli-mcp-fallback",
                            &[("error", labely_i18n::Arg::Str(&err))],
                        );
                        eprintln!("labely mcp: {text}");
                    }
                },
                None if connect => bail!(
                    "no running studio: {} does not exist (start the studio with MCP enabled)",
                    path.display()
                ),
                None => {}
            }
        }
        let roots = if roots.is_empty() {
            vec![std::env::current_dir()?]
        } else {
            roots
        };
        let mut policy = labely_core::Policy::restricted(roots);
        policy.allow_print = allow_print;
        let mut session = Session::with_policy(labely_printers::default_services(), policy);
        session.set_template_dir(Some(conn::data_dir().join("templates")));
        session.set_history_dir(Some(conn::data_dir().join("history")));
        session.set_paper_file(Some(conn::data_dir().join("papers.json")));
        let server = LabelyMcp::new(std::sync::Arc::new(LocalExecutor::new(session)));
        labely_mcp::serve_stdio(server).await?;
        Ok(())
    })
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
