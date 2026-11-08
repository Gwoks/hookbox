//! `PatchObject`: a typed reader over a PATCH body. A PATCH distinguishes an
//! ABSENT key (leave the column alone) from an explicit `null` (clear it),
//! which a plain `#[derive(Deserialize)]` struct cannot. Unknown keys are
//! ignored (no `deny_unknown_fields`), exactly as before.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::ApiError;

pub(crate) struct PatchObject<'a> {
    fields: &'a Map<String, Value>,
}

impl<'a> PatchObject<'a> {
    /// The body must be a JSON object.
    pub(crate) fn new(raw: &'a Value) -> Result<Self, ApiError> {
        raw.as_object()
            .map(|fields| PatchObject { fields })
            .ok_or_else(|| ApiError::validation("Request body must be a JSON object."))
    }

    /// A non-nullable field: absent → `None`; any value that does not
    /// deserialize as `T` (including `null`) → 422 `detail`.
    pub(crate) fn field<T: DeserializeOwned>(
        &self,
        key: &str,
        detail: &str,
    ) -> Result<Option<T>, ApiError> {
        self.fields
            .get(key)
            .map(|v| T::deserialize(v).map_err(|_| ApiError::validation(detail)))
            .transpose()
    }

    /// A nullable field: absent → `None`; `null` → `Some(None)` (clear);
    /// otherwise `Some(Some(T))`, or 422 `detail` when it is not a `T`.
    pub(crate) fn nullable<T: DeserializeOwned>(
        &self,
        key: &str,
        detail: &str,
    ) -> Result<Option<Option<T>>, ApiError> {
        match self.fields.get(key) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(v) => T::deserialize(v)
                .map(|t| Some(Some(t)))
                .map_err(|_| ApiError::validation(detail)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn non_object_body_is_rejected() {
        let err = PatchObject::new(&json!([1])).err().unwrap();
        assert_eq!(err.detail, "Request body must be a JSON object.");
    }

    #[test]
    fn field_absent_typed_and_mistyped() {
        let raw = json!({"n": 5, "s": "x", "f": 5.5, "z": null, "extra": true});
        let obj = PatchObject::new(&raw).unwrap();
        assert_eq!(obj.field::<i64>("missing", "bad").unwrap(), None);
        assert_eq!(obj.field::<i64>("n", "bad").unwrap(), Some(5));
        assert_eq!(obj.field::<String>("s", "bad").unwrap(), Some("x".into()));
        assert_eq!(obj.field::<i64>("f", "bad n").unwrap_err().detail, "bad n");
        assert_eq!(
            obj.field::<String>("n", "bad s").unwrap_err().detail,
            "bad s"
        );
        assert!(obj.field::<bool>("z", "null is not a bool").is_err());
    }

    #[test]
    fn nullable_distinguishes_absent_null_and_value() {
        let raw = json!({"z": null, "s": "x", "n": 5});
        let obj = PatchObject::new(&raw).unwrap();
        assert_eq!(obj.nullable::<String>("missing", "bad").unwrap(), None);
        assert_eq!(obj.nullable::<String>("z", "bad").unwrap(), Some(None));
        assert_eq!(
            obj.nullable::<String>("s", "bad").unwrap(),
            Some(Some("x".into()))
        );
        assert_eq!(
            obj.nullable::<String>("n", "nope").unwrap_err().detail,
            "nope"
        );
    }
}
