//! Bridge mode: `labely mcp` on stdio forwarding every tool call to the running studio, so an
//! agent edits the document the user sees.

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ListToolsResult, PaginatedRequestParams, ServerConfig,
    Tool,
};
use rmcp::service::{RequestContext, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{ErrorData, RoleClient, RoleServer, ServerHandler, ServiceExt};

use crate::connection::ConnectionInfo;
use crate::{LabelyMcp, McpError};

/// Stdio server whose tools are the studio's.
pub struct Bridge {
    client: RunningService<RoleClient, ()>,
    tools: Vec<Tool>,
    info: ServerConfig,
}

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge").finish_non_exhaustive()
    }
}

/// Connect to the studio described by `info`.
pub async fn connect(info: &ConnectionInfo) -> Result<Bridge, McpError> {
    let config = StreamableHttpClientTransportConfig::with_uri(info.url.clone())
        .auth_header(info.token.clone());
    let transport = StreamableHttpClientTransport::from_config(config);
    let client = ().serve(transport).await.map_err(|e| {
        McpError::Transport(format!("cannot reach the studio at {}: {e}", info.url))
    })?;
    let tools = client
        .list_all_tools()
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    let mut server = LabelyMcp::info_template();
    if let Some(peer) = client.peer_info() {
        server.instructions = peer.instructions.clone();
    }
    Ok(Bridge {
        client,
        tools,
        info: server,
    })
}

/// Connect, then serve stdio until the client disconnects.
pub async fn run(info: &ConnectionInfo) -> Result<(), McpError> {
    let bridge = connect(info).await?;
    let running = bridge
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    running
        .waiting()
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    Ok(())
}

impl ServerHandler for Bridge {
    fn get_info(&self) -> ServerConfig {
        self.info.clone()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools.clone()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.iter().find(|t| t.name == name).cloned()
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        self.client
            .call_tool(request)
            .await
            .map(Into::into)
            .map_err(|e| ErrorData::internal_error(format!("studio call failed: {e}"), None))
    }
}
