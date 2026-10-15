//! JSON-RPC tối giản: mỗi dòng là một request hoặc response.
use interprocess::local_socket::{prelude::*, GenericFilePath, Name};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tk_core::paths::Paths;

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

/// Unix: file socket trong data dir (quyền 0600). Windows: named pipe theo tên user.
#[cfg(unix)]
pub fn socket_name(paths: &Paths) -> std::io::Result<Name<'static>> {
    paths.socket().to_fs_name::<GenericFilePath>()
}

#[cfg(windows)]
pub fn socket_name(_paths: &Paths) -> std::io::Result<Name<'static>> {
    use interprocess::local_socket::GenericNamespaced;
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
    format!("bumeter-{user}").to_ns_name::<GenericNamespaced>()
}
