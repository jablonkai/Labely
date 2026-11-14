//! Step lists and result references (`$N`, `$N.path`, `$last.path`, `$$` escape).
//!
//! Shared by `command.batch` and CLI scripts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::document::ElementId;

/// One step: a command id and its params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Command id, e.g. `element.add_text`.
    pub id: String,
    /// Params object. A string that is exactly `$N`, `$N.path` or `$last.path` is replaced by a
    /// value from an earlier step's result (`$0.id`, `$1.created.0`); `$$…` is a literal `$…`.
    #[serde(default)]
    pub params: Value,
}

/// What an earlier step produced, as seen by references.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StepValue {
    pub result: Value,
    pub created: Vec<ElementId>,
    pub changed: Vec<ElementId>,
    pub warnings: Vec<String>,
}

impl StepValue {
    fn get(&self, key: &str) -> Option<Value> {
        if let Some(v) = self.result.get(key) {
            return Some(v.clone());
        }
        match key {
            "created" => serde_json::to_value(&self.created).ok(),
            "changed" => serde_json::to_value(&self.changed).ok(),
            "warnings" => serde_json::to_value(&self.warnings).ok(),
            "result" => Some(self.result.clone()),
            _ => None,
        }
    }
}

/// Whether a JSON value contains any reference.
pub fn has_refs(v: &Value) -> bool {
    match v {
        Value::String(s) => parse_ref(s).is_some(),
        Value::Array(a) => a.iter().any(has_refs),
        Value::Object(m) => m.values().any(has_refs),
        _ => false,
    }
}

enum Base {
    Index(usize),
    Last,
}

fn parse_ref(s: &str) -> Option<(Base, Vec<&str>)> {
    let rest = s.strip_prefix('$')?;
    if rest.starts_with('$') {
        return None;
    }
    let mut parts = rest.split('.');
    let head = parts.next()?;
    let base = if head == "last" {
        Base::Last
    } else if !head.is_empty() && head.bytes().all(|b| b.is_ascii_digit()) {
        Base::Index(head.parse().ok()?)
    } else {
        return None;
    };
    let path: Vec<&str> = parts.collect();
    if path.iter().any(|p| p.is_empty()) {
        return None;
    }
    Some((base, path))
}

/// Replace references in `params` using results of steps `0..current` (`None` = failed step).
pub fn resolve(
    params: &Value,
    current: usize,
    earlier: &[Option<StepValue>],
) -> Result<Value, String> {
    match params {
        Value::String(s) => {
            if let Some(lit) = s.strip_prefix("$$") {
                return Ok(Value::String(format!("${lit}")));
            }
            match parse_ref(s) {
                Some((base, path)) => resolve_one(s, base, &path, current, earlier),
                None => Ok(params.clone()),
            }
        }
        Value::Array(a) => a
            .iter()
            .map(|v| resolve(v, current, earlier))
            .collect::<Result<_, _>>()
            .map(Value::Array),
        Value::Object(m) => {
            let mut out = Map::new();
            for (k, v) in m {
                out.insert(k.clone(), resolve(v, current, earlier)?);
            }
            Ok(Value::Object(out))
        }
        _ => Ok(params.clone()),
    }
}

fn resolve_one(
    text: &str,
    base: Base,
    path: &[&str],
    current: usize,
    earlier: &[Option<StepValue>],
) -> Result<Value, String> {
    let index = match base {
        Base::Index(i) => i,
        Base::Last => current
            .checked_sub(1)
            .ok_or_else(|| format!("unresolved reference `{text}`: there is no earlier step"))?,
    };
    if index >= current {
        return Err(format!(
            "unresolved reference `{text}`: step {index} has not run yet (refs may only point to earlier steps)"
        ));
    }
    let step = earlier
        .get(index)
        .and_then(Option::as_ref)
        .ok_or_else(|| format!("unresolved reference `{text}`: step {index} failed"))?;
    let Some((first, rest)) = path.split_first() else {
        return Ok(step.result.clone());
    };
    let mut cur = step
        .get(first)
        .ok_or_else(|| format!("unresolved reference `{text}`: step {index} has no `{first}`"))?;
    for seg in rest {
        let next = match &cur {
            Value::Object(m) => m.get(*seg).cloned(),
            Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get(i).cloned()),
            _ => None,
        };
        cur = next.ok_or_else(|| format!("unresolved reference `{text}`: no `{seg}`"))?;
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sv(result: Value, created: &[u64]) -> Option<StepValue> {
        Some(StepValue {
            result,
            created: created.iter().map(|&i| ElementId(i)).collect(),
            ..Default::default()
        })
    }

    #[test]
    fn refs() {
        let earlier = vec![
            sv(json!({"id": 7}), &[7]),
            sv(json!({"a": [1, {"b": "x"}]}), &[8, 9]),
        ];
        let p = json!({
            "id": "$0.id", "two": "$1.created.1", "deep": "$1.a.1.b", "last": "$last.a.0",
            "whole": "$0", "lit": "$$0.id", "plain": "$ 5", "arr": ["$0.id"]
        });
        let r = resolve(&p, 2, &earlier).unwrap();
        assert_eq!(
            r,
            json!({"id": 7, "two": 9, "deep": "x", "last": 1, "whole": {"id": 7},
                   "lit": "$0.id", "plain": "$ 5", "arr": [7]})
        );
        assert!(has_refs(&p));
        assert!(!has_refs(&json!({"x": "$$1"})));
    }

    #[test]
    fn unresolved() {
        let earlier = vec![sv(json!({"id": 7}), &[]), None];
        assert!(
            resolve(&json!("$0.nope"), 2, &earlier)
                .unwrap_err()
                .contains("no `nope`")
        );
        assert!(
            resolve(&json!("$1.id"), 2, &earlier)
                .unwrap_err()
                .contains("failed")
        );
        assert!(
            resolve(&json!("$5"), 2, &earlier)
                .unwrap_err()
                .contains("not run yet")
        );
        assert!(resolve(&json!("$last.id"), 0, &[]).is_err());
    }
}
