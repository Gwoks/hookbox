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

use serde_json::Value;

use crate::config::Config;
use crate::crud_store::{self, Mutation};
use crate::helpers::is_safe_key;
use crate::state::AppState;

/// A CRUD outcome the engine renders into an HTTP response (served_by="crud").
pub struct CrudResponse {
    pub status: u16,
    /// `None` for a 204 (no body).
    pub body: Option<Value>,
}

impl CrudResponse {
    fn json(status: u16, body: Value) -> Self {
        CrudResponse { status, body: Some(body) }
    }
    fn no_content() -> Self {
        CrudResponse { status: 204, body: None }
    }
    fn bad_request(detail: &str) -> Self {
        CrudResponse::json(400, serde_json::json!({"error": "bad_request", "detail": detail}))
    }
    fn not_found() -> Self {
        CrudResponse::json(404, serde_json::json!({"error": "not_found", "detail": "No such item."}))
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

fn parse_object_body(body_text: &str, cfg: &Config) -> Result<serde_json::Map<String, Value>, String> {
    if body_text.trim().is_empty() {
        return Err("empty body".into());
    }
    if body_text.len() > cfg.crud_max_item_bytes {
        return Err("item too large".into());
    }
    let v: Value = serde_json::from_str(body_text).map_err(|_| "body is not valid JSON".to_string())?;
    match v {
        Value::Object(m) => Ok(m),
        _ => Err("body must be a JSON object".into()),
    }
}

fn find<'a>(items: &'a [Value], id: &str) -> Option<&'a Value> {
    items.iter().find(|it| it.get("id").and_then(|v| v.as_str()) == Some(id))
}

/// Execute one Auto-CRUD operation. Returns a `CrudResponse`, or `Err(())` on a
/// SQLite fault (the engine maps to 503 store_unavailable).
pub async fn handle(
    state: &AppState,
    token: &str,
    method: &str,
    mock_path: &str,
    body_text: &str,
) -> Result<CrudResponse, ()> {
    let (coll, ident) = match parse_path(mock_path) {
        Some(p) => p,
        None => return Ok(CrudResponse::not_found()),
    };
    let method = method.to_ascii_uppercase();
    let cfg = &state.cfg;
    let ttl = cfg.crud_ttl_seconds;

    // Collection-level (no id).
    if ident.is_none() {
        match method.as_str() {
            "GET" | "HEAD" => {
                let items = crud_store::list_items(&state.pool, token, &coll).await.map_err(|_| ())?;
                return Ok(CrudResponse::json(200, Value::Array(items)));
            }
            "POST" => {
                let obj = match parse_object_body(body_text, cfg) {
                    Ok(o) => o,
                    Err(e) => return Ok(CrudResponse::bad_request(&e)),
                };
                return match crud_store::append_item(&state.pool, token, &coll, obj, cfg.crud_max_items, ttl).await {
                    Ok(Mutation::Ok(_, item)) => Ok(CrudResponse::json(201, item)),
                    Ok(Mutation::TooLarge) => Ok(CrudResponse::bad_request("collection is full")),
                    Ok(Mutation::NotFound) => Ok(CrudResponse::not_found()),
                    Err(_) => Err(()),
                };
            }
            _ => return Ok(CrudResponse::bad_request("an item id is required for this method")),
        }
    }

    // Item-level (collection + id).
    let id = ident.unwrap();
    match method.as_str() {
        "GET" | "HEAD" => {
            let items = crud_store::list_items(&state.pool, token, &coll).await.map_err(|_| ())?;
            match find(&items, &id) {
                Some(item) => Ok(CrudResponse::json(200, item.clone())),
                None => Ok(CrudResponse::not_found()),
            }
        }
        "PUT" => {
            let obj = match parse_object_body(body_text, cfg) {
                Ok(o) => o,
                Err(e) => return Ok(CrudResponse::bad_request(&e)),
            };
            match crud_store::replace_item(&state.pool, token, &coll, &id, obj, ttl).await {
                Ok(Mutation::Ok(_, item)) => Ok(CrudResponse::json(200, item)),
                Ok(Mutation::NotFound) => Ok(CrudResponse::not_found()),
                Ok(Mutation::TooLarge) => Ok(CrudResponse::bad_request("item too large")),
                Err(_) => Err(()),
            }
        }
        "PATCH" => {
            let patch = match parse_object_body(body_text, cfg) {
                Ok(o) => o,
                Err(e) => return Ok(CrudResponse::bad_request(&e)),
            };
            match crud_store::merge_item(&state.pool, token, &coll, &id, patch, cfg.crud_max_item_bytes, ttl).await {
                Ok(Mutation::Ok(_, item)) => Ok(CrudResponse::json(200, item)),
                Ok(Mutation::NotFound) => Ok(CrudResponse::not_found()),
                Ok(Mutation::TooLarge) => Ok(CrudResponse::bad_request("item too large")),
                Err(_) => Err(()),
            }
        }
        "DELETE" => match crud_store::delete_item(&state.pool, token, &coll, &id, ttl).await {
            Ok(Mutation::Ok(..)) => Ok(CrudResponse::no_content()),
            Ok(Mutation::NotFound) => Ok(CrudResponse::not_found()),
            Ok(Mutation::TooLarge) => Ok(CrudResponse::bad_request("item too large")),
            Err(_) => Err(()),
        },
        _ => Ok(CrudResponse::bad_request("unsupported method for an item")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_path_rules() {
        assert_eq!(parse_path("/books"), Some(("books".into(), None)));
        assert_eq!(parse_path("/books/"), Some(("books".into(), None)));
        assert_eq!(parse_path("/books/abc"), Some(("books".into(), Some("abc".into()))));
        // 3 segments -> not CRUD
        assert_eq!(parse_path("/books/abc/x"), None);
        // unsafe segment -> not CRUD
        assert_eq!(parse_path("/bad name"), None);
        assert_eq!(parse_path("/books/bad id"), None);
        assert!(matches("/books"));
        assert!(!matches("/a/b/c"));
    }
}
