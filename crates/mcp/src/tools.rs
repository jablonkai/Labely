//! The MCP tool table: three generic tools plus curated shortcuts onto single commands.

use std::sync::Arc;

use labely_core::{CommandError, CommandKind, Envelope, Registry};
use rmcp::model::{CallToolResult, ContentBlock, Tool, ToolAnnotations};
use serde_json::{Map, Value, json};

/// One MCP tool.
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub name: &'static str,
    pub title: &'static str,
    /// Command the tool runs; `None` for `command_run` (the id is an argument).
    pub command: Option<&'static str>,
    /// Extra text in front of the command's own description.
    pub summary: &'static str,
}

const fn t(
    name: &'static str,
    title: &'static str,
    command: Option<&'static str>,
    summary: &'static str,
) -> ToolSpec {
    ToolSpec {
        name,
        title,
        command,
        summary,
    }
}

/// All tools (ADR-0005), in listing order.
pub const TOOLS: &[ToolSpec] = &[
    t(
        "command_list",
        "List commands",
        Some("command.list"),
        "Every Labely command with its params and output JSON schemas. Start here.",
    ),
    t(
        "command_run",
        "Run command",
        None,
        "Run any command by id: {id, params}. Ids and schemas come from command_list.",
    ),
    t(
        "command_batch",
        "Run batch",
        Some("command.batch"),
        "Preferred way to build a label: many steps, atomic, one undo step.",
    ),
    t("document_new", "New document", Some("doc.new"), ""),
    t("document_open", "Open document", Some("doc.open"), ""),
    t("document_save", "Save document", Some("doc.save"), ""),
    t("document_get", "Get document", Some("doc.get"), ""),
    t(
        "render_preview",
        "Preview",
        Some("render.preview"),
        "Returns the print raster as an image/png content block.",
    ),
    t("check", "Check", Some("doc.check"), "Run before printing."),
    t("device_list", "List printers", Some("device.list"), ""),
    t("device_status", "Printer status", Some("device.status"), ""),
    t(
        "print",
        "Print",
        Some("print.submit"),
        "Needs the user's permission (forbidden until allowed).",
    ),
    t("undo", "Undo", Some("history.undo"), ""),
    t("redo", "Redo", Some("history.redo"), ""),
];

fn entry(cmd: &str) -> Option<&'static labely_core::registry::Entry> {
    Registry::builtin().get(cmd)
}

fn object(v: Value) -> Arc<Map<String, Value>> {
    let mut m = match v {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    m.remove("$schema");
    m.entry("type").or_insert_with(|| json!("object"));
    Arc::new(m)
}

/// The rmcp tool definition.
pub fn tool(spec: &ToolSpec) -> Tool {
    let (schema, description, kind) = match spec.command.and_then(entry) {
        Some(e) => {
            let d = if spec.summary.is_empty() {
                e.meta.description.to_owned()
            } else {
                format!("{} {}", spec.summary, e.meta.description)
            };
            (e.params_schema(), d, Some(e.meta.kind))
        }
        None => (
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Command id, e.g. element.add_text"},
                    "params": {"type": "object", "description": "Command params; see command_list"}
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            spec.summary.to_owned(),
            None,
        ),
    };
    let annotations = match kind {
        Some(CommandKind::Query) => ToolAnnotations::with_title(spec.title)
            .read_only(true)
            .destructive(false),
        Some(CommandKind::Edit | CommandKind::Ui) => ToolAnnotations::with_title(spec.title)
            .read_only(false)
            .destructive(false),
        Some(CommandKind::Device) => ToolAnnotations::with_title(spec.title)
            .read_only(false)
            .destructive(true)
            .open_world(true),
        None => ToolAnnotations::with_title(spec.title)
            .read_only(false)
            .destructive(true),
    };
    let mut tool = Tool::new(spec.name, description, object(schema)).with_annotations(annotations);
    tool.title = Some(spec.title.to_owned());
    tool
}

/// Map tool arguments onto `(command id, params)`.
pub fn to_command(
    spec: &ToolSpec,
    mut args: Map<String, Value>,
) -> Result<(String, Value), CommandError> {
    match spec.command {
        Some(cmd) => Ok((cmd.to_owned(), Value::Object(args))),
        None => {
            let id = match args.remove("id") {
                Some(Value::String(s)) => s,
                _ => {
                    return Err(CommandError::bad_params("`id` (string) is required")
                        .with_param("id")
                        .with_hint("call command_list for command ids"));
                }
            };
            let params = args.remove("params").unwrap_or_else(|| json!({}));
            if let Some(extra) = args.keys().next() {
                return Err(
                    CommandError::bad_params(format!("unknown argument `{extra}`"))
                        .with_param(extra.clone())
                        .with_accepted(["id", "params"]),
                );
            }
            Ok((id, params))
        }
    }
}

/// Build the tool result from an envelope. Previews become an image block.
pub fn to_result(spec: &ToolSpec, env: Envelope) -> CallToolResult {
    let mut v = env.to_value();
    let mut content = Vec::new();
    if env.ok
        && spec.name == "render_preview"
        && let Some(Value::String(png)) = v
            .get_mut("result")
            .and_then(Value::as_object_mut)
            .and_then(|r| r.remove("png_base64"))
    {
        content.push(ContentBlock::image(png, "image/png"));
    }
    content.insert(0, ContentBlock::text(v.to_string()));
    let mut r = if env.ok {
        CallToolResult::success(content)
    } else {
        CallToolResult::error(content)
    };
    r.structured_content = Some(v);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_maps_to_a_command() {
        for spec in TOOLS {
            if let Some(cmd) = spec.command {
                assert!(entry(cmd).is_some(), "{} -> {cmd}", spec.name);
            }
            let tool = tool(spec);
            assert_eq!(tool.input_schema.get("type"), Some(&json!("object")));
            assert!(tool.description.is_some_and(|d| !d.is_empty()));
        }
    }
}
