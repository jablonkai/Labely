//! Streamable HTTP server for the studio: loopback only, `Origin` check, bearer token.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio_util::sync::CancellationToken;

use crate::{LabelyMcp, McpError};

/// A running HTTP server.
#[derive(Debug)]
pub struct HttpServer {
    pub addr: SocketAddr,
    /// `http://<addr>/mcp`
    pub url: String,
    cancel: CancellationToken,
}

impl HttpServer {
    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Bind `127.0.0.1:port` (0 = any free port) and serve on the current tokio runtime.
pub async fn serve(server: LabelyMcp, port: u16, token: String) -> Result<HttpServer, McpError> {
    let cancel = CancellationToken::new();
    let config = StreamableHttpServerConfig::default()
        .with_json_response(true)
        .with_cancellation_token(cancel.child_token());
    let service: StreamableHttpService<LabelyMcp, LocalSessionManager> =
        StreamableHttpService::new(move || Ok(server.clone()), Default::default(), config);
    let token = Arc::new(token);
    let app = axum::Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(token, guard));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let addr = listener.local_addr()?;
    let stop = cancel.clone();
    tokio::spawn(async move {
        let r = axum::serve(listener, app)
            .with_graceful_shutdown(async move { stop.cancelled_owned().await })
            .await;
        if let Err(e) = r {
            tracing::warn!("MCP HTTP server stopped: {e}");
        }
    });
    Ok(HttpServer {
        addr,
        url: format!("http://{addr}/mcp"),
        cancel,
    })
}

/// Reject non-local `Origin`s (DNS rebinding) and requests without the bearer token.
async fn guard(State(token): State<Arc<String>>, req: Request, next: Next) -> Response {
    if !origin_ok(req.headers()) {
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    let auth = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if !auth.is_some_and(|a| constant_time_eq(a.as_bytes(), token.as_bytes())) {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "missing or wrong bearer token; see mcp-connection.json",
        )
            .into_response();
    }
    next.run(req).await
}

fn origin_ok(h: &HeaderMap) -> bool {
    let Some(origin) = h.get(header::ORIGIN) else {
        return true;
    };
    let Ok(o) = origin.to_str() else {
        return false;
    };
    let Some(rest) = o
        .strip_prefix("http://")
        .or_else(|| o.strip_prefix("https://"))
    else {
        return false;
    };
    let host = if let Some(v6) = rest.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        rest.split(':').next().unwrap_or("")
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn origins() {
        let mut h = HeaderMap::new();
        assert!(origin_ok(&h));
        for (o, ok) in [
            ("http://localhost:3000", true),
            ("http://127.0.0.1", true),
            ("http://[::1]:8080", true),
            ("https://evil.example", false),
            ("http://localhost.evil.example", false),
            ("null", false),
        ] {
            h.insert(header::ORIGIN, HeaderValue::from_static(o));
            assert_eq!(origin_ok(&h), ok, "{o}");
        }
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
    }
}
