//! Error types for the Squirrel daemon.

use thiserror::Error;

/// Daemon error type.
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("File watch error: {0}")]
    Watch(#[from] notify::Error),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid log entry: {0}")]
    InvalidLogEntry(String),

    #[error("Home directory not found")]
    HomeDirNotFound,
}

/// IPC error codes from INTERFACES.md.
#[derive(Debug, Clone, Copy)]
#[repr(i32)]
#[allow(dead_code)]
pub enum IpcErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    ProjectNotInitialized = -32001,
    DaemonNotRunning = -32002,
    LlmError = -32003,
    InvalidProjectRoot = -32004,
    NoMemoriesFound = -32005,
}
