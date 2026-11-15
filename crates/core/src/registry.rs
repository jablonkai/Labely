//! Type-erased command registry built from an explicit list (no `inventory`; FFI-safe).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::command::{Command, CommandError, CommandKind, CommandMeta, Ctx, ErrorCode};
use crate::commands::{
    arrange, data, decor, device, doc, element, export, history, images, jobs, meta, paper, render,
    selection, table, template, variable,
};
use crate::schema::{output_schema, params_schema};

type RunFn = fn(&mut Ctx<'_>, Value) -> Result<Value, CommandError>;
type ValidateFn = fn(&Value) -> Result<(), CommandError>;
type SchemaFn = fn() -> Value;

/// One registered command.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub id: &'static str,
    pub meta: CommandMeta,
    pub(crate) run: RunFn,
    pub(crate) validate: ValidateFn,
    params_schema: SchemaFn,
    output_schema: SchemaFn,
}

impl Entry {
    pub fn of<C: Command>() -> Self {
        Self {
            id: C::ID,
            meta: C::meta(),
            run: run_erased::<C>,
            validate: validate_erased::<C>,
            params_schema: params_schema::<C::Params>,
            output_schema: output_schema::<C::Output>,
        }
    }

    /// Place the command in a studio menu, optionally with a default shortcut.
    pub const fn in_menu(mut self, menu: &'static str, shortcut: Option<&'static str>) -> Self {
        self.meta.menu = Some(menu);
        self.meta.shortcut = shortcut;
        self
    }

    pub fn params_schema(&self) -> Value {
        (self.params_schema)()
    }

    pub fn output_schema(&self) -> Value {
        (self.output_schema)()
    }

    /// Check params shape without running.
    pub fn validate(&self, params: &Value) -> Result<(), CommandError> {
        (self.validate)(params)
    }
}

fn run_erased<C: Command>(ctx: &mut Ctx<'_>, params: Value) -> Result<Value, CommandError> {
    let p = parse_params::<C::Params>(C::ID, params)?;
    C::enabled(ctx)?;
    let out = C::run(ctx, p)?;
    serde_json::to_value(out)
        .map_err(|e| CommandError::internal(format!("{}: cannot serialise result: {e}", C::ID)))
}

fn validate_erased<C: Command>(params: &Value) -> Result<(), CommandError> {
    parse_params::<C::Params>(C::ID, params.clone()).map(|_| ())
}

/// Deserialize params; errors become `bad_params` with the param path when known.
pub fn parse_params<P: DeserializeOwned>(id: &str, params: Value) -> Result<P, CommandError> {
    serde_path_to_error::deserialize::<_, P>(params).map_err(|e| {
        let path = e.path().to_string();
        let inner = e.into_inner().to_string();
        serde_error(&format!("bad params for `{id}`"), &path, &inner)
            .with_hint(format!("see command.list for the `{id}` params schema"))
    })
}

/// Build a `bad_params` error from a serde path (`.` = root) and message.
pub(crate) fn serde_error(prefix: &str, path: &str, inner: &str) -> CommandError {
    let field = backticked_after(inner, "unknown field ")
        .or_else(|| backticked_after(inner, "missing field "));
    let param = match (path, field) {
        (".", Some(f)) => Some(f),
        (".", None) => None,
        (p, Some(f)) if inner.starts_with("missing field") => Some(format!("{p}.{f}")),
        (p, _) => Some(p.to_owned()),
    };
    let message = match &param {
        Some(p) => format!("{prefix}: `{p}`: {inner}"),
        None => format!("{prefix}: {inner}"),
    };
    let mut err = CommandError::bad_params(message);
    if let Some(p) = param {
        err = err.with_param(p);
    }
    if let Some(rest) = inner.split("expected one of ").nth(1) {
        let accepted: Vec<String> = rest
            .split(',')
            .map(|s| s.trim().trim_matches('`').to_owned())
            .collect();
        err = err.with_accepted(accepted);
    }
    err
}

fn backticked_after(s: &str, marker: &str) -> Option<String> {
    let rest = s.split(marker).nth(1)?.strip_prefix('`')?;
    rest.split('`').next().map(str::to_owned)
}

/// Listing entry returned by `command.list`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct CommandInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    pub kind: CommandKind,
    pub journal: bool,
    pub undoable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// JSON Schema (2020-12, refs inlined) of the params object.
    pub params_schema: Value,
    /// JSON Schema of `result`.
    pub output_schema: Value,
}

/// All commands, keyed by id.
#[derive(Debug)]
pub struct Registry {
    entries: Vec<Entry>,
    index: BTreeMap<&'static str, usize>,
}

impl Registry {
    pub fn new(entries: Vec<Entry>) -> Self {
        let index = entries.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
        Self { entries, index }
    }

    /// The built-in command set.
    pub fn builtin() -> &'static Registry {
        static REG: OnceLock<Registry> = OnceLock::new();
        REG.get_or_init(|| Registry::new(builtin_entries()))
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.index.get(id).map(|&i| &self.entries[i])
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|e| e.id)
    }

    /// Every command with its metadata and schemas, in registration order.
    pub fn list(&self) -> Vec<CommandInfo> {
        self.entries
            .iter()
            .map(|e| CommandInfo {
                id: e.id.to_owned(),
                label: e.meta.label.to_owned(),
                description: e.meta.description.to_owned(),
                kind: e.meta.kind,
                journal: e.meta.journal,
                undoable: e.meta.undoable,
                menu: e.meta.menu.map(str::to_owned),
                shortcut: e.meta.shortcut.map(str::to_owned),
                params_schema: e.params_schema(),
                output_schema: e.output_schema(),
            })
            .collect()
    }

    /// sha256 (hex) of every command's id, kind and schemas, with descriptions and titles
    /// removed. Pinned next to [`crate::API_VERSION`] by a test: a schema change needs a bump.
    pub fn schema_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        fn strip(v: &mut Value) {
            match v {
                Value::Object(m) => {
                    m.remove("description");
                    m.remove("title");
                    m.values_mut().for_each(strip);
                }
                Value::Array(a) => a.iter_mut().for_each(strip),
                _ => {}
            }
        }
        let mut all: Vec<Value> = self
            .entries
            .iter()
            .map(|e| {
                let mut v = serde_json::json!({
                    "id": e.id,
                    "kind": e.meta.kind,
                    "undoable": e.meta.undoable,
                    "params": e.params_schema(),
                    "output": e.output_schema(),
                });
                strip(&mut v);
                v
            })
            .collect();
        all.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let bytes = serde_json::to_vec(&all).unwrap_or_default();
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Up to five ids closest to `id` (edit distance, then shared area).
    pub fn suggest(&self, id: &str) -> Vec<&'static str> {
        let mut scored: Vec<(usize, &'static str)> = self
            .ids()
            .map(|c| {
                let d = levenshtein(id, c);
                let tail = c.split('.').nth(1).unwrap_or(c);
                let d = if !id.is_empty() && (c.contains(id) || id.contains(tail)) {
                    d.min(2)
                } else {
                    d
                };
                (d, c)
            })
            .collect();
        scored.sort();
        let limit = (id.len() / 2).max(3);
        let mut out: Vec<&'static str> = scored
            .iter()
            .filter(|(d, _)| *d <= limit)
            .take(5)
            .map(|(_, c)| *c)
            .collect();
        if out.is_empty() {
            let area = id.split('.').next().unwrap_or("");
            out = self
                .ids()
                .filter(|c| c.split('.').next() == Some(area))
                .take(5)
                .collect();
        }
        out
    }

    /// `unknown_command` error with nearest ids.
    pub fn unknown(&self, id: &str) -> CommandError {
        let near = self.suggest(id);
        let message = match near.first() {
            Some(first) => format!("unknown command `{id}`; did you mean `{first}`?"),
            None => format!("unknown command `{id}`"),
        };
        CommandError::new(ErrorCode::UnknownCommand, message)
            .with_accepted(near)
            .with_hint("run command.list for all command ids")
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != *cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The explicit command list. Order is the listing order.
fn builtin_entries() -> Vec<Entry> {
    vec![
        Entry::of::<meta::CommandList>(),
        Entry::of::<meta::CommandBatch>(),
        Entry::of::<doc::DocNew>().in_menu("file", Some("Ctrl+N")),
        Entry::of::<doc::DocOpen>().in_menu("file", Some("Ctrl+O")),
        Entry::of::<doc::DocSave>().in_menu("file", Some("Ctrl+S")),
        Entry::of::<doc::DocClose>().in_menu("file", Some("Ctrl+W")),
        Entry::of::<doc::DocList>(),
        Entry::of::<doc::DocActivate>(),
        Entry::of::<template::TemplateNew>().in_menu("file", Some("Ctrl+Shift+N")),
        Entry::of::<template::TemplateSave>().in_menu("file", None),
        Entry::of::<export::ExportPng>().in_menu("file", None),
        Entry::of::<export::ExportPdf>().in_menu("file", Some("Ctrl+E")),
        Entry::of::<template::TemplateList>(),
        Entry::of::<template::TemplateDelete>(),
        Entry::of::<template::TemplateMove>(),
        Entry::of::<template::TemplateDuplicate>(),
        Entry::of::<template::TemplateFolder>(),
        Entry::of::<doc::DocGet>(),
        Entry::of::<doc::DocSetLabel>().in_menu("label", None),
        Entry::of::<doc::DocRotateLabel>().in_menu("label", None),
        Entry::of::<paper::PaperList>().in_menu("label", None),
        Entry::of::<paper::PaperApply>(),
        Entry::of::<paper::PaperAdd>(),
        Entry::of::<paper::PaperDelete>(),
        Entry::of::<paper::PaperLink>(),
        Entry::of::<paper::PaperUnlink>(),
        Entry::of::<render::DocCheck>().in_menu("label", Some("Ctrl+K")),
        Entry::of::<element::AddText>().in_menu("insert", Some("T")),
        Entry::of::<element::AddBarcode>().in_menu("insert", Some("B")),
        Entry::of::<element::AddCode2d>().in_menu("insert", Some("Q")),
        Entry::of::<element::AddImage>().in_menu("insert", Some("I")),
        Entry::of::<element::AddRect>().in_menu("insert", Some("R")),
        Entry::of::<element::AddEllipse>().in_menu("insert", Some("E")),
        Entry::of::<element::AddLine>().in_menu("insert", Some("L")),
        Entry::of::<table::AddTable>().in_menu("insert", None),
        Entry::of::<decor::AddIcon>().in_menu("insert", None),
        Entry::of::<decor::AddBorder>().in_menu("insert", None),
        Entry::of::<table::TableEdit>(),
        Entry::of::<decor::IconList>(),
        Entry::of::<decor::FontList>(),
        Entry::of::<decor::CodeDecode>(),
        Entry::of::<element::SetProps>(),
        Entry::of::<element::Move>(),
        Entry::of::<element::Resize>(),
        Entry::of::<element::Rotate>(),
        Entry::of::<element::Delete>().in_menu("edit", Some("Delete")),
        Entry::of::<element::Duplicate>().in_menu("edit", Some("Ctrl+D")),
        Entry::of::<element::Mirror>().in_menu("arrange", None),
        Entry::of::<element::Paste>(),
        Entry::of::<element::Reorder>().in_menu("arrange", None),
        Entry::of::<arrange::Align>(),
        Entry::of::<arrange::Distribute>(),
        Entry::of::<arrange::Group>().in_menu("arrange", Some("Ctrl+G")),
        Entry::of::<arrange::Ungroup>().in_menu("arrange", Some("Ctrl+Shift+G")),
        Entry::of::<arrange::Lock>().in_menu("arrange", Some("Ctrl+L")),
        Entry::of::<history::Undo>().in_menu("edit", Some("Ctrl+Z")),
        Entry::of::<history::Redo>().in_menu("edit", Some("Ctrl+Shift+Z")),
        Entry::of::<history::Journal>(),
        Entry::of::<selection::SelectionSet>().in_menu("edit", None),
        Entry::of::<selection::SelectionGet>(),
        Entry::of::<variable::VariableSet>(),
        Entry::of::<data::AttachCsv>().in_menu("data", None),
        Entry::of::<data::AttachXlsx>().in_menu("data", None),
        Entry::of::<images::AttachImages>().in_menu("data", None),
        Entry::of::<data::Detach>().in_menu("data", None),
        Entry::of::<data::Get>(),
        Entry::of::<data::Bind>(),
        Entry::of::<data::PreviewRecord>(),
        Entry::of::<data::SetSerial>().in_menu("data", None),
        Entry::of::<data::RemoveSerial>(),
        Entry::of::<render::RenderPreview>().in_menu("view", None),
        Entry::of::<device::DeviceScan>().in_menu("print", None),
        Entry::of::<device::DeviceList>(),
        Entry::of::<device::DeviceConnect>(),
        Entry::of::<device::DeviceDisconnect>(),
        Entry::of::<device::DeviceStatusCmd>(),
        Entry::of::<paper::PaperDetect>().in_menu("print", None),
        Entry::of::<device::DeviceCalibrate>().in_menu("print", None),
        Entry::of::<device::DeviceSettingsCmd>(),
        Entry::of::<device::DeviceSetSettings>(),
        Entry::of::<device::PrintSubmit>().in_menu("print", Some("Ctrl+P")),
        Entry::of::<jobs::JobList>(),
        Entry::of::<jobs::JobGet>(),
        Entry::of::<jobs::JobWait>(),
        Entry::of::<jobs::JobCancel>(),
        Entry::of::<jobs::JobPause>(),
        Entry::of::<jobs::JobResume>(),
        Entry::of::<jobs::JobClear>(),
        Entry::of::<jobs::JobHistory>().in_menu("print", None),
        Entry::of::<jobs::JobHistoryDelete>(),
        Entry::of::<jobs::JobHistoryClear>(),
        Entry::of::<jobs::JobHistoryOpen>(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_unique_and_well_formed() {
        let r = Registry::builtin();
        assert_eq!(r.index.len(), r.entries.len());
        for id in r.ids() {
            let (area, verb) = id.split_once('.').unwrap();
            assert!(!area.is_empty() && !verb.is_empty());
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_' || c.is_ascii_digit()),
                "{id}"
            );
        }
    }

    #[test]
    fn suggestions() {
        let r = Registry::builtin();
        assert_eq!(
            r.suggest("element.add_txt").first(),
            Some(&"element.add_text")
        );
        assert!(r.suggest("doc.nwe").contains(&"doc.new"));
        assert!(r.suggest("undo").contains(&"history.undo"));
    }

    #[test]
    fn serde_error_paths() {
        let e = serde_error("p", ".", "unknown field `foo`, expected one of `a`, `b`");
        assert_eq!(e.param.as_deref(), Some("foo"));
        assert_eq!(e.accepted, Some(vec!["a".to_owned(), "b".to_owned()]));
        let e = serde_error("p", "props", "missing field `text`");
        assert_eq!(e.param.as_deref(), Some("props.text"));
    }
}
