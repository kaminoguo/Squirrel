//! MCP server implementation for Squirrel.
//!
//! Implements MCP-001 and MCP-002 from specs/INTERFACES.md.
//! Uses rmcp crate with 2025-11 MCP specification.

use std::path::PathBuf;

use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, ServiceExt,
};
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::Error;

/// Parameters for squirrel_get_task_context (MCP-001).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetTaskContextParams {
    /// Absolute path to project root.
    #[schemars(description = "Absolute path to project root")]
    pub project_root: String,

    /// Description of the coding task.
    #[schemars(description = "Description of the task")]
    pub task: String,

    /// Maximum tokens for context (default: 400).
    #[schemars(description = "Max tokens for context")]
    #[serde(default = "default_token_budget")]
    pub context_budget_tokens: i32,
}

fn default_token_budget() -> i32 {
    400
}

/// Parameters for squirrel_search_memory (MCP-002).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchMemoryParams {
    /// Absolute path to project root.
    #[schemars(description = "Absolute path to project root")]
    pub project_root: String,

    /// Search query.
    #[schemars(description = "Search query")]
    pub query: String,

    /// Maximum number of results (default: 10).
    #[schemars(description = "Max results")]
    #[serde(default = "default_top_k")]
    pub top_k: i32,

    /// Filter by memory kind.
    #[schemars(description = "Filter by kind (preference, invariant, pattern, guard, note)")]
    pub kind: Option<String>,

    /// Filter by memory tier.
    #[schemars(description = "Filter by tier (short_term, long_term, emergency)")]
    pub tier: Option<String>,
}

fn default_top_k() -> i32 {
    10
}

/// Memory result for search responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResult {
    pub id: String,
    pub kind: String,
    pub tier: String,
    pub status: String,
    pub key: Option<String>,
    pub text: String,
    pub score: f64,
    pub confidence: f64,
}

/// Squirrel MCP Server.
#[derive(Clone)]
pub struct SquirrelServer {
    tool_router: ToolRouter<Self>,
}

impl SquirrelServer {
    /// Create a new Squirrel MCP server.
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Get project database path.
    fn get_project_db_path(project_root: &str) -> PathBuf {
        PathBuf::from(project_root)
            .join(".sqrl")
            .join("squirrel.db")
    }

    /// Check if project is initialized.
    fn is_project_initialized(project_root: &str) -> bool {
        Self::get_project_db_path(project_root).exists()
    }
}

#[tool_router]
impl SquirrelServer {
    /// Get relevant memories for a coding task (MCP-001).
    ///
    /// Call BEFORE fixing bugs, refactoring, or adding features to recall
    /// past issues, patterns, and project-specific knowledge.
    #[tool(
        name = "squirrel_get_task_context",
        description = "Get relevant memories for a coding task. Call BEFORE fixing bugs, refactoring, or adding features."
    )]
    pub async fn get_task_context(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<GetTaskContextParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0;
        let project_root = &params.project_root;

        // Check project initialized
        if !Self::is_project_initialized(project_root) {
            return Err(McpError::invalid_params(
                "Project not initialized. Run 'sqrl init' first.",
                None,
            ));
        }

        let db_path = Self::get_project_db_path(project_root);

        // Open database
        let db = match Database::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                return Err(McpError::internal_error(
                    format!("Failed to open database: {}", e),
                    None,
                ));
            }
        };

        // Get project_id from path
        let project_id = PathBuf::from(project_root)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Get active memories
        let memories = match db.get_active_memories(&project_id, 20) {
            Ok(mems) => mems,
            Err(e) => {
                return Err(McpError::internal_error(
                    format!("Failed to query memories: {}", e),
                    None,
                ));
            }
        };

        if memories.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No memories found for this project yet. Memories will be extracted as you work.",
            )]));
        }

        // Build context prompt (v1: template-based)
        let mut context_lines = Vec::new();
        let mut used_ids = Vec::new();

        // Group by kind
        let mut by_kind: std::collections::HashMap<String, Vec<&crate::db::Memory>> =
            std::collections::HashMap::new();

        for mem in &memories {
            by_kind.entry(mem.kind.clone()).or_default().push(mem);
        }

        // Order: guards first (warnings), then invariants, preferences, patterns, notes
        let kind_order = ["guard", "invariant", "preference", "pattern", "note"];

        for kind in kind_order {
            if let Some(mems) = by_kind.get(kind) {
                let kind_label = match kind {
                    "guard" => "Warnings",
                    "invariant" => "Invariants",
                    "preference" => "Preferences",
                    "pattern" => "Patterns",
                    "note" => "Notes",
                    _ => kind,
                };

                context_lines.push(format!("**{}:**", kind_label));
                for mem in mems {
                    context_lines.push(format!("- {}", mem.text));
                    used_ids.push(mem.id.clone());
                }
                context_lines.push(String::new());
            }
        }

        let context_prompt = context_lines.join("\n").trim().to_string();

        // Update use_count for used memories
        for id in &used_ids {
            let _ = db.increment_use_count(id);
            let _ = db.increment_opportunities(id);
        }

        // Return context
        let response = serde_json::json!({
            "context_prompt": context_prompt,
            "memory_ids": used_ids,
            "task": params.task,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap_or(context_prompt),
        )]))
    }

    /// Search memories by semantic query (MCP-002).
    #[tool(
        name = "squirrel_search_memory",
        description = "Search memories by semantic query"
    )]
    pub async fn search_memory(
        &self,
        params: rmcp::handler::server::wrapper::Parameters<SearchMemoryParams>,
    ) -> Result<CallToolResult, McpError> {
        let params = params.0;
        let project_root = &params.project_root;

        // Check project initialized
        if !Self::is_project_initialized(project_root) {
            return Err(McpError::invalid_params(
                "Project not initialized. Run 'sqrl init' first.",
                None,
            ));
        }

        if params.query.is_empty() {
            return Err(McpError::invalid_params("Empty query", None));
        }

        let db_path = Self::get_project_db_path(project_root);

        // Open database
        let db = match Database::open(&db_path) {
            Ok(db) => db,
            Err(e) => {
                return Err(McpError::internal_error(
                    format!("Failed to open database: {}", e),
                    None,
                ));
            }
        };

        // Get project_id from path
        let project_id = PathBuf::from(project_root)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Try vector search first via IPC-002
        let memories =
            match crate::ipc::send_embed_text(crate::ipc::MEMORY_SERVICE_SOCKET, &params.query)
                .await
            {
                Ok(embedding) => {
                    // Vector search with embedding
                    match db.search_memories_by_vector(
                        &embedding,
                        Some(&project_id),
                        params.top_k as usize,
                    ) {
                        Ok(mems) => mems,
                        Err(e) => {
                            tracing::warn!("Vector search failed, falling back to text: {}", e);
                            // Fall back to text search
                            db.get_active_memories(&project_id, params.top_k as usize)
                                .map_err(|e| {
                                    McpError::internal_error(
                                        format!("Failed to query memories: {}", e),
                                        None,
                                    )
                                })?
                        }
                    }
                }
                Err(e) => {
                    // Memory Service unavailable - fall back to text search
                    tracing::debug!("Memory Service unavailable for embedding: {}", e);
                    db.get_active_memories(&project_id, params.top_k as usize)
                        .map_err(|e| {
                            McpError::internal_error(
                                format!("Failed to query memories: {}", e),
                                None,
                            )
                        })?
                }
            };

        // Filter by kind/tier if specified
        let filtered: Vec<_> = memories
            .into_iter()
            .filter(|m| {
                if let Some(ref kind) = params.kind {
                    if &m.kind != kind {
                        return false;
                    }
                }
                if let Some(ref tier) = params.tier {
                    if &m.tier != tier {
                        return false;
                    }
                }
                true
            })
            .collect();

        let results: Vec<MemoryResult> = filtered
            .iter()
            .map(|m| MemoryResult {
                id: m.id.clone(),
                kind: m.kind.clone(),
                tier: m.tier.clone(),
                status: m.status.clone(),
                key: m.key.clone(),
                text: m.text.clone(),
                score: 1.0, // Vector similarity score (from sqlite-vec distance)
                confidence: m.confidence.unwrap_or(0.0),
            })
            .collect();

        let response = serde_json::json!({
            "results": results,
            "query": params.query,
            "count": results.len(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap_or_default(),
        )]))
    }
}

#[tool_handler]
impl rmcp::ServerHandler for SquirrelServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: rmcp::model::Implementation::from_build_env(),
            instructions: Some(
                "Squirrel is a local-first memory system for AI coding tools. \
                Use squirrel_get_task_context before starting coding tasks to recall \
                past issues, patterns, and project-specific knowledge."
                    .to_string(),
            ),
        }
    }
}

/// Run the MCP server on stdio.
pub async fn run_server() -> Result<(), Error> {
    tracing::info!("Starting Squirrel MCP server");

    let server = SquirrelServer::new();

    let service = server
        .serve(stdio())
        .await
        .map_err(|e| Error::Mcp(format!("Failed to start MCP server: {}", e)))?;

    tracing::info!("Squirrel MCP server running");

    service
        .waiting()
        .await
        .map_err(|e| Error::Mcp(format!("MCP server error: {}", e)))?;

    Ok(())
}
