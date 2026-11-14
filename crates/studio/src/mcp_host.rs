//! The studio's MCP server: Streamable HTTP on loopback; calls reach the UI thread over a
//! channel with a deadline (ADR-0005).

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::time::{Duration, Instant};

use labely_core::{CommandError, Envelope};
use labely_mcp::connection::{self, ConnectionInfo};
use labely_mcp::http::HttpServer;
use labely_mcp::{Executor, LabelyMcp};
use serde_json::Value;

/// How long an agent call may wait for the UI thread.
pub const DEADLINE: Duration = Duration::from_secs(30);

/// One agent command waiting for the UI thread.
#[derive(Debug)]
pub struct McpCall {
    pub id: String,
    pub params: Value,
    deadline: Instant,
    reply: SyncSender<Envelope>,
}

impl McpCall {
    pub fn expired(&self) -> bool {
        Instant::now() > self.deadline
    }

    pub fn reply(self, env: Envelope) {
        // The caller may have timed out already.
        let _ = self.reply.send(env);
    }
}

struct StudioExecutor {
    tx: std::sync::Mutex<Sender<McpCall>>,
    ctx: egui::Context,
}

impl Executor for StudioExecutor {
    fn execute(&self, id: &str, params: Value) -> Envelope {
        let (reply, rx) = sync_channel(1);
        let call = McpCall {
            id: id.to_owned(),
            params,
            deadline: Instant::now() + DEADLINE,
            reply,
        };
        let sent = self
            .tx
            .lock()
            .map(|tx| tx.send(call).is_ok())
            .unwrap_or(false);
        if !sent {
            return Envelope::failure(CommandError::internal("the studio is shutting down"));
        }
        self.ctx.request_repaint();
        rx.recv_timeout(DEADLINE).unwrap_or_else(|_| {
            Envelope::failure(
                CommandError::new(
                    labely_core::ErrorCode::Cancelled,
                    "the studio did not answer in time",
                )
                .with_hint("the user may be dragging or in a dialog; retry shortly"),
            )
        })
    }
}

/// Running MCP server plus its inbox.
pub struct McpHost {
    pub inbox: Receiver<McpCall>,
    pub url: String,
    pub conn_path: PathBuf,
    server: HttpServer,
    _rt: tokio::runtime::Runtime,
}

impl std::fmt::Debug for McpHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpHost").field("url", &self.url).finish()
    }
}

impl McpHost {
    /// Serve on `127.0.0.1:port` (any free port if taken) and write the connection file.
    pub fn start(ctx: egui::Context, port: u16, conn_path: PathBuf) -> Result<Self, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let (tx, inbox) = channel();
        let exec = StudioExecutor {
            tx: std::sync::Mutex::new(tx),
            ctx,
        };
        let token = connection::new_token().map_err(|e| e.to_string())?;
        let server = LabelyMcp::new(std::sync::Arc::new(exec));
        let srv = rt.block_on(async {
            match labely_mcp::http::serve(server.clone(), port, token.clone()).await {
                Ok(s) => Ok(s),
                Err(_) if port != 0 => labely_mcp::http::serve(server, 0, token.clone()).await,
                Err(e) => Err(e),
            }
        });
        let server = srv.map_err(|e| e.to_string())?;
        connection::write(
            &conn_path,
            &ConnectionInfo {
                url: server.url.clone(),
                token,
                pid: std::process::id(),
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            inbox,
            url: server.url.clone(),
            conn_path,
            server,
            _rt: rt,
        })
    }
}

impl Drop for McpHost {
    fn drop(&mut self) {
        self.server.shutdown();
        connection::remove_own(&self.conn_path);
    }
}
