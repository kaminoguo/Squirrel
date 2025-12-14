//! MCP server (stdio transport).
//!
//! Implements MCP-001 and MCP-002 from specs/INTERFACES.md.

use crate::error::Error;
use crate::mcp;

/// Run MCP server on stdio.
pub async fn run() -> Result<(), Error> {
    mcp::run_server().await
}
