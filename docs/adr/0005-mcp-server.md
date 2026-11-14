# ADR-0005: MCP server design

- Status: proposed
- Date: 2026-11-14

## Decision
- **Crate.** `rmcp` 3.10 (official SDK; MCP spec 2026-07-28) in `labely-mcp`, using the features
  `transport-io`, `transport-streamable-http-server` and `macros`.
- **Studio server.** Streamable HTTP on `127.0.0.1:7878/mcp`.
  - Loopback bind only. Rejects requests whose `Origin` is not local.
  - Requires a bearer token from `<app-data>/mcp-connection.json` (mode 0600).
  - The user can turn it off.
- **`labely mcp` (stdio).** Bridges to the running studio. Without one it runs a headless
  in-process Session (`--headless` / `--connect` force either mode).
- **Tools.**
  - Generic: `command_list`, `command_run`, `command_batch`.
  - Curated: `document_get`, `document_new`, `document_open`, `document_save`, `render_preview`
    (PNG of the actual print raster, plus dots and dpi), `check`, `device_list`, `device_status`,
    `print`, `undo`, `redo`.
  - Hints: `readOnlyHint` and `destructiveHint` come from `CommandMeta.kind`.
- **Safety.**
  - Agents cannot print until the user allows it in the studio, or in headless mode with
    `--allow-print`.
  - File access is limited to the configured read and write roots.
- **Threading.** Studio calls reach the UI thread through a channel with a deadline. They are
  deferred during gestures and dialogs.

## Consequences
- One tool per command was rejected because it creates too many tools for clients.
  `command_list` exposes the full schemas instead.
- UNCERTAIN: rmcp may only reply over SSE instead of plain JSON. Clients cope either way. The
  fallback is a JSON-only endpoint behind the same handler.
