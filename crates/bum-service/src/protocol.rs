//! Minimal JSON-RPC: each line is one request or response.
use interprocess::local_socket::{prelude::*, GenericFilePath, Name};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use bum_core::paths::Paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Self { id, result: Some(result), error: None }
    }

    pub fn err(id: u64, error: impl Into<String>) -> Self {
        Self { id, result: None, error: Some(error.into()) }
    }
}

/// Unix: socket file in the data dir (permissions 0600). Windows: named pipe based on the user name.
#[cfg(unix)]
pub fn socket_name(paths: &Paths) -> std::io::Result<Name<'static>> {
    paths.socket().to_fs_name::<GenericFilePath>()
}

#[cfg(windows)]
pub fn socket_name(paths: &Paths) -> std::io::Result<Name<'static>> {
    use interprocess::local_socket::GenericNamespaced;
    use std::hash::{Hash, Hasher};
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
    // One namespace per data dir: parallel instances (and tests) must not collide on a global pipe name.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    paths.data_dir.hash(&mut h);
    format!("bumeter-{user}-{:016x}", h.finish()).to_ns_name::<GenericNamespaced>()
}
