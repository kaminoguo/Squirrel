//! MCP (Model Context Protocol) server for AI tools.
//!
//! Implements MCP-001: squirrel_get_memory tool.
//! Implements MCP-002: squirrel_get_doc_debt tool.

use std::io::{BufRead, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, error, info};

use crate::error::Error;
use crate::storage;

/// MCP protocol version.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server info.
const SERVER_NAME: &str = "squirrel";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON-RPC request.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Value,
    id: Option<Value>,
}

/// JSON-RPC response.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: Value,
}

/// JSON-RPC error.
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError { code, message }),
            id,
        }
    }
}

/// Tool definition for MCP.
fn get_tools() -> Value {
    json!({
        "tools": [
            {
                "name": "squirrel_get_memory",
                "description": "Get project memories from Squirrel. Call when user asks to 'use Squirrel' or wants project context.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_root": {
                            "type": "string",
                            "description": "Absolute path to project root"
                        }
                    },
                    "required": ["project_root"]
                }
            },
            {
                "name": "squirrel_get_doc_debt",
                "description": "Get documentation debt for a project. Shows commits that changed code without updating expected docs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project_root": {
                            "type": "string",
                            "description": "Absolute path to project root"
                        }
                    },
                    "required": ["project_root"]
                }
            }
        ]
    })
}

/// Handle squirrel_get_memory tool call.
fn handle_get_memory(params: &Value) -> Result<Value, Error> {
    let project_root = params
        .get("arguments")
        .and_then(|a| a.get("project_root"))
        .and_then(|p| p.as_str())
        .ok_or_else(|| Error::Ipc("Missing project_root parameter".to_string()))?;

    let path = Path::new(project_root);
    if !path.exists() {
        return Err(Error::Ipc(format!(
            "Project root does not exist: {}",
            project_root
        )));
    }

    let markdown = storage::format_memories_as_markdown(path)?;

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": markdown
            }
        ]
    }))
}

/// Handle squirrel_get_doc_debt tool call.
fn handle_get_doc_debt(params: &Value) -> Result<Value, Error> {
    let project_root = params
        .get("arguments")
        .and_then(|a| a.get("project_root"))
        .and_then(|p| p.as_str())
        .ok_or_else(|| Error::Ipc("Missing project_root parameter".to_string()))?;

    let path = Path::new(project_root);
    if !path.exists() {
        return Err(Error::Ipc(format!(
            "Project root does not exist: {}",
            project_root
        )));
    }

    let debts = storage::get_unresolved_doc_debt(path)?;

    if debts.is_empty() {
        return Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": "No documentation debt found. All docs are up to date!"
                }
            ]
        }));
    }

    // Format as markdown
    let mut markdown = format!(
        "# Documentation Debt\n\n{} commit(s) have unupdated docs:\n\n",
        debts.len()
    );

    for debt in &debts {
        let short_sha = &debt.commit_sha[..7.min(debt.commit_sha.len())];
        let msg = debt.commit_message.as_deref().unwrap_or("(no message)");
        markdown.push_str(&format!("## `{}` {}\n\n", short_sha, msg));
        markdown.push_str("**Changed code:**\n");
        for file in &debt.code_files {
            markdown.push_str(&format!("- {}\n", file));
        }
        markdown.push_str("\n**Expected doc updates:**\n");
        for doc in &debt.expected_docs {
            markdown.push_str(&format!("- {}\n", doc));
        }
        markdown.push_str(&format!("\n*Detection rule: {}*\n\n", debt.detection_rule));
    }

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": markdown
            }
        ]
    }))
}

/// Handle incoming MCP request.
fn handle_request(request: &JsonRpcRequest) -> JsonRpcResponse {
    let id = request.id.clone().unwrap_or(Value::Null);

    match request.method.as_str() {
        "initialize" => {
            info!("MCP initialize");
            JsonRpcResponse::success(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION
                    }
                }),
            )
        }

        "notifications/initialized" => {
            debug!("MCP initialized notification");
            // Notification, no response needed but we return empty for consistency
            JsonRpcResponse::success(id, json!({}))
        }

        "tools/list" => {
            debug!("MCP tools/list");
            JsonRpcResponse::success(id, get_tools())
        }

        "tools/call" => {
            let tool_name = request
                .params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("");

            debug!(tool = tool_name, "MCP tools/call");

            match tool_name {
                "squirrel_get_memory" => match handle_get_memory(&request.params) {
                    Ok(result) => JsonRpcResponse::success(id, result),
                    Err(e) => JsonRpcResponse::error(id, -32000, e.to_string()),
                },
                "squirrel_get_doc_debt" => match handle_get_doc_debt(&request.params) {
                    Ok(result) => JsonRpcResponse::success(id, result),
                    Err(e) => JsonRpcResponse::error(id, -32000, e.to_string()),
                },
                _ => JsonRpcResponse::error(id, -32601, format!("Unknown tool: {}", tool_name)),
            }
        }

        _ => {
            debug!(method = request.method, "Unknown MCP method");
            JsonRpcResponse::error(id, -32601, format!("Method not found: {}", request.method))
        }
    }
}

/// Run the MCP server (stdio mode).
pub fn run() -> Result<(), Error> {
    info!("Starting MCP server");

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        debug!(request = %line, "MCP request");

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "Failed to parse MCP request");
                let response =
                    JsonRpcResponse::error(Value::Null, -32700, format!("Parse error: {}", e));
                let response_str = serde_json::to_string(&response)?;
                writeln!(stdout, "{}", response_str)?;
                stdout.flush()?;
                continue;
            }
        };

        // Skip notifications (no id)
        if request.id.is_none() && request.method.starts_with("notifications/") {
            debug!(method = request.method, "Skipping notification");
            continue;
        }

        let response = handle_request(&request);
        let response_str = serde_json::to_string(&response)?;

        debug!(response = %response_str, "MCP response");

        writeln!(stdout, "{}", response_str)?;
        stdout.flush()?;
    }

    info!("MCP server stopped");
    Ok(())
}
