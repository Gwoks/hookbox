//! Typed PATCH bodies (§5.2 #5, #10). Each is parsed through
//! `validate::PatchObject` with the SAME field validators as the create
//! routes; fields are checked in the frozen order and the first failure is
//! the 422 (its `detail` string is part of the contract). `None` = the key
//! was absent; `Some(None)` on a nullable column = clear it.

use serde_json::Value;

use super::{MatchCriteria, ResponseSpec, StateWrite, WebhookAction};
use crate::config::Config;
use crate::error::ApiError;
use crate::validate::{
    self, clamp_to, validate_target_url, PatchObject, CHAOS_MODE_MSG, DEFAULT_MODE_MSG,
    NAME_TYPE_MSG,
};

/// `PATCH /api/endpoints/{token}`.
#[derive(Debug, Default)]
pub(crate) struct EndpointPatch {
    pub(crate) name: Option<Option<String>>,
    pub(crate) auto_crud: Option<bool>,
    pub(crate) target_url: Option<Option<String>>,
    pub(crate) default_mode: Option<String>,
    pub(crate) latency_ms: Option<i64>,
    pub(crate) rate_limit_per_min: Option<i64>,
    pub(crate) chaos_pct: Option<i64>,
    pub(crate) chaos_mode: Option<String>,
    pub(crate) cors_enabled: Option<bool>,
}

impl EndpointPatch {
    pub(crate) fn parse(raw: &Value, cfg: &Config) -> Result<Self, ApiError> {
        let obj = PatchObject::new(raw)?;
        let name = obj.nullable::<String>("name", NAME_TYPE_MSG)?;
        validate::endpoint_name(name.as_ref().and_then(|n| n.as_deref()))?;
        let auto_crud = obj.field("auto_crud", "auto_crud must be a boolean.")?;
        let target_url = obj
            .nullable::<String>("target_url", "target_url must be a string or null.")?
            .map(|url| validate_target_url(url.as_deref()).map_err(ApiError::validation))
            .transpose()?;
        let default_mode = obj.field::<String>("default_mode", DEFAULT_MODE_MSG)?;
        default_mode
            .as_deref()
            .map(validate::default_mode)
            .transpose()?;
        let int = |key: &str, detail: &str| obj.field::<i64>(key, detail);
        Ok(EndpointPatch {
            name,
            auto_crud,
            target_url,
            default_mode,
            latency_ms: clamp_to(
                int("latency_ms", "latency_ms must be an integer.")?,
                cfg.latency_max_ms,
            ),
            rate_limit_per_min: clamp_to(
                int(
                    "rate_limit_per_min",
                    "rate_limit_per_min must be an integer.",
                )?,
                cfg.rate_limit_max_per_min,
            ),
            chaos_pct: clamp_to(
                int("chaos_pct", "chaos_pct must be an integer.")?,
                cfg.chaos_max_pct,
            ),
            chaos_mode: endpoint_chaos_mode(&obj)?,
            cors_enabled: obj.field("cors_enabled", "cors_enabled must be a boolean.")?,
        })
    }
}

fn endpoint_chaos_mode(obj: &PatchObject<'_>) -> Result<Option<String>, ApiError> {
    let mode = obj.field::<String>("chaos_mode", CHAOS_MODE_MSG)?;
    mode.as_deref().map(validate::chaos_mode).transpose()?;
    Ok(mode)
}

/// `PATCH /api/endpoints/{token}/rules/{id}`.
#[derive(Debug, Default)]
pub(crate) struct RulePatch {
    pub(crate) name: Option<Option<String>>,
    pub(crate) priority: Option<i64>,
    pub(crate) enabled: Option<bool>,
    pub(crate) r#match: Option<MatchCriteria>,
    pub(crate) response: Option<ResponseSpec>,
    pub(crate) state_writes: Option<Vec<StateWrite>>,
    pub(crate) latency_ms: Option<Option<i64>>,
    pub(crate) rate_limit_per_min: Option<Option<i64>>,
    pub(crate) chaos_mode: Option<Option<String>>,
    pub(crate) webhook_action: Option<Option<WebhookAction>>,
}

impl RulePatch {
    pub(crate) fn parse(raw: &Value, cfg: &Config) -> Result<Self, ApiError> {
        let obj = PatchObject::new(raw)?;
        let name = obj.nullable::<String>("name", NAME_TYPE_MSG)?;
        validate::rule_name(name.as_ref().and_then(|n| n.as_deref()))?;
        let priority = obj.field::<i64>("priority", "priority must be an integer.")?;
        priority.map(validate::priority).transpose()?;
        let enabled = obj.field("enabled", "enabled must be a boolean.")?;
        let r#match = obj.field("match", "match is malformed.")?;
        let response = obj.field::<ResponseSpec>("response", "response is malformed.")?;
        if let Some(spec) = &response {
            validate::response_spec(spec, cfg.template_max_size)?;
        }
        let state_writes = obj.field("state_writes", "state_writes is malformed.")?;
        Ok(RulePatch {
            name,
            priority,
            enabled,
            r#match,
            response,
            state_writes,
            ..rule_limits(&obj, cfg)?
        })
    }
}

/// The per-rule overrides (each nullable = inherit the endpoint's value).
fn rule_limits(obj: &PatchObject<'_>, cfg: &Config) -> Result<RulePatch, ApiError> {
    let latency = obj.nullable::<i64>("latency_ms", "latency_ms must be an integer.")?;
    let rate = obj.nullable::<i64>(
        "rate_limit_per_min",
        "rate_limit_per_min must be an integer.",
    )?;
    let chaos_mode = obj.nullable::<String>("chaos_mode", CHAOS_MODE_MSG)?;
    if let Some(Some(mode)) = &chaos_mode {
        validate::chaos_mode(mode)?;
    }
    Ok(RulePatch {
        latency_ms: latency.map(|n| clamp_to(n, cfg.latency_max_ms)),
        rate_limit_per_min: rate.map(|n| clamp_to(n, cfg.rate_limit_max_per_min)),
        chaos_mode,
        webhook_action: obj.nullable("webhook_action", "webhook_action is malformed.")?,
        ..RulePatch::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::test_config;
    use serde_json::json;

    fn endpoint_err(raw: Value) -> String {
        EndpointPatch::parse(&raw, &test_config())
            .unwrap_err()
            .detail
    }

    fn rule_err(raw: Value) -> String {
        RulePatch::parse(&raw, &test_config()).unwrap_err().detail
    }

    #[test]
    fn endpoint_patch_types_every_field() {
        assert_eq!(endpoint_err(json!({"name": 5})), NAME_TYPE_MSG);
        assert_eq!(
            endpoint_err(json!({"target_url": 5})),
            "target_url must be a string or null."
        );
        assert_eq!(endpoint_err(json!({"default_mode": 1})), DEFAULT_MODE_MSG);
        assert_eq!(endpoint_err(json!({"chaos_mode": "x"})), CHAOS_MODE_MSG);
        // First failure wins, in field order.
        assert_eq!(
            endpoint_err(json!({"cors_enabled": 1, "auto_crud": "no"})),
            "auto_crud must be a boolean."
        );
    }

    #[test]
    fn endpoint_patch_clamps_clears_and_ignores_unknown_keys() {
        let raw = json!({"name": null, "target_url": "  ", "latency_ms": 999_999, "bogus": 1});
        let p = EndpointPatch::parse(&raw, &test_config()).unwrap();
        assert_eq!(p.name, Some(None));
        assert_eq!(p.target_url, Some(None));
        assert_eq!(p.latency_ms, Some(10_000));
        assert!(p.auto_crud.is_none() && p.chaos_mode.is_none());
    }

    #[test]
    fn rule_patch_matches_create_validation() {
        assert_eq!(rule_err(json!({"name": 7})), NAME_TYPE_MSG);
        assert_eq!(
            rule_err(json!({"name": "x".repeat(121)})),
            "name must be at most 120 characters."
        );
        let big = "x".repeat(test_config().template_max_size + 1);
        assert_eq!(
            rule_err(json!({"response": {"body_template": big}})),
            "response.body_template is too large."
        );
        assert_eq!(
            rule_err(json!({"webhook_action": {"url": 5}})),
            "webhook_action is malformed."
        );
        assert_eq!(rule_err(json!({"chaos_mode": 3})), CHAOS_MODE_MSG);
    }

    #[test]
    fn rule_patch_nullable_overrides() {
        let raw = json!({"latency_ms": null, "rate_limit_per_min": -4, "webhook_action": null});
        let p = RulePatch::parse(&raw, &test_config()).unwrap();
        assert_eq!(p.latency_ms, Some(None));
        assert_eq!(p.rate_limit_per_min, Some(Some(0)));
        assert!(matches!(p.webhook_action, Some(None)));
        assert!(p.name.is_none() && p.r#match.is_none());
    }
}
