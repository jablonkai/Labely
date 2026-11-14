//! `command.list`, `command.batch`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::API_VERSION;
use crate::command::{Command, CommandError, CommandMeta, Ctx};
use crate::document::ElementId;
use crate::registry::CommandInfo;
use crate::steps::{Step, StepValue, has_refs, resolve};

/// Maximum steps per batch.
pub const MAX_BATCH_STEPS: usize = 128;
/// Maximum total size of kept step results, bytes.
pub const MAX_BATCH_RESULT_BYTES: usize = 4 * 1024 * 1024;
const MAX_BATCH_DEPTH: u32 = 4;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// Only list ids starting with this prefix, e.g. `element.`.
    #[serde(default)]
    pub prefix: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListOutput {
    pub api_version: u32,
    pub commands: Vec<CommandInfo>,
    /// Params accepted by every command in addition to its own.
    pub envelope_params: BTreeMap<String, String>,
}

/// `command.list`
#[derive(Debug)]
pub struct CommandList;

impl Command for CommandList {
    const ID: &'static str = "command.list";
    type Params = ListParams;
    type Output = ListOutput;

    fn meta() -> CommandMeta {
        CommandMeta::query(
            "List commands",
            "List every command with its kind, description and JSON Schemas for params and \
             result. Optional `prefix` filters ids (e.g. `element.`).",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: ListParams) -> Result<ListOutput, CommandError> {
        let prefix = p.prefix.unwrap_or_default();
        let commands = ctx
            .session()
            .registry()
            .list()
            .into_iter()
            .filter(|c| c.id.starts_with(&prefix))
            .collect();
        let envelope_params = BTreeMap::from([
            (
                "terse".to_owned(),
                "boolean; true drops `result` from the envelope".to_owned(),
            ),
            (
                "coalesce".to_owned(),
                "string key; consecutive edits with the same key form one undo step".to_owned(),
            ),
        ]);
        Ok(ListOutput {
            api_version: API_VERSION,
            commands,
            envelope_params,
        })
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchParams {
    /// Steps to run in order (max 128).
    pub steps: Vec<Step>,
    /// All or nothing (default true): any failure restores the state from before the batch,
    /// and success is one undo step. `print.*`, `device.*`, `doc.open`, `doc.close`,
    /// `history.*` and nested batches are refused.
    #[serde(default = "default_true")]
    pub atomic: bool,
    /// Stop at the first failed step (default true; always true when atomic).
    #[serde(default = "default_true")]
    pub stop_on_error: bool,
    /// Undo step label (default `command.batch`).
    #[serde(default)]
    pub label: Option<String>,
    /// Omit per-step `result` from the reply.
    #[serde(default)]
    pub terse: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StepResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CommandError>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BatchOutput {
    /// Steps that succeeded.
    pub completed: usize,
    /// Steps that failed (only non-zero when `atomic` is false).
    pub failed: usize,
    pub results: Vec<StepResult>,
    /// Elements created by all steps.
    pub created: Vec<ElementId>,
}

/// Whether a command may not run inside an atomic batch.
pub fn refused_in_atomic(id: &str) -> bool {
    id.starts_with("print.")
        || id.starts_with("device.")
        || id.starts_with("history.")
        || matches!(id, "doc.open" | "doc.close" | "command.batch")
}

/// `command.batch`
#[derive(Debug)]
pub struct CommandBatch;

impl Command for CommandBatch {
    const ID: &'static str = "command.batch";
    type Params = BatchParams;
    type Output = BatchOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Run batch",
            "Run several commands as one. Params: steps [{id, params}], atomic (default true: all \
             or nothing, one undo step), stop_on_error (default true), label, terse. A param \
             string that is exactly `$N`, `$N.path` or `$last.path` takes a value from an earlier \
             step's result, e.g. `$0.id` or `$1.created.0`; `$$` escapes a literal `$`. Max 128 \
             steps.",
        )
        .journal(false)
        .undoable(false)
    }

    fn run(ctx: &mut Ctx<'_>, p: BatchParams) -> Result<BatchOutput, CommandError> {
        let n = p.steps.len();
        if n == 0 {
            return Err(CommandError::bad_params("`steps` is empty").with_param("steps"));
        }
        if n > MAX_BATCH_STEPS {
            return Err(CommandError::bad_params(format!(
                "{n} steps exceed the limit of {MAX_BATCH_STEPS}; split the batch"
            ))
            .with_param("steps"));
        }
        if ctx.session().batch_depth >= MAX_BATCH_DEPTH {
            return Err(CommandError::bad_params("batches are nested too deeply"));
        }
        let atomic = p.atomic;
        let registry = ctx.session().registry();

        // Validate before running anything.
        for (i, step) in p.steps.iter().enumerate() {
            let Some(entry) = registry.get(&step.id) else {
                let mut e = registry.unknown(&step.id);
                e.message = format!("step {i}: {}; nothing was changed", e.message);
                e.param = Some(format!("steps.{i}.id"));
                return Err(e);
            };
            if atomic && refused_in_atomic(&step.id) {
                return Err(CommandError::bad_params(format!(
                    "step {i} ({}) cannot run inside an atomic batch because it cannot be rolled \
                     back; nothing was changed",
                    step.id
                ))
                .with_param(format!("steps.{i}.id"))
                .with_hint("run it after the batch, or pass atomic:false"));
            }
            if atomic && !has_refs(&step.params) {
                let mut params = match &step.params {
                    Value::Null => Value::Object(Default::default()),
                    v => v.clone(),
                };
                if let Value::Object(m) = &mut params {
                    m.remove("coalesce");
                    if step.id != Self::ID {
                        m.remove("terse");
                    }
                }
                if let Err(e) = entry.validate(&params) {
                    return Err(step_failed(i, &step.id, e));
                }
            }
        }

        let before = ctx.session().state.clone();
        ctx.session().batch_depth += 1;

        let mut values: Vec<Option<StepValue>> = Vec::with_capacity(n);
        let mut results = Vec::with_capacity(n);
        let mut created = Vec::new();
        let mut completed = 0;
        let mut failed = 0;
        let mut bytes = 0usize;
        let mut over_limit = false;

        for (i, step) in p.steps.iter().enumerate() {
            let envelope = match resolve(&step.params, i, &values) {
                Ok(params) => ctx.session().execute(&step.id, params),
                Err(msg) => crate::Envelope::failure(
                    CommandError::bad_params(msg).with_param(format!("steps.{i}.params")),
                ),
            };
            for w in &envelope.warnings {
                ctx.warn(format!("step {i}: {w}"));
            }
            if envelope.ok {
                completed += 1;
                created.extend(envelope.created.iter().copied());
                let result = envelope.result.clone().unwrap_or(Value::Null);
                let size = serde_json::to_string(&result).map(|s| s.len()).unwrap_or(0);
                bytes += size;
                let keep = !p.terse && bytes <= MAX_BATCH_RESULT_BYTES;
                if bytes > MAX_BATCH_RESULT_BYTES && !over_limit {
                    over_limit = true;
                    ctx.warn(format!(
                        "results from step {i} on were dropped from the reply: over the 4 MiB limit"
                    ));
                }
                results.push(StepResult {
                    ok: true,
                    result: keep.then_some(result.clone()),
                    error: None,
                });
                values.push(Some(StepValue {
                    result,
                    created: envelope.created,
                    changed: envelope.changed,
                    warnings: envelope.warnings,
                }));
            } else {
                let error = envelope
                    .error
                    .unwrap_or_else(|| CommandError::internal("step failed without an error"));
                if atomic {
                    return Err(step_failed(i, &step.id, error));
                }
                failed += 1;
                ctx.warn(format!("step {i} ({}) failed: {}", step.id, error.message));
                results.push(StepResult {
                    ok: false,
                    result: None,
                    error: Some(error),
                });
                values.push(None);
                if p.stop_on_error {
                    break;
                }
            }
        }

        if atomic {
            let label = p.label.as_deref().unwrap_or(Self::ID);
            ctx.session().collapse_history(&before, label);
        }
        Ok(BatchOutput {
            completed,
            failed,
            results,
            created,
        })
    }
}

fn step_failed(i: usize, id: &str, mut e: CommandError) -> CommandError {
    e.message = format!("step {i} ({id}) failed: {}; nothing was changed", e.message);
    e
}
