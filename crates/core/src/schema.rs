//! JSON Schema generation (schemars 1.x, draft 2020-12, `$ref`s inlined for strict MCP clients).

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::Value;

/// Schema of a params type, as accepted on deserialisation.
pub fn params_schema<T: JsonSchema>() -> Value {
    SchemaSettings::draft2020_12()
        .with(|s| s.inline_subschemas = true)
        .for_deserialize()
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value()
}

/// Schema of an output type, as produced on serialisation.
pub fn output_schema<T: JsonSchema>() -> Value {
    SchemaSettings::draft2020_12()
        .with(|s| s.inline_subschemas = true)
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value()
}

/// Top-level property names of a schema (used to validate partial element props).
pub(crate) fn property_names<T: JsonSchema>() -> Vec<String> {
    params_schema::<T>()
        .get("properties")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_ref(v: &Value) -> bool {
        match v {
            Value::Object(m) => m.contains_key("$ref") || m.values().any(has_ref),
            Value::Array(a) => a.iter().any(has_ref),
            _ => false,
        }
    }

    #[test]
    fn document_schema_is_inlined() {
        let s = output_schema::<crate::Document>();
        assert!(!has_ref(&s), "{s}");
        assert!(s.get("$defs").is_none());
    }
}
