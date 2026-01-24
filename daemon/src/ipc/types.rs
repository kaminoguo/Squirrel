//! IPC request and response types.

use serde::{Deserialize, Serialize};

use crate::watcher::EpisodeEvent;

/// Existing user style with ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingUserStyle {
    pub id: String,
    pub text: String,
}

/// Existing project memory with ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingProjectMemory {
    pub id: String,
    pub category: String,
    pub subcategory: String,
    pub text: String,
}

/// IPC-001: process_episode request params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEpisodeRequest {
    pub project_id: String,
    pub project_root: String,
    pub events: Vec<EpisodeEvent>,
    pub existing_user_styles: Vec<ExistingUserStyle>,
    pub existing_project_memories: Vec<ExistingProjectMemory>,
}

/// Memory operation type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MemoryOp {
    Add,
    Update,
    Delete,
}

/// User style change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStyleChange {
    pub op: MemoryOp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    pub text: String,
}

/// Project memory change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemoryChange {
    pub op: MemoryOp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    pub category: String,
    pub subcategory: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_text: Option<String>,
}

/// IPC-001: process_episode response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEpisodeResponse {
    pub skipped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
    pub user_styles: Vec<UserStyleChange>,
    pub project_memories: Vec<ProjectMemoryChange>,
}

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest<T> {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: T,
    pub id: u64,
}

impl<T> JsonRpcRequest<T> {
    pub fn new(method: impl Into<String>, params: T, id: u64) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
            id,
        }
    }
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

/// JSON-RPC 2.0 error.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}
