//! CLI commands for Squirrel.

pub mod agents;
pub mod config;
pub mod daemon;
pub mod export;
pub mod flush;
pub mod forget;
pub mod gitignore;
pub mod import;
pub mod init;
pub mod instructions;
pub mod mcp;
pub mod mcp_config;
pub mod policy;
pub mod search;
pub mod status;
pub mod sync;

use clap::{Parser, Subcommand};

/// Squirrel - Local-first memory system for AI coding tools
#[derive(Parser)]
#[command(name = "sqrl")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize project for Squirrel
    Init {
        /// Don't ingest historical logs
        #[arg(long)]
        skip_history: bool,
    },

    /// Search memories
    Search {
        /// Search query
        query: String,

        /// Filter by kind (preference, invariant, pattern, guard, note)
        #[arg(long)]
        kind: Option<String>,

        /// Filter by tier (short_term, long_term, emergency)
        #[arg(long)]
        tier: Option<String>,
    },

    /// Soft delete memory
    Forget {
        /// Memory ID
        #[arg(long)]
        id: Option<String>,

        /// Search query to find memory
        #[arg(long)]
        query: Option<String>,

        /// Skip confirmation
        #[arg(long)]
        confirm: bool,
    },

    /// Export memories as JSON
    Export {
        /// Filter by kind
        #[arg(long)]
        kind: Option<String>,

        /// Export project memories only
        #[arg(long)]
        project: bool,
    },

    /// Import memories from JSON
    Import {
        /// JSON file path
        file: String,
    },

    /// Show daemon and memory stats
    Status,

    /// Configure settings
    Config {
        /// Configuration key
        key: Option<String>,

        /// Configuration value
        value: Option<String>,
    },

    /// Sync all projects with CLI configs
    Sync,

    /// Force episode boundary and processing
    Flush,

    /// Manage memory policy
    Policy {
        /// Action: show or reload
        #[arg(default_value = "show")]
        action: String,
    },

    /// Run MCP server (stdio transport)
    Mcp,

    /// Run background daemon
    Daemon,
}
