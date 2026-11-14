//! Labely MCP server (ADR-0005).
//!
//! [`LabelyMcp`] is an rmcp [`ServerHandler`] over an [`Executor`], the thing that runs
//! commands. A headless server wraps an in-process [`Session`] ([`LocalExecutor`]). The studio
//! passes an executor that forwards to its UI thread. Transports:
//! - [`serve_stdio`]: stdio, for `labely mcp`;
//! - [`http::serve`]: Streamable HTTP on loopback with a bearer token, for the studio;
//! - [`bridge::run`]: stdio that forwards to a running studio.
//!
//! Safety (file roots, print gating) is the session [`Policy`](labely_core::Policy), so it
//! also holds inside batches.

pub mod bridge;
pub mod connection;
pub mod http;
mod tools;

use std::sync::{Arc, Mutex, PoisonError};

use labely_core::{CommandError, Envelope, Session};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::{Map, Value};

pub use tools::{TOOLS, ToolSpec};

/// Runs commands. Implementations block; the server calls them off the async runtime.
pub trait Executor: Send + Sync + 'static {
    fn execute(&self, id: &str, params: Value) -> Envelope;
}

/// Executor over an in-process session (headless mode, tests).
#[derive(Debug)]
pub struct LocalExecutor {
    session: Mutex<Session>,
}

impl LocalExecutor {
    pub fn new(session: Session) -> Self {
        Self {
            session: Mutex::new(session),
        }
    }

    /// Run `f` with the session locked.
    pub fn with_session<R>(&self, f: impl FnOnce(&mut Session) -> R) -> R {
        let mut s = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut s)
    }
}

impl Executor for LocalExecutor {
    fn execute(&self, id: &str, params: Value) -> Envelope {
        self.with_session(|s| s.execute(id, params))
    }
}

/// Text sent to clients in `initialize`.
pub const INSTRUCTIONS: &str = "Labely designs and prints thermal labels. Units are mm. \
Typical flow: document_new {width_mm, height_mm, printer_model?} -> command_batch with \
element.add_* steps (later steps can use earlier results, e.g. \"$0.id\") -> render_preview \
(PNG of the exact print raster) -> check (fix every error) -> print {address}. \
command_list returns every command id with its JSON schemas; command_run runs any of them. \
Every result is an envelope {ok, result, changed, created, warnings} or {ok:false, error:{code, \
message, param?, accepted?, hint?}}; follow the hint. Printing is refused (code forbidden) until \
the user allows agent printing.";

/// The MCP handler.
#[derive(Clone)]
pub struct LabelyMcp {
    exec: Arc<dyn Executor>,
}

impl std::fmt::Debug for LabelyMcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LabelyMcp").finish_non_exhaustive()
    }
}

impl LabelyMcp {
    pub fn new(exec: Arc<dyn Executor>) -> Self {
        Self { exec }
    }

    /// Run a command off the async runtime.
    async fn run(&self, id: String, params: Value) -> Envelope {
        let exec = self.exec.clone();
        tokio::task::spawn_blocking(move || exec.execute(&id, params))
            .await
            .unwrap_or_else(|e| {
                Envelope::failure(CommandError::internal(format!("executor failed: {e}")))
            })
    }

    /// Execute a tool call and build its result (also used by tests without a transport).
    pub async fn call(&self, name: &str, args: Map<String, Value>) -> CallToolResult {
        let Some(spec) = TOOLS.iter().find(|t| t.name == name) else {
            let names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
            return error_result(&Envelope::failure(
                CommandError::new(
                    labely_core::ErrorCode::UnknownCommand,
                    format!("unknown tool `{name}`"),
                )
                .with_accepted(names),
            ));
        };
        let (id, params) = match tools::to_command(spec, args) {
            Ok(v) => v,
            Err(e) => return error_result(&Envelope::failure(e)),
        };
        let env = self.run(id, params).await;
        tools::to_result(spec, env)
    }
}

fn error_result(env: &Envelope) -> CallToolResult {
    let v = env.to_value();
    let mut r = CallToolResult::error(vec![ContentBlock::text(v.to_string())]);
    r.structured_content = Some(v);
    r
}

impl LabelyMcp {
    /// Server info shared by the handler and the bridge.
    pub fn info_template() -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("labely", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

impl ServerHandler for LabelyMcp {
    fn get_info(&self) -> ServerConfig {
        Self::info_template()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tool_list()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tool_list().into_iter().find(|t| t.name == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let args = request.arguments.unwrap_or_default();
        Ok(self.call(&request.name, args).await.into())
    }
}

/// Every tool with its schema and hints.
pub fn tool_list() -> Vec<Tool> {
    TOOLS.iter().map(tools::tool).collect()
}

/// Serve `server` on stdin/stdout until the client disconnects.
pub async fn serve_stdio(server: LabelyMcp) -> Result<(), McpError> {
    let running = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    running
        .waiting()
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("MCP transport: {0}")]
    Transport(String),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("connection file: {0}")]
    Connection(String),
}
