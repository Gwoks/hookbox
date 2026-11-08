//! A rule's body conditions (jsonpath-lite `eq|neq|contains|exists`) and
//! state requirements (`eq|neq|exists|absent`), with the op parsed once at
//! compile time. The op stays a free string on the wire and in storage; an
//! unknown op compiles to `None` and never holds (as it always has).

use std::collections::BTreeMap;

use crate::models::{BodyCondition, BodyOp, StateOp, StateRequirement};
use crate::util::json::jsonpath_lite;

/// A compiled `body_conditions[]` entry.
#[derive(Debug, Clone)]
pub struct BodyPredicate {
    pub path: String,
    pub op: Option<BodyOp>,
    pub value: String,
}

impl From<BodyCondition> for BodyPredicate {
    fn from(c: BodyCondition) -> Self {
        BodyPredicate {
            op: c.op.parse().ok(),
            value: c.value.unwrap_or_default(),
            path: c.path,
        }
    }
}

impl BodyPredicate {
    /// Whether the condition holds against the raw request body.
    pub fn holds(&self, body: &str) -> bool {
        let got = jsonpath_lite(body, &self.path);
        let want = self.value.as_str();
        match self.op {
            Some(BodyOp::Exists) => got.is_some(),
            Some(BodyOp::Eq) => got.as_deref() == Some(want),
            Some(BodyOp::Neq) => got.as_deref() != Some(want),
            Some(BodyOp::Contains) => got.is_some_and(|g| g.contains(want)),
            None => false,
        }
    }
}

/// A compiled `state_requirements[]` entry.
#[derive(Debug, Clone)]
pub struct StatePredicate {
    pub key: String,
    pub op: Option<StateOp>,
    pub value: String,
}

impl From<StateRequirement> for StatePredicate {
    fn from(r: StateRequirement) -> Self {
        StatePredicate {
            op: r.op.parse().ok(),
            value: r.value.unwrap_or_default(),
            key: r.key,
        }
    }
}

impl StatePredicate {
    /// Whether the requirement holds against the endpoint state. Fails
    /// closed: against an empty (unreadable) state only `absent` / `neq`
    /// can hold.
    pub fn holds(&self, state: &BTreeMap<String, String>) -> bool {
        let got = state.get(&self.key);
        match self.op {
            Some(StateOp::Exists) => got.is_some(),
            Some(StateOp::Absent) => got.is_none(),
            Some(StateOp::Eq) => got == Some(&self.value),
            // neq holds if absent OR differs.
            Some(StateOp::Neq) => got != Some(&self.value),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(op: &str, value: Option<&str>) -> BodyPredicate {
        BodyPredicate::from(BodyCondition {
            path: "kind".into(),
            op: op.into(),
            value: value.map(str::to_string),
        })
    }

    fn state(op: &str, value: Option<&str>) -> StatePredicate {
        StatePredicate::from(StateRequirement {
            key: "k".into(),
            op: op.into(),
            value: value.map(str::to_string),
        })
    }

    #[test]
    fn body_ops() {
        let vip = r#"{"kind":"vip"}"#;
        assert!(body("eq", Some("vip")).holds(vip));
        assert!(!body("eq", Some("std")).holds(vip));
        assert!(body("neq", Some("std")).holds(vip));
        assert!(body("contains", Some("v")).holds(vip));
        assert!(body("exists", None).holds(vip));
        assert!(!body("exists", None).holds("{}"));
        assert!(
            !body("bogus", Some("vip")).holds(vip),
            "unknown op never holds"
        );
        assert!(
            !body("EQ", Some("vip")).holds(vip),
            "ops are case-sensitive"
        );
    }

    #[test]
    fn state_ops_fail_closed() {
        let empty = BTreeMap::new();
        let set: BTreeMap<String, String> = [("k".to_string(), "1".to_string())].into();
        assert!(state("eq", Some("1")).holds(&set));
        assert!(!state("eq", Some("1")).holds(&empty));
        assert!(state("neq", Some("2")).holds(&set));
        assert!(state("neq", Some("1")).holds(&empty));
        assert!(state("exists", None).holds(&set));
        assert!(state("absent", None).holds(&empty));
        assert!(!state("bogus", None).holds(&set));
    }
}
