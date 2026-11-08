//! Data-access layer: the SQL behind the SQLite-backed stores that replace
//! the Python app's two Redis responsibilities (per-endpoint state, Auto-CRUD
//! collections), the trace write path, and every `owners` query. `db` keeps
//! only the pool and the migrations.

pub(crate) mod crud;
pub(crate) mod endpoints;
pub(crate) mod owners;
pub(crate) mod requests;
pub(crate) mod retention;
pub(crate) mod rules;
pub(crate) mod shares;
pub(crate) mod state;

use serde_json::Value;
use sqlx::{Sqlite, SqlitePool, Transaction};

/// Begin a transaction that takes SQLite's write lock up front
/// (`BEGIN IMMEDIATE`, via `pool.begin_with`), for every read-modify-write.
/// Concurrent writers queue on `busy_timeout` instead of reading a snapshot
/// that a competing commit can invalidate before their write. (Issuing
/// `BEGIN IMMEDIATE` as a statement inside `pool.begin()` cannot do this: it
/// fails with "cannot start a transaction within a transaction", and that
/// error used to be swallowed.)
pub(crate) async fn begin_immediate(
    pool: &SqlitePool,
) -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
    pool.begin_with("BEGIN IMMEDIATE").await
}

/// Bind a JSON scalar onto a query (string/int/bool/null). Used by the
/// allow-listed PATCH builders.
pub(crate) fn bind_value<'q>(
    q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    v: &'q Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match v {
        Value::Null => q.bind(Option::<String>::None),
        Value::Bool(b) => q.bind(*b as i64),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                q.bind(i)
            } else {
                q.bind(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => q.bind(s.as_str()),
        other => q.bind(other.to_string()),
    }
}
