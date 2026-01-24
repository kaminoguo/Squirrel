//! IPC client for communicating with Python Memory Service.

pub mod client;
pub mod types;

pub use client::IpcClient;
pub use types::ProcessEpisodeRequest;
