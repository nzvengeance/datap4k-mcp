// Tool request types and formatting helpers for the MCP server.

use schemars::JsonSchema;
use serde::Deserialize;

use crate::model::Node;

// ---------------------------------------------------------------------------
// Request structs for each tool
// ---------------------------------------------------------------------------

/// Full-text search across entity names and properties.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRequest {
    /// Search query string (FTS5 syntax supported).
    pub query: String,
    /// Maximum number of results to return (default 20).
    pub limit: Option<usize>,
}

/// Look up a single entity by UUID or class name.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LookupRequest {
    /// Entity UUID.
    pub uuid: Option<String>,
    /// Entity class name (exact match).
    pub class_name: Option<String>,
}

/// Traverse the graph from a starting entity.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TraverseRequest {
    /// UUID of the starting entity.
    pub uuid: String,
    /// Maximum traversal depth (default 2, max 5).
    pub depth: Option<u32>,
}

/// Find the shortest path between two entities.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PathRequest {
    /// UUID of the starting entity.
    pub from: String,
    /// UUID of the target entity.
    pub to: String,
    /// Maximum search depth (default 5, max 10).
    pub max_depth: Option<u32>,
}

/// Compare an entity across two game versions.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DiffRequest {
    /// Entity UUID or class name.
    pub entity: String,
    /// First version code (e.g. "4.6.0-live").
    pub version_a: String,
    /// Second version code (e.g. "4.7.0-live").
    pub version_b: String,
}

/// Execute a raw SQL query against the SQLite index.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryRequest {
    /// SQL query string (SELECT only recommended).
    pub sql: String,
}

/// Execute a raw Datalog/CozoScript query against the graph.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphQueryRequest {
    /// CozoScript query string.
    pub query: String,
}

/// Find where an entity can be found (Location-type relationships).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LocateRequest {
    /// Entity UUID or class name to locate.
    pub entity: String,
}

/// Find which NPCs, loadouts, or ships use an item.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WhoUsesRequest {
    /// Item UUID or class name.
    pub item: String,
}

/// Trigger indexing of a p4k data directory.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct IndexRequest {
    /// Path to extracted p4k data directory.
    pub path: String,
    /// Game version code (auto-detected from directory name if omitted).
    pub version: Option<String>,
    /// If true, drop existing data for this version before re-indexing.
    pub reindex: Option<bool>,
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// One-line summary per node: `[Type] ClassName - uuid: UUID (source: src)`
pub fn format_nodes(nodes: &[Node]) -> String {
    if nodes.is_empty() {
        return "No results found.".to_string();
    }
    nodes
        .iter()
        .map(|n| {
            format!(
                "[{}] {} — uuid: {} (source: {})",
                n.entity_type, n.class_name, n.id, n.source
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detailed markdown output with properties JSON block per node.
pub fn format_nodes_detailed(nodes: &[Node]) -> String {
    if nodes.is_empty() {
        return "No results found.".to_string();
    }
    nodes
        .iter()
        .map(|n| {
            let props = serde_json::to_string_pretty(&n.properties).unwrap_or_default();
            format!(
                "## [{}] {}\n- **UUID:** {}\n- **Record:** {}\n- **Source:** {} ({})\n- **Version:** {}\n\n```json\n{}\n```",
                n.entity_type, n.class_name, n.id, n.record_name, n.source, n.source_path, n.game_version, props
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}
