//! Auto-CRUD store over the `crud_collections` table (replaces Redis
//! `crud:<token>:<coll>`). §5.5 / AC-24..27, AC-S17.
//!
//! Each collection is a JSON array of objects, each with a server-assigned uuid
//! `id`. Mutations run as an atomic read-modify-write inside one
//! `BEGIN IMMEDIATE` transaction: the write lock + `busy_timeout` serialize
//! concurrent writers to the same collection (replacing the Redis WATCH/MULTI
//! CAS), so no update is lost (AC-26) and two POSTs never collide. Item/byte caps
//! are enforced in-transaction. The collection row is upserted with a fresh TTL.

use serde_json::{json, Value};
use sqlx::SqlitePool;

use super::begin_immediate;
use crate::util::json::parse_json_or;

/// Outcome of a mutation closure run inside the transaction.
pub enum Mutation {
    /// New array + the item value to return to the caller.
    Ok(Vec<Value>, Value),
    /// The item id was not found.
    NotFound,
    /// A cap (item bytes) was exceeded.
    TooLarge,
}

/// The stored items JSON of a live (non-expired) collection, verbatim.
pub(crate) async fn read_items_json(
    pool: &SqlitePool,
    token: &str,
    name: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT items_json FROM crud_collections WHERE token = ? AND name = ? AND expires_at > datetime('now')",
    )
    .bind(token)
    .bind(name)
    .fetch_optional(pool)
    .await
}

/// Read the current items array for a collection (live, non-expired).
pub async fn list_items(
    pool: &SqlitePool,
    token: &str,
    name: &str,
) -> Result<Vec<Value>, sqlx::Error> {
    let items = read_items_json(pool, token, name).await?;
    Ok(parse_json_or(items.as_deref(), Vec::new()))
}

/// Drop a collection outright (management #18).
pub(crate) async fn clear(pool: &SqlitePool, token: &str, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM crud_collections WHERE token = ? AND name = ?")
        .bind(token)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

/// Persist a new items array for a collection inside the calling transaction.
async fn write_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    token: &str,
    name: &str,
    items: &[Value],
    ttl_seconds: i64,
) -> Result<(), sqlx::Error> {
    let items_json = serde_json::to_string(items).unwrap_or_else(|_| "[]".into());
    let expires = format!("{ttl_seconds:+} seconds");
    sqlx::query(
        "INSERT INTO crud_collections (token, name, items_json, updated_at, expires_at)
         VALUES (?, ?, ?, datetime('now'), datetime('now', ?))
         ON CONFLICT(token, name) DO UPDATE SET
             items_json = excluded.items_json,
             updated_at = datetime('now'),
             expires_at = excluded.expires_at",
    )
    .bind(token)
    .bind(name)
    .bind(&items_json)
    .bind(&expires)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Append a new item (server-assigned uuid `id`). Returns the stored item, or
/// `Mutation::TooLarge` when `CRUD_MAX_ITEMS` would be exceeded.
pub async fn append_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    mut obj: serde_json::Map<String, Value>,
    max_items: usize,
    ttl_seconds: i64,
) -> Result<Mutation, sqlx::Error> {
    cas_mutate(pool, token, name, ttl_seconds, |mut items| {
        if items.len() >= max_items {
            return Mutation::TooLarge;
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        obj.insert("id".into(), json!(id));
        let stored = Value::Object(obj);
        items.push(stored.clone());
        Mutation::Ok(items, stored)
    })
    .await
}

/// Replace an item by id (PUT). `id` is forced immutable.
pub async fn replace_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    id: &str,
    mut obj: serde_json::Map<String, Value>,
    ttl_seconds: i64,
) -> Result<Mutation, sqlx::Error> {
    obj.insert("id".into(), json!(id));
    cas_mutate(
        pool,
        token,
        name,
        ttl_seconds,
        |mut items| match find_index(&items, id) {
            Some(i) => {
                let v = Value::Object(obj);
                items[i] = v.clone();
                Mutation::Ok(items, v)
            }
            None => Mutation::NotFound,
        },
    )
    .await
}

/// Shallow-merge an item by id (PATCH). `id` immutable; enforces item byte cap.
pub async fn merge_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    id: &str,
    patch: serde_json::Map<String, Value>,
    max_item_bytes: usize,
    ttl_seconds: i64,
) -> Result<Mutation, sqlx::Error> {
    cas_mutate(
        pool,
        token,
        name,
        ttl_seconds,
        |mut items| match find_index(&items, id) {
            Some(i) => {
                let mut merged = items[i].as_object().cloned().unwrap_or_default();
                merged.extend(patch);
                merged.insert("id".into(), json!(id));
                let v = Value::Object(merged);
                if serde_json::to_string(&v).map(|s| s.len()).unwrap_or(0) > max_item_bytes {
                    return Mutation::TooLarge;
                }
                items[i] = v.clone();
                Mutation::Ok(items, v)
            }
            None => Mutation::NotFound,
        },
    )
    .await
}

/// Delete an item by id (DELETE).
pub async fn delete_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    id: &str,
    ttl_seconds: i64,
) -> Result<Mutation, sqlx::Error> {
    cas_mutate(
        pool,
        token,
        name,
        ttl_seconds,
        |mut items| match find_index(&items, id) {
            Some(i) => {
                items.remove(i);
                Mutation::Ok(items, Value::Null)
            }
            None => Mutation::NotFound,
        },
    )
    .await
}

/// Run a mutation closure as one atomic read-modify-write under
/// `repo::begin_immediate` (the write lock is held BEFORE the read, so a
/// concurrent writer can never invalidate the snapshot this mutates). The
/// closure receives the parsed items and returns a `Mutation`; on `Ok` the
/// new array is persisted and committed, otherwise the transaction is
/// dropped (rolled back) untouched.
async fn cas_mutate<F>(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    ttl_seconds: i64,
    f: F,
) -> Result<Mutation, sqlx::Error>
where
    F: FnOnce(Vec<Value>) -> Mutation,
{
    let mut tx = begin_immediate(pool).await?;
    let current: Option<String> =
        sqlx::query_scalar("SELECT items_json FROM crud_collections WHERE token = ? AND name = ?")
            .bind(token)
            .bind(name)
            .fetch_optional(&mut *tx)
            .await?;
    let outcome = f(parse_json_or(current.as_deref(), Vec::new()));
    if let Mutation::Ok(new_items, _) = &outcome {
        write_items(&mut tx, token, name, new_items, ttl_seconds).await?;
        tx.commit().await?;
    }
    Ok(outcome)
}

fn find_index(items: &[Value], id: &str) -> Option<usize> {
    items.iter().position(|it| {
        it.as_object()
            .and_then(|o| o.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s == id)
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests;
