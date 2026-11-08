//! The FROZEN §5.3 interface contract as serde structs (PORT of `app/models.py`,
//! + the OQ-2 `chaos_mode` additions from architecture.md §5.3).
//!
//! These serialize to the EXACT §5.3 JSON shapes: field names/types/defaults
//! are frozen. `Option<T>` fields serialize as `null` when absent (the JSON key
//! is present with `null`, matching pydantic's default) — we deliberately do NOT
//! `skip_serializing_if`. Collection/string defaults use `#[serde(default)]` so
//! a partial request body fills the documented defaults.
//!
//! Split by domain; every type is re-exported here so `crate::models::X`
//! paths keep working.

mod common;
mod endpoint;
mod rule;
mod share;
mod trace;

pub use common::*;
pub use endpoint::*;
pub use rule::*;
pub use share::*;
pub use trace::*;
