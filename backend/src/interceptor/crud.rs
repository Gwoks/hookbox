//! Auto-CRUD path routing — PORT of `app/interceptor/crud.py` (§5.5, AC-24..27).
//!
//! When `auto_crud` is on and no rule matched, the interceptor turns the endpoint
//! into a zero-config REST DB over `crud_collections`. The frozen lifecycle:
//!   POST /<coll>        -> 201 (server uuid id)
//!   GET  /<coll>        -> 200 array
//!   GET  /<coll>/<id>   -> 200 | 404
//!   PUT  /<coll>/<id>   -> 200 | 404
//!   PATCH/<coll>/<id>   -> 200 | 404
//!   DELETE /<coll>/<id> -> 204 | 404   (HEAD mirrors GET)
//!
//! Segments must match `^[A-Za-z0-9_-]{1,64}$`; a 3+-segment or unsafe path is NOT
//! CRUD (`matches` is false) → the engine falls through to tunnel/MITM/default.
//! A write body must be a JSON object (else 400); per-item byte cap → 400; the
//! collection item cap → 400. A SQLite fault surfaces as 503.

use axum::http::StatusCode;
use serde_json::Value;

use crate::config::Config;
use crate::error::{ApiError, ErrorCode};
use crate::repo::crud::{self as crud_repo, Mutation};
use crate::state::AppState;
use crate::validate::is_safe_key;

/// A CRUD outcome the engine renders into an HTTP response (served_by="crud").
pub struct CrudResponse {
    pub status: u16,
    /// `None` for a 204 (no body).
    pub body: Option<Value>,
}

impl CrudResponse {
    fn json(status: u16, body: Value) -> Self {
        CrudResponse {
            status,
            body: Some(body),
        }
    }
    fn no_content() -> Self {
        CrudResponse {
            status: 204,
            body: None,
        }
    }
    fn error(error: ApiError) -> Self {
        CrudResponse::json(error.status.as_u16(), error.body())
    }
    fn bad_request(detail: &str) -> Self {
        CrudResponse::error(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::BadRequest,
            detail,
        ))
    }
    fn not_found() -> Self {
        CrudResponse::error(ApiError::not_found("No such item."))
    }
}

/// Parse `/<coll>[/<id>]` into `(coll, id?)`; `None` when not a valid 1- or
/// 2-segment safe CRUD path.
pub fn parse_path(mock_path: &str) -> Option<(String, Option<String>)> {
    if mock_path.is_empty() || !mock_path.starts_with('/') {
        return None;
    }
    let trimmed = if mock_path.len() > 1 && mock_path.ends_with('/') {
        &mock_path[..mock_path.len() - 1]
    } else {
        mock_path
    };
    let segs: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    match segs.len() {
        1 if is_safe_key(segs[0]) => Some((segs[0].to_string(), None)),
        2 if is_safe_key(segs[0]) && is_safe_key(segs[1]) => {
            Some((segs[0].to_string(), Some(segs[1].to_string())))
        }
        _ => None,
    }
}

/// True iff `mock_path` looks like a CRUD collection/item path (AC-24).
pub fn matches(mock_path: &str) -> bool {
    parse_path(mock_path).is_some()
}

fn parse_object_body(
    body_text: &str,
    cfg: &Config,
) -> Result<serde_json::Map<String, Value>, String> {
    if body_text.trim().is_empty() {
        return Err("empty body".into());
    }
    if body_text.len() > cfg.crud_max_item_bytes {
        return Err("item too large".into());
    }
    let v: Value =
        serde_json::from_str(body_text).map_err(|_| "body is not valid JSON".to_string())?;
    match v {
        Value::Object(m) => Ok(m),
        _ => Err("body must be a JSON object".into()),
    }
}

fn find<'a>(items: &'a [Value], id: &str) -> Option<&'a Value> {
    items
        .iter()
        .find(|it| it.get("id").and_then(|v| v.as_str()) == Some(id))
}

/// The collection one Auto-CRUD call operates on.
struct Target<'a> {
    state: &'a AppState,
    token: &'a str,
    coll: &'a str,
}

/// Execute one Auto-CRUD operation. Returns a `CrudResponse`, or the
/// `sqlx::Error` of a SQLite fault (the engine maps it to 503
/// `store_unavailable`).
pub async fn handle(
    state: &AppState,
    token: &str,
    method: &str,
    mock_path: &str,
    body_text: &str,
) -> Result<CrudResponse, sqlx::Error> {
    let Some((coll, ident)) = parse_path(mock_path) else {
        return Ok(CrudResponse::not_found());
    };
    let method = method.to_ascii_uppercase();
    let target = Target {
        state,
        token,
        coll: &coll,
    };
    match ident {
        None => collection(&target, &method, body_text).await,
        Some(id) => item(&target, &method, &id, body_text).await,
    }
}

/// `/<coll>`: GET / HEAD lists, POST appends (server-assigned `id`).
async fn collection(
    t: &Target<'_>,
    method: &str,
    body_text: &str,
) -> Result<CrudResponse, sqlx::Error> {
    let (pool, cfg) = (&t.state.pool, &t.state.cfg);
    match method {
        "GET" | "HEAD" => {
            let items = crud_repo::list_items(pool, t.token, t.coll).await?;
            Ok(CrudResponse::json(200, Value::Array(items)))
        }
        "POST" => {
            let obj = match parse_object_body(body_text, cfg) {
                Ok(obj) => obj,
                Err(e) => return Ok(CrudResponse::bad_request(&e)),
            };
            let (max, ttl) = (cfg.crud_max_items, cfg.crud_ttl_seconds);
            let outcome = crud_repo::append_item(pool, t.token, t.coll, obj, max, ttl).await?;
            Ok(map_mutation(outcome, 201, "collection is full"))
        }
        _ => Ok(CrudResponse::bad_request(
            "an item id is required for this method",
        )),
    }
}

/// `/<coll>/<id>`: GET / HEAD read, PUT replaces, PATCH shallow-merges,
/// DELETE removes (204).
async fn item(
    t: &Target<'_>,
    method: &str,
    id: &str,
    body_text: &str,
) -> Result<CrudResponse, sqlx::Error> {
    let (pool, cfg) = (&t.state.pool, &t.state.cfg);
    let ttl = cfg.crud_ttl_seconds;
    if !matches!(method, "GET" | "HEAD" | "PUT" | "PATCH" | "DELETE") {
        return Ok(CrudResponse::bad_request("unsupported method for an item"));
    }
    if matches!(method, "GET" | "HEAD") {
        let items = crud_repo::list_items(pool, t.token, t.coll).await?;
        return Ok(match find(&items, id) {
            Some(item) => CrudResponse::json(200, item.clone()),
            None => CrudResponse::not_found(),
        });
    }
    if method == "DELETE" {
        let outcome = crud_repo::delete_item(pool, t.token, t.coll, id, ttl).await?;
        return Ok(match outcome {
            Mutation::Ok(..) => CrudResponse::no_content(),
            other => map_mutation(other, 204, "item too large"),
        });
    }
    let obj = match parse_object_body(body_text, cfg) {
        Ok(obj) => obj,
        Err(e) => return Ok(CrudResponse::bad_request(&e)),
    };
    let outcome = if method == "PUT" {
        crud_repo::replace_item(pool, t.token, t.coll, id, obj, ttl).await?
    } else {
        let max = cfg.crud_max_item_bytes;
        crud_repo::merge_item(pool, t.token, t.coll, id, obj, max, ttl).await?
    };
    Ok(map_mutation(outcome, 200, "item too large"))
}

/// A stored mutation's outcome as the HTTP answer.
fn map_mutation(outcome: Mutation, ok_status: u16, too_large: &str) -> CrudResponse {
    match outcome {
        Mutation::Ok(_, item) => CrudResponse::json(ok_status, item),
        Mutation::NotFound => CrudResponse::not_found(),
        Mutation::TooLarge => CrudResponse::bad_request(too_large),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_path_rules() {
        assert_eq!(parse_path("/books"), Some(("books".into(), None)));
        assert_eq!(parse_path("/books/"), Some(("books".into(), None)));
        assert_eq!(
            parse_path("/books/abc"),
            Some(("books".into(), Some("abc".into())))
        );
        // 3 segments -> not CRUD
        assert_eq!(parse_path("/books/abc/x"), None);
        // unsafe segment -> not CRUD
        assert_eq!(parse_path("/bad name"), None);
        assert_eq!(parse_path("/books/bad id"), None);
        assert!(matches("/books"));
        assert!(!matches("/a/b/c"));
    }
}
