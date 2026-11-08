//! Generic response shapes shared across resources.

use serde::Serialize;

// ---- Generic ----
#[derive(Debug, Serialize)]
pub struct Message {
    pub message: String,
    pub success: bool,
}

impl Message {
    pub fn new(message: impl Into<String>) -> Self {
        Message {
            message: message.into(),
            success: true,
        }
    }
}
