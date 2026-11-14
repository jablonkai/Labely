//! The result envelope returned by [`Session::execute`](crate::Session::execute).

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::command::{CommandError, ErrorCode};
use crate::document::ElementId;

/// `{ok:true,result,changed,created,warnings}` or `{ok:false,error}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Envelope {
    pub ok: bool,
    /// Command output; `None` on failure or when `terse` was requested.
    #[serde(default)]
    pub result: Option<Value>,
    /// Elements modified or removed.
    #[serde(default)]
    pub changed: Vec<ElementId>,
    /// Elements added.
    #[serde(default)]
    pub created: Vec<ElementId>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub error: Option<CommandError>,
}

impl Envelope {
    pub fn success(
        result: Option<Value>,
        changed: Vec<ElementId>,
        created: Vec<ElementId>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            ok: true,
            result,
            changed,
            created,
            warnings,
            error: None,
        }
    }

    pub fn failure(error: CommandError) -> Self {
        Self {
            ok: false,
            result: None,
            changed: Vec::new(),
            created: Vec::new(),
            warnings: Vec::new(),
            error: Some(error),
        }
    }

    /// Error code on failure.
    pub fn code(&self) -> Option<ErrorCode> {
        self.error.as_ref().map(|e| e.code)
    }

    /// `result` or JSON null.
    pub fn result_or_null(&self) -> &Value {
        self.result.as_ref().unwrap_or(&Value::Null)
    }

    /// The envelope as JSON.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

impl Serialize for Envelope {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("ok", &self.ok)?;
        if self.ok {
            if let Some(r) = &self.result {
                m.serialize_entry("result", r)?;
            }
            m.serialize_entry("changed", &self.changed)?;
            m.serialize_entry("created", &self.created)?;
            m.serialize_entry("warnings", &self.warnings)?;
        } else if let Some(e) = &self.error {
            m.serialize_entry("error", e)?;
        }
        m.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes() {
        let ok = Envelope::success(
            Some(serde_json::json!({"id": 1})),
            vec![],
            vec![ElementId(1)],
            vec![],
        );
        assert_eq!(
            ok.to_value(),
            serde_json::json!({"ok":true,"result":{"id":1},"changed":[],"created":[1],"warnings":[]})
        );
        let err = Envelope::failure(CommandError::bad_params("nope").with_param("x_mm"));
        assert_eq!(
            err.to_value(),
            serde_json::json!({"ok":false,"error":{"code":"bad_params","message":"nope","param":"x_mm"}})
        );
        let back: Envelope = serde_json::from_value(err.to_value()).unwrap();
        assert_eq!(back, err);
    }
}
