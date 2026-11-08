//! HTTP/WebSocket plumbing shared by the planes: client-IP resolution for the
//! rate limiters and the named header policies.

pub(crate) mod client_ip;
pub(crate) mod headers;
