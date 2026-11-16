//! UniFFI bindings of the Labely engine for the Android and iOS shells (ADR-0025).
//!
//! - [`LabelyEngine`] wraps one [`Session`]. Every edit goes through
//!   [`LabelyEngine::execute`] with the same command ids and JSON envelopes as the CLI and
//!   MCP, so the shells hold no editing logic.
//! - [`NativeBle`] and [`NativeBleLink`] are implemented by the shell (Kotlin
//!   `BluetoothGatt`, Swift CoreBluetooth). Framing, chunking and the printer protocol stay
//!   in Rust (ADR-0006).
//!
//! Calls block. The shells call them off the main thread.

// UniFFI's generated scaffolding uses `unsafe` for the C ABI.
#![allow(unsafe_code)]

mod ble;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use labely_core::services::RenderOptions;
use labely_core::{CommandError, Envelope, Policy, Session};
use serde_json::Value;

pub use ble::{BleDevice, BleError, BleNotifySink, NativeBle, NativeBleLink};

uniffi::setup_scaffolding!();

/// Engine settings chosen by the shell.
#[derive(Debug, Clone, uniffi::Record)]
pub struct EngineConfig {
    /// App-private directory for templates, print history and the paper library.
    pub data_dir: String,
    /// Extra directories file commands may read and write. `data_dir` is always allowed.
    pub file_roots: Vec<String>,
    /// Whether `print.*` commands may run (see [`LabelyEngine::set_allow_print`]).
    pub allow_print: bool,
}

/// Errors of the typed helpers. Commands report errors inside their envelope instead.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum EngineError {
    #[error("{code}: {detail}")]
    Command { code: String, detail: String },
}

impl From<CommandError> for EngineError {
    fn from(e: CommandError) -> Self {
        Self::Command {
            code: e.code.as_str().to_owned(),
            detail: e.message,
        }
    }
}

/// One editing session: open documents, undo history, print queue and printers.
#[derive(Debug, uniffi::Object)]
pub struct LabelyEngine {
    session: Mutex<Session>,
}

#[uniffi::export]
impl LabelyEngine {
    /// Create an engine. With `ble`, `ble:` printers go through the shell's BLE stack;
    /// without it only `fake:` and `sim:` printers work.
    #[uniffi::constructor]
    pub fn new(config: EngineConfig, ble: Option<Arc<dyn NativeBle>>) -> Arc<Self> {
        let services = match ble {
            Some(ble) => labely_printers::mobile_services(Arc::new(ble::Host::new(ble))),
            None => labely_printers::default_services(),
        };
        let data = PathBuf::from(&config.data_dir);
        let mut roots: Vec<PathBuf> = vec![data.clone()];
        roots.extend(config.file_roots.iter().map(PathBuf::from));
        let mut policy = Policy::restricted(roots);
        policy.allow_print = config.allow_print;
        let mut session = Session::with_policy(services, policy);
        session.set_template_dir(Some(data.join("templates")));
        session.set_history_dir(Some(data.join("history")));
        session.set_paper_file(Some(data.join("papers.json")));
        Arc::new(Self {
            session: Mutex::new(session),
        })
    }

    /// Run one command. `params_json` is a JSON object; the result is the JSON envelope
    /// (`{"ok": true, "result": …}` or `{"ok": false, "error": …}`), as over MCP.
    pub fn execute(&self, id: String, params_json: String) -> String {
        let env = match parse_params(&params_json) {
            Ok(params) => self.lock().execute(&id, params),
            Err(e) => Envelope::failure(e),
        };
        env.to_value().to_string()
    }

    /// The command catalogue (`command.list`) as JSON.
    pub fn commands_json(&self) -> String {
        self.execute("command.list".into(), "{}".into())
    }

    /// Command API version (`labely_core::API_VERSION`).
    pub fn api_version(&self) -> u32 {
        labely_core::API_VERSION
    }

    /// Allow or forbid printing, e.g. after the user's one-time consent for AppFunctions.
    pub fn set_allow_print(&self, allow: bool) {
        let mut s = self.lock();
        let mut policy = s.policy().clone();
        policy.allow_print = allow;
        s.set_policy(policy);
    }

    /// PNG of a page's exact 1-bit print raster, for on-screen display without base64.
    /// `doc_id` defaults to the active document.
    pub fn render_png(
        &self,
        doc_id: Option<String>,
        dpi: u32,
        page: u32,
    ) -> Result<Vec<u8>, EngineError> {
        let s = self.lock();
        let doc = s
            .doc_arc(doc_id.as_deref())
            .ok_or_else(CommandError::no_document)?;
        let mut opts = RenderOptions::at_dpi(dpi.clamp(50, 1200));
        opts.page = usize::try_from(page).unwrap_or(usize::MAX);
        let renderer = Arc::clone(&s.services().renderer);
        drop(s);
        let out = renderer
            .render(&doc, &opts)
            .map_err(|e| CommandError::bad_params(e.to_string()))?;
        Ok(renderer
            .to_png(&out.bitmap)
            .map_err(|e| CommandError::internal(e.to_string()))?)
    }
}

impl LabelyEngine {
    fn lock(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn parse_params(json: &str) -> Result<Value, CommandError> {
    if json.trim().is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    let v: Value = serde_json::from_str(json)
        .map_err(|e| CommandError::bad_params(format!("params are not valid JSON: {e}")))?;
    if !v.is_object() {
        return Err(CommandError::bad_params("params must be a JSON object"));
    }
    Ok(v)
}
