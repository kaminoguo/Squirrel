//! Error types for Squirrel daemon.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Project not initialized: {0}")]
    NotInitialized(String),

    #[allow(dead_code)] // For sqrl forget (CLI-005)
    #[error("Memory not found: {0}")]
    MemoryNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Watcher error: {0}")]
    Watcher(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn ipc(msg: impl Into<String>) -> Self {
        Error::Ipc(msg.into())
    }

    pub fn not_initialized(path: impl Into<String>) -> Self {
        Error::NotInitialized(path.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Error::Validation(msg.into())
    }
}
