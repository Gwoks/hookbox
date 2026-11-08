//! Data-access layer: the SQL behind the SQLite-backed stores that replace
//! the Python app's two Redis responsibilities (per-endpoint state, Auto-CRUD
//! collections), the trace write path, and every `owners` query. `db` keeps
//! only the pool and the migrations.

pub(crate) mod crud;
pub(crate) mod owners;
pub(crate) mod requests;
pub(crate) mod state;

use serde_json::Value;

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
