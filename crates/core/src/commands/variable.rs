//! `variable.set`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, CommandError, CommandMeta, Ctx};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetParams {
    #[serde(default)]
    pub doc_id: Option<String>,
    /// Variable name, used as `{{name}}` in text and code data.
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SetOutput {
    pub name: String,
    pub value: String,
    /// Previous value, if the variable existed.
    pub previous: Option<String>,
}

/// `variable.set`
#[derive(Debug)]
pub struct VariableSet;

impl Command for VariableSet {
    const ID: &'static str = "variable.set";
    type Params = SetParams;
    type Output = SetOutput;

    fn meta() -> CommandMeta {
        CommandMeta::edit(
            "Set variable",
            "Set a document variable; `{{name}}` in text, barcode and 2D-code data is replaced \
             by its value when rendering and printing.",
        )
    }

    fn run(ctx: &mut Ctx<'_>, p: SetParams) -> Result<SetOutput, CommandError> {
        let name = p.name.trim().to_owned();
        if name.is_empty() || name.contains("{{") || name.contains("}}") {
            return Err(CommandError::bad_params(
                "variable name must be non-empty and contain no braces",
            )
            .with_param("name"));
        }
        let doc = ctx.doc_mut(p.doc_id.as_deref())?;
        let previous = doc.variables.insert(name.clone(), p.value.clone());
        Ok(SetOutput {
            name,
            value: p.value,
            previous,
        })
    }
}
