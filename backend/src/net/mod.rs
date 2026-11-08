//! HTTP/WebSocket plumbing shared by the planes: client-IP resolution for the
//! rate limiters, the named header policies, and the WebSocket close codes.

pub(crate) mod client_ip;
pub(crate) mod headers;
pub mod ws;
