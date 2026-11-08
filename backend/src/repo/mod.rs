//! Data-access layer: the SQL behind the SQLite-backed stores that replace
//! the Python app's two Redis responsibilities (per-endpoint state, Auto-CRUD
//! collections) plus the trace write path. `db` keeps only the pool and the
//! migrations.

pub(crate) mod crud;
pub(crate) mod requests;
pub(crate) mod state;
