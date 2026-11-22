//! Cloudflare Turnstile verification for the first anonymous mint
//! (`POST /api/session` without a valid cookie).
//!
//! `TurnstileVerifier::verify` sends the widget token to siteverify and
//! passes only on `success == true`, `action == "session_mint"` and a
//! `hostname` in the configured allowlist. Everything else — a missing or
//! oversized token (rejected before any network call), a non-2xx answer, a
//! network/timeout error, an unparseable body, any mismatch — fails closed
//! with `403 turnstile_failed`. The reason is logged at `warn`; the token and
//! the secret never are.

use std::net::IpAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{Config, Secret};
use crate::error::ApiError;

/// The widget `action` the SPA renders for the first mint.
pub const SESSION_MINT_ACTION: &str = "session_mint";

/// Longest token accepted (Cloudflare documents a 2048-character maximum).
const MAX_TOKEN_LEN: usize = 2048;

/// Upper bound on one siteverify round trip.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(10);

/// Siteverify request body (JSON is accepted alongside form encoding).
#[derive(Serialize)]
struct SiteverifyRequest<'a> {
    secret: &'a str,
    response: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    remoteip: Option<String>,
}

/// The siteverify fields the verdict depends on.
#[derive(Debug, Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

/// Verifies Turnstile tokens against siteverify. Built once at startup
/// (when a secret is configured) and shared through `AppState`.
#[derive(Debug)]
pub struct TurnstileVerifier {
    client: reqwest::Client,
    secret: Secret,
    /// Lowercased allowlist (see `Config::turnstile_hostnames`).
    hostnames: Vec<String>,
    verify_url: String,
}

impl TurnstileVerifier {
    /// The verifier for `cfg`, or `None` when no secret is configured
    /// (verification disabled).
    pub fn from_config(cfg: &Config) -> Option<Self> {
        let secret = cfg.turnstile_secret.clone()?;
        Some(TurnstileVerifier {
            client: http_client(),
            secret,
            hostnames: cfg.turnstile_hostnames.clone(),
            verify_url: cfg.turnstile_verify_url.clone(),
        })
    }

    /// `Ok(())` only for a token siteverify accepts for this app's action
    /// and hostnames; otherwise `403 turnstile_failed` (see module docs).
    pub async fn verify(&self, token: &str, remote_ip: Option<IpAddr>) -> Result<(), ApiError> {
        self.check(token, remote_ip).await.map_err(|reason| {
            tracing::warn!("turnstile verification failed: {reason}");
            ApiError::turnstile_failed()
        })
    }

    /// `verify` with the failure reason kept (for the log line only).
    async fn check(&self, token: &str, remote_ip: Option<IpAddr>) -> Result<(), String> {
        precheck(token)?;
        let resp = self.siteverify(token, remote_ip).await?;
        self.judge(&resp)
    }

    /// One siteverify round trip; `Err` carries a loggable reason.
    async fn siteverify(
        &self,
        token: &str,
        remote_ip: Option<IpAddr>,
    ) -> Result<SiteverifyResponse, String> {
        let body = SiteverifyRequest {
            secret: self.secret.expose(),
            response: token,
            remoteip: remote_ip.map(|ip| ip.to_string()),
        };
        let resp = self
            .client
            .post(&self.verify_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("siteverify unreachable ({})", e.without_url()))?;
        if !resp.status().is_success() {
            return Err(format!("siteverify returned HTTP {}", resp.status()));
        }
        resp.json::<SiteverifyResponse>()
            .await
            .map_err(|e| format!("siteverify body unparseable ({})", e.without_url()))
    }

    /// The verdict on a parsed siteverify answer.
    fn judge(&self, resp: &SiteverifyResponse) -> Result<(), String> {
        if !resp.success {
            return Err(format!("rejected, error-codes {:?}", resp.error_codes));
        }
        if resp.action.as_deref() != Some(SESSION_MINT_ACTION) {
            return Err(format!("unexpected action {:?}", resp.action));
        }
        let hostname = resp.hostname.as_deref().unwrap_or_default();
        if !self
            .hostnames
            .iter()
            .any(|h| h.eq_ignore_ascii_case(hostname))
        {
            return Err(format!("hostname {hostname:?} not allowed"));
        }
        Ok(())
    }
}

/// Reject without a network call: a token must be present and at most
/// `MAX_TOKEN_LEN` bytes.
fn precheck(token: &str) -> Result<(), String> {
    if token.is_empty() {
        Err("missing token".to_string())
    } else if token.len() > MAX_TOKEN_LEN {
        Err(format!("token longer than {MAX_TOKEN_LEN} bytes"))
    } else {
        Ok(())
    }
}

/// The shared siteverify client: bounded by `VERIFY_TIMEOUT`, and it never
/// follows redirects (the answer must come from the configured URL itself).
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(VERIFY_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        // Only fails if the TLS backend cannot initialise — the same
        // condition under which `reqwest::Client::new()` panics.
        .expect("build the Turnstile siteverify HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::config_from;

    fn verifier(hostnames: &str) -> TurnstileVerifier {
        let cfg = config_from(&[
            ("TURNSTILE_SECRET_KEY", "s"),
            ("TURNSTILE_HOSTNAMES", hostnames),
        ]);
        TurnstileVerifier::from_config(&cfg).expect("enabled")
    }

    fn answer(success: bool, action: &str, hostname: &str) -> SiteverifyResponse {
        SiteverifyResponse {
            success,
            action: Some(action.to_string()),
            hostname: Some(hostname.to_string()),
            error_codes: vec!["invalid-input-response".to_string()],
        }
    }

    #[test]
    fn disabled_without_a_secret() {
        assert!(TurnstileVerifier::from_config(&config_from(&[])).is_none());
    }

    #[test]
    fn precheck_bounds_the_token() {
        assert!(precheck("").is_err());
        assert!(precheck(&"t".repeat(MAX_TOKEN_LEN)).is_ok());
        assert!(precheck(&"t".repeat(MAX_TOKEN_LEN + 1)).is_err());
    }

    #[test]
    fn judge_requires_success_action_and_allowed_hostname() {
        let v = verifier("a.example, b.example");
        assert!(v.judge(&answer(true, "session_mint", "b.example")).is_ok());
        assert!(v.judge(&answer(true, "session_mint", "A.Example")).is_ok());
        assert!(v
            .judge(&answer(false, "session_mint", "a.example"))
            .is_err());
        assert!(v.judge(&answer(true, "login", "a.example")).is_err());
        assert!(v.judge(&answer(true, "session_mint", "c.example")).is_err());
        let bare = SiteverifyResponse {
            success: true,
            action: None,
            hostname: None,
            error_codes: Vec::new(),
        };
        assert!(v.judge(&bare).is_err());
    }

    #[test]
    fn verifier_debug_never_prints_the_secret() {
        let cfg = config_from(&[("TURNSTILE_SECRET_KEY", "hunter2-secret")]);
        let v = TurnstileVerifier::from_config(&cfg).unwrap();
        assert!(!format!("{v:?}").contains("hunter2-secret"));
    }

    #[test]
    fn request_omits_remoteip_when_unknown() {
        let body = SiteverifyRequest {
            secret: "s",
            response: "t",
            remoteip: None,
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json, serde_json::json!({"secret": "s", "response": "t"}));
    }
}
