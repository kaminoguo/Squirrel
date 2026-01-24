//! JSON-RPC 2.0 client over Unix socket.

use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, error};

use super::types::{
    JsonRpcRequest, JsonRpcResponse, ProcessEpisodeRequest, ProcessEpisodeResponse,
};
use crate::Error;

/// Default socket path.
const SOCKET_PATH: &str = "/tmp/sqrl_agent.sock";

/// IPC client for communicating with Python Memory Service.
pub struct IpcClient {
    socket_path: String,
    request_id: AtomicU64,
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new(SOCKET_PATH.to_string())
    }
}

impl IpcClient {
    /// Create a new IPC client.
    pub fn new(socket_path: String) -> Self {
        Self {
            socket_path,
            request_id: AtomicU64::new(1),
        }
    }

    /// Get the next request ID.
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Check if the Python service is running.
    pub async fn is_service_running(&self) -> bool {
        UnixStream::connect(&self.socket_path).await.is_ok()
    }

    /// Send process_episode request (IPC-001).
    pub async fn process_episode(
        &self,
        request: ProcessEpisodeRequest,
    ) -> Result<ProcessEpisodeResponse, Error> {
        let rpc_request = JsonRpcRequest::new("process_episode", request, self.next_id());
        self.send_request(rpc_request).await
    }

    /// Send a JSON-RPC request and receive the response.
    async fn send_request<T, R>(&self, request: JsonRpcRequest<T>) -> Result<R, Error>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        debug!(method = %request.method, id = request.id, "Sending IPC request");

        // Connect to socket
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| Error::Ipc(format!("Failed to connect to {}: {}", self.socket_path, e)))?;

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Serialize and send request
        let request_json = serde_json::to_string(&request)?;
        writer
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| Error::Ipc(format!("Failed to write request: {}", e)))?;
        writer
            .write_all(b"\n")
            .await
            .map_err(|e| Error::Ipc(format!("Failed to write newline: {}", e)))?;
        writer
            .flush()
            .await
            .map_err(|e| Error::Ipc(format!("Failed to flush: {}", e)))?;

        // Read response
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| Error::Ipc(format!("Failed to read response: {}", e)))?;

        // Parse response
        let response: JsonRpcResponse<R> = serde_json::from_str(&response_line)
            .map_err(|e| Error::Ipc(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = response.error {
            error!(code = error.code, message = %error.message, "IPC error");
            return Err(Error::Ipc(error.to_string()));
        }

        response
            .result
            .ok_or_else(|| Error::Ipc("No result in response".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_increment() {
        let client = IpcClient::default();
        assert_eq!(client.next_id(), 1);
        assert_eq!(client.next_id(), 2);
        assert_eq!(client.next_id(), 3);
    }
}
