// MCP server — DataP4kServer with 11 tools.

pub mod prompts;
pub mod resources;
pub mod tools;

use std::sync::{Arc, Mutex, RwLock};

use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, ErrorData, GetPromptRequestParams, GetPromptResult,
        Implementation, ListPromptsResult, ListResourcesResult, ReadResourceRequestParams,
        ReadResourceResult, ServerCapabilities, ServerInfo,
    },
    tool, tool_handler, tool_router, transport,
};
use uuid::Uuid;

use crate::config::Config;
use crate::index::Indexer;
use crate::model::{EntityType, Node};
use crate::query::QueryEngine;

use prompts as prompt_handlers;
use resources as resource_handlers;
use tools::*;

/// MCP server backed by the p4k data index.
///
/// The `Indexer` contains a `rusqlite::Connection` which is `!Sync`, so it must
/// be wrapped in a `Mutex` to satisfy the `ServerHandler: Send + Sync` bound.
pub struct DataP4kServer {
    indexer: Arc<Mutex<Indexer>>,
    config: Arc<RwLock<Config>>,
    tool_router: ToolRouter<Self>,
}

impl DataP4kServer {
    /// Create a new server wrapping the given indexer and config.
    pub fn new(indexer: Indexer, config: Config) -> Self {
        Self {
            indexer: Arc::new(Mutex::new(indexer)),
            config: Arc::new(RwLock::new(config)),
            tool_router: Self::tool_router(),
        }
    }

    /// Start the MCP server on stdio transport.
    pub async fn run(self) -> Result<()> {
        let transport = transport::io::stdio();
        let server = self.serve(transport).await?;
        server.waiting().await?;
        Ok(())
    }

    // -------------------------------------------------------------------
    // Internal helpers — all acquire the Mutex lock
    // -------------------------------------------------------------------

    /// Run a closure with a borrowed QueryEngine.
    fn with_query_engine<F, R>(&self, f: F) -> R
    where
        F: FnOnce(QueryEngine<'_>) -> R,
    {
        let indexer = self.indexer.lock().unwrap();
        f(QueryEngine::new(&indexer))
    }

    /// Run a closure with a borrowed Indexer (for index/reindex).
    fn with_indexer<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Indexer) -> R,
    {
        let indexer = self.indexer.lock().unwrap();
        f(&indexer)
    }

    /// Resolve a string that might be a UUID or a class name into a list of nodes.
    fn resolve_entity(&self, input: &str) -> Result<Vec<Node>, String> {
        self.with_query_engine(|qe| {
            // Try parsing as UUID first
            if let Ok(uuid) = input.parse::<Uuid>() {
                return match qe.lookup_by_uuid(&uuid) {
                    Ok(Some(node)) => Ok(vec![node]),
                    Ok(None) => Err(format!("No entity found with UUID {uuid}")),
                    Err(e) => Err(format!("Lookup error: {e}")),
                };
            }

            // Fall back to class name lookup
            match qe.lookup_by_class_name(input) {
                Ok(nodes) if nodes.is_empty() => {
                    Err(format!("No entity found with class name '{input}'"))
                }
                Ok(nodes) => Ok(nodes),
                Err(e) => Err(format!("Lookup error: {e}")),
            }
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DataP4kServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new(
            "datap4k-mcp",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(
            "Star Citizen p4k game data server. Use 'search' for full-text search, \
             'lookup' by UUID/class name, 'traverse'/'path' for graph queries, \
             'diff' to compare versions, 'query'/'graph_query' for raw SQL/Datalog, \
             'locate'/'who_uses' for relationship lookups, 'index' to load data, \
             and 'status' for current index stats. \
             Resources: p4k://versions, p4k://categories, p4k://stats, p4k://schema. \
             Prompts: investigate-item, compare-versions, explore-location, trace-reward-chain.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(resource_handlers::list()))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let uri = request.uri.as_str();
        let json = match uri {
            "p4k://versions" => {
                self.with_query_engine(|qe| {
                    qe.status().map(|info| {
                        let items: Vec<serde_json::Value> = info
                            .versions
                            .iter()
                            .map(|v| {
                                serde_json::json!({
                                    "code": v.code,
                                    "build_number": v.build_number,
                                    "data_path": v.data_path,
                                })
                            })
                            .collect();
                        serde_json::to_string_pretty(&items)
                            .unwrap_or_else(|_| "[]".to_string())
                    })
                })
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            }
            "p4k://categories" => {
                self.with_query_engine(|qe| {
                    qe.status().map(|info| {
                        let map: serde_json::Map<String, serde_json::Value> = info
                            .category_counts
                            .into_iter()
                            .map(|(k, v)| (k, serde_json::json!(v)))
                            .collect();
                        serde_json::to_string_pretty(&map)
                            .unwrap_or_else(|_| "{}".to_string())
                    })
                })
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            }
            "p4k://stats" => {
                self.with_query_engine(|qe| {
                    qe.status().map(|info| {
                        let obj = serde_json::json!({
                            "total_entities": info.entity_count,
                            "version_count": info.versions.len(),
                            "versions": info.versions.iter().map(|v| &v.code).collect::<Vec<_>>(),
                        });
                        serde_json::to_string_pretty(&obj)
                            .unwrap_or_else(|_| "{}".to_string())
                    })
                })
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            }
            "p4k://schema" => {
                let variants = &[
                    "Ship", "Vehicle", "WeaponPersonal", "WeaponShip", "Component",
                    "Ammo", "Armor", "Consumable", "Commodity", "Mission", "Location",
                    "Shop", "NPC", "Loadout", "CraftingBlueprint", "Faction",
                    "Reputation", "LootTable", "AudioDef", "Material", "Tag", "Unknown",
                ];
                serde_json::to_string_pretty(variants)
                    .unwrap_or_else(|_| "[]".to_string())
            }
            _ => {
                return Err(ErrorData::resource_not_found(
                    format!("Unknown resource URI: {uri}"),
                    None,
                ));
            }
        };

        Ok(ReadResourceResult::new(vec![
            resource_handlers::text_contents(uri, json),
        ]))
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult::with_all_items(prompt_handlers::list()))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        let args = request.arguments.as_ref();
        let get_arg = |key: &str| -> Option<String> {
            args.and_then(|m| m.get(key))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };

        match request.name.as_str() {
            "investigate-item" => {
                let item_name = get_arg("item_name").unwrap_or_else(|| "<item>".to_string());
                Ok(prompt_handlers::investigate_item(&item_name))
            }
            "compare-versions" => {
                let version_a = get_arg("version_a").unwrap_or_else(|| "<version_a>".to_string());
                let version_b = get_arg("version_b").unwrap_or_else(|| "<version_b>".to_string());
                let category = get_arg("category");
                Ok(prompt_handlers::compare_versions(&version_a, &version_b, category.as_deref()))
            }
            "explore-location" => {
                let location = get_arg("location").unwrap_or_else(|| "<location>".to_string());
                Ok(prompt_handlers::explore_location(&location))
            }
            "trace-reward-chain" => {
                let mission_name = get_arg("mission_name").unwrap_or_else(|| "<mission>".to_string());
                Ok(prompt_handlers::trace_reward_chain(&mission_name))
            }
            _other => Err(ErrorData::method_not_found::<rmcp::model::GetPromptRequestMethod>()),
        }
    }
}

#[tool_router]
impl DataP4kServer {
    /// Full-text search across entity names and properties.
    #[tool(name = "search", description = "Full-text search across entity class names, record names, and properties. Supports FTS5 query syntax.")]
    async fn search(&self, Parameters(req): Parameters<SearchRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let limit = req.limit.unwrap_or(20).min(100);
        self.with_query_engine(|qe| {
            match qe.search(&req.query, limit) {
                Ok(nodes) => {
                    let text = format!("Found {} results for '{}':\n\n{}", nodes.len(), req.query, format_nodes(&nodes));
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Search failed: {e}"))])),
            }
        })
    }

    /// Look up entities by UUID or class name.
    #[tool(name = "lookup", description = "Look up a specific entity by its UUID or exact class name. Provide either uuid or class_name (not both).")]
    async fn lookup(&self, Parameters(req): Parameters<LookupRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        if let Some(uuid_str) = &req.uuid {
            let uuid: Uuid = uuid_str.parse().map_err(|e| {
                rmcp::ErrorData::invalid_params(format!("Invalid UUID: {e}"), None)
            })?;
            self.with_query_engine(|qe| {
                match qe.lookup_by_uuid(&uuid) {
                    Ok(Some(node)) => {
                        Ok(CallToolResult::success(vec![Content::text(format_nodes_detailed(&[node]))]))
                    }
                    Ok(None) => Ok(CallToolResult::error(vec![Content::text(format!("No entity found with UUID {uuid_str}"))])),
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Lookup error: {e}"))])),
                }
            })
        } else if let Some(class_name) = &req.class_name {
            self.with_query_engine(|qe| {
                match qe.lookup_by_class_name(class_name) {
                    Ok(nodes) if nodes.is_empty() => {
                        Ok(CallToolResult::error(vec![Content::text(format!("No entity found with class name '{class_name}'"))]))
                    }
                    Ok(nodes) => {
                        let text = format!("Found {} entities:\n\n{}", nodes.len(), format_nodes_detailed(&nodes));
                        Ok(CallToolResult::success(vec![Content::text(text)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Lookup error: {e}"))])),
                }
            })
        } else {
            Err(rmcp::ErrorData::invalid_params(
                "Either 'uuid' or 'class_name' must be provided",
                None,
            ))
        }
    }

    /// Traverse the graph from a starting entity.
    #[tool(name = "traverse", description = "Traverse the entity graph starting from a UUID. Returns all entities reachable within the given depth (default 2, max 5).")]
    async fn traverse(&self, Parameters(req): Parameters<TraverseRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let uuid: Uuid = req.uuid.parse().map_err(|e| {
            rmcp::ErrorData::invalid_params(format!("Invalid UUID: {e}"), None)
        })?;
        let depth = req.depth.unwrap_or(2).min(5);
        self.with_query_engine(|qe| {
            match qe.traverse(&uuid, depth) {
                Ok(nodes) => {
                    let text = format!(
                        "Traversal from {} (depth {}): {} nodes found\n\n{}",
                        req.uuid, depth, nodes.len(), format_nodes(&nodes)
                    );
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Traversal failed: {e}"))])),
            }
        })
    }

    /// Find shortest path between two entities.
    #[tool(name = "path", description = "Find the shortest path between two entities by UUID. Returns the chain of nodes connecting them (max depth 10).")]
    async fn path(&self, Parameters(req): Parameters<PathRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let from: Uuid = req.from.parse().map_err(|e| {
            rmcp::ErrorData::invalid_params(format!("Invalid 'from' UUID: {e}"), None)
        })?;
        let to: Uuid = req.to.parse().map_err(|e| {
            rmcp::ErrorData::invalid_params(format!("Invalid 'to' UUID: {e}"), None)
        })?;
        let max_depth = req.max_depth.unwrap_or(5).min(10);
        self.with_query_engine(|qe| {
            match qe.find_path(&from, &to, max_depth) {
                Ok(Some(nodes)) => {
                    let text = format!(
                        "Path from {} to {} ({} hops):\n\n{}",
                        req.from, req.to, nodes.len().saturating_sub(1), format_nodes(&nodes)
                    );
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Ok(None) => {
                    Ok(CallToolResult::error(vec![Content::text(format!(
                        "No path found between {} and {} within depth {}",
                        req.from, req.to, max_depth
                    ))]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Pathfinding failed: {e}"))])),
            }
        })
    }

    /// Compare an entity across two game versions.
    #[tool(name = "diff", description = "Compare an entity's properties between two game versions. Shows added, removed, and changed properties.")]
    async fn diff(&self, Parameters(req): Parameters<DiffRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        // Resolve the entity — could be UUID or class name
        let nodes = self.resolve_entity(&req.entity).map_err(|e| {
            rmcp::ErrorData::invalid_params(e, None)
        })?;

        // Find nodes matching each version
        let node_a = nodes.iter().find(|n| n.game_version == req.version_a);
        let node_b = nodes.iter().find(|n| n.game_version == req.version_b);

        match (node_a, node_b) {
            (Some(a), Some(b)) => {
                let mut lines = vec![format!(
                    "# Diff: {} ({} vs {})\n",
                    a.class_name, req.version_a, req.version_b
                )];

                // Collect all property keys
                let mut all_keys: Vec<&String> = a.properties.keys().chain(b.properties.keys()).collect();
                all_keys.sort();
                all_keys.dedup();

                let mut changed = 0;
                for key in &all_keys {
                    let val_a = a.properties.get(*key);
                    let val_b = b.properties.get(*key);
                    match (val_a, val_b) {
                        (Some(va), Some(vb)) if va != vb => {
                            lines.push(format!("  ~ {key}: {va} -> {vb}"));
                            changed += 1;
                        }
                        (Some(va), None) => {
                            lines.push(format!("  - {key}: {va}"));
                            changed += 1;
                        }
                        (None, Some(vb)) => {
                            lines.push(format!("  + {key}: {vb}"));
                            changed += 1;
                        }
                        _ => {} // unchanged
                    }
                }

                if changed == 0 {
                    lines.push("No property differences found.".to_string());
                } else {
                    lines.insert(1, format!("{changed} properties differ:\n"));
                }

                Ok(CallToolResult::success(vec![Content::text(lines.join("\n"))]))
            }
            (None, _) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Entity '{}' not found in version {}",
                req.entity, req.version_a
            ))])),
            (_, None) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Entity '{}' not found in version {}",
                req.entity, req.version_b
            ))])),
        }
    }

    /// Execute raw SQL against the SQLite index.
    #[tool(name = "query", description = "Execute a raw SQL query against the SQLite entity index. Returns tabular results.")]
    async fn query(&self, Parameters(req): Parameters<QueryRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        self.with_query_engine(|qe| {
            match qe.raw_sql(&req.sql) {
                Ok(rows) => {
                    if rows.is_empty() {
                        Ok(CallToolResult::success(vec![Content::text("Query returned no results.")]))
                    } else {
                        let text = rows
                            .iter()
                            .map(|row| row.join(" | "))
                            .collect::<Vec<_>>()
                            .join("\n");
                        Ok(CallToolResult::success(vec![Content::text(format!(
                            "{} rows returned:\n\n{}",
                            rows.len(),
                            text
                        ))]))
                    }
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("SQL error: {e}"))])),
            }
        })
    }

    /// Execute raw Datalog/CozoScript against the graph.
    #[tool(name = "graph_query", description = "Execute a raw CozoScript/Datalog query against the Cozo graph index. Returns JSON results.")]
    async fn graph_query(&self, Parameters(req): Parameters<GraphQueryRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        self.with_query_engine(|qe| {
            match qe.raw_datalog(&req.query) {
                Ok(json) => Ok(CallToolResult::success(vec![Content::text(json)])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Datalog error: {e}"))])),
            }
        })
    }

    /// Find locations where an entity can be found.
    #[tool(name = "locate", description = "Find where an entity is located in the game world. Smart multi-hop: searches direct locations first, then follows NPC/loadout chains to find spawn locations.")]
    async fn locate(&self, Parameters(req): Parameters<LocateRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let nodes = self.resolve_entity(&req.entity).map_err(|e| {
            rmcp::ErrorData::invalid_params(e, None)
        })?;

        let mut locations: Vec<Node> = Vec::new();
        let mut via_npcs: Vec<String> = Vec::new(); // track which NPCs led us to locations

        self.with_query_engine(|qe| {
            // Phase 1: Direct location search (3 hops from the entity)
            for node in &nodes {
                match qe.traverse(&node.id, 3) {
                    Ok(reachable) => {
                        for n in reachable {
                            if matches!(n.entity_type, EntityType::Location | EntityType::Shop) {
                                locations.push(n);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Traverse failed for {}: {e}", node.id);
                    }
                }
            }

            // Phase 2: If no meaningful locations found, chain through NPCs/loadouts
            // Find NPCs that use this item, then find their spawn locations
            let meaningful_direct_locs: Vec<&Node> = locations.iter().filter(|l| {
                let cn = l.class_name.to_lowercase();
                !cn.contains("entitycomponent") && !cn.contains("harvestable")
                    && !cn.contains("sctransit") && cn != "default"
                    && !cn.contains("actionarea") && !cn.contains("subsumption")
                    && !cn.starts_with("old_") && !cn.contains("transitui")
                    && !cn.contains("transitdisplay") && !cn.contains("transitstopdisplay")
            }).collect();
            tracing::info!("locate phase 1: {} raw locations, {} meaningful", locations.len(), meaningful_direct_locs.len());
            if meaningful_direct_locs.is_empty() {
                // Phase 2: Find where this item can be acquired via faction-location heuristic.
                //
                // Use SQL to find all loadout names containing this item's class_name,
                // extract faction prefixes from those loadout names, then find locations
                // matching those factions. This is fast and avoids expensive graph traversal.

                let item_class = nodes.first().map(|n| n.class_name.clone()).unwrap_or_default();

                // Find loadouts that reference this item by searching class_name patterns
                let sql = format!(
                    "SELECT DISTINCT class_name FROM entities WHERE entity_type = 'Loadout' \
                     AND class_name LIKE '%{}%' LIMIT 100",
                    item_class.replace('\'', "''")
                );

                let mut faction_prefixes: std::collections::HashSet<String> = std::collections::HashSet::new();

                if let Ok(rows) = qe.raw_sql(&sql) {
                    // If no loadouts reference the item directly, try broader search:
                    // find loadouts that share the same "default" SOC location (depth 1 from item)
                    let loadout_names: Vec<String> = rows.iter()
                        .filter_map(|r| r.first().cloned())
                        .collect();

                    // Also search for loadout class_names that match via `who_uses` pattern
                    // (loadouts connected via graph edges)
                    for node in &nodes {
                        if let Ok(reachable) = qe.traverse(&node.id, 3) {
                            for n in reachable {
                                if n.entity_type == EntityType::Loadout {
                                    let cn_lower = n.class_name.to_lowercase();
                                    for prefix in &["asd", "ninetails", "headhunter", "roughandready",
                                                   "salamander", "shatteredblade", "citizensforprosperity",
                                                   "xenothreat"] {
                                        if cn_lower.starts_with(prefix) {
                                            if !faction_prefixes.contains(*prefix) {
                                                via_npcs.push(n.class_name.clone());
                                            }
                                            faction_prefixes.insert(prefix.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Also extract factions from loadout names found via SQL
                    for name in &loadout_names {
                        let cn_lower = name.to_lowercase();
                        for prefix in &["asd", "ninetails", "headhunter", "roughandready",
                                       "salamander", "shatteredblade", "citizensforprosperity",
                                       "xenothreat"] {
                            if cn_lower.starts_with(prefix) {
                                faction_prefixes.insert(prefix.to_string());
                            }
                        }
                    }
                }

                // Find locations matching discovered factions via SQL
                tracing::info!("locate phase 2: {} factions found: {:?}", faction_prefixes.len(), faction_prefixes);
                if !faction_prefixes.is_empty() {
                    let conditions: Vec<String> = faction_prefixes.iter()
                        .map(|f| format!("source_path LIKE '%{f}_%'"))
                        .collect();
                    let sql = format!(
                        "SELECT uuid FROM entities WHERE entity_type = 'Location' AND ({}) LIMIT 200",
                        conditions.join(" OR ")
                    );
                    if let Ok(rows) = qe.raw_sql(&sql) {
                        for row in &rows {
                            if let Some(uuid_str) = row.first() {
                                if let Ok(uuid) = uuid_str.parse::<uuid::Uuid>() {
                                    if let Ok(Some(loc)) = qe.lookup_by_uuid(&uuid) {
                                        locations.push(loc);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        // Deduplicate and filter out generic metadata locations
        locations.sort_by(|a, b| a.id.cmp(&b.id));
        locations.dedup_by(|a, b| a.id == b.id);

        // Filter out noise: metadata files, harvestable components, transit peripherals
        let meaningful: Vec<&Node> = locations.iter()
            .filter(|l| {
                let cn = l.class_name.to_lowercase();
                !cn.contains("entitycomponent") && !cn.contains("harvestablecomponent")
                    && !cn.contains("sctransit") && cn != "default"
                    && !cn.contains("actionarea") && !cn.contains("subsumption")
                    && !cn.starts_with("old_") && !cn.contains("transitui")
            })
            .collect();

        if meaningful.is_empty() {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "No specific location data found for '{}'.",
                req.entity
            ))]))
        } else {
            let mut text = format!(
                "Found {} locations for '{}':\n\n",
                meaningful.len(),
                req.entity,
            );
            if !via_npcs.is_empty() {
                text.push_str(&format!(
                    "(via NPCs: {})\n\n",
                    via_npcs.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            text.push_str(&format_nodes(&meaningful.into_iter().cloned().collect::<Vec<_>>()));
            Ok(CallToolResult::success(vec![Content::text(text)]))
        }
    }

    /// Find what uses a given item.
    #[tool(name = "who_uses", description = "Find which NPCs, loadouts, or ships reference a given item. Searches graph relationships for NPC/Loadout/Ship nodes connected to the entity.")]
    async fn who_uses(&self, Parameters(req): Parameters<WhoUsesRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let nodes = self.resolve_entity(&req.item).map_err(|e| {
            rmcp::ErrorData::invalid_params(e, None)
        })?;

        let mut users: Vec<Node> = Vec::new();
        self.with_query_engine(|qe| {
            for node in &nodes {
                match qe.traverse(&node.id, 3) {
                    Ok(reachable) => {
                        for n in reachable {
                            if matches!(
                                n.entity_type,
                                EntityType::NPC | EntityType::Loadout | EntityType::Ship | EntityType::Vehicle
                            ) {
                                users.push(n);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Traverse failed for {}: {e}", node.id);
                    }
                }
            }
        });

        // Deduplicate by UUID
        users.sort_by(|a, b| a.id.cmp(&b.id));
        users.dedup_by(|a, b| a.id == b.id);

        if users.is_empty() {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "No NPCs, loadouts, or ships found that use '{}'.",
                req.item
            ))]))
        } else {
            let text = format!(
                "Found {} entities that use '{}':\n\n{}",
                users.len(),
                req.item,
                format_nodes(&users)
            );
            Ok(CallToolResult::success(vec![Content::text(text)]))
        }
    }

    /// Trigger indexing of a p4k data directory.
    #[tool(name = "index", description = "Index an extracted p4k data directory. Parses game data files and adds them to the search index. Set reindex=true to replace existing data for a version.")]
    async fn index_dir(&self, Parameters(req): Parameters<IndexRequest>) -> Result<CallToolResult, rmcp::ErrorData> {
        let version = req.version.unwrap_or_else(|| {
            let dirname = std::path::Path::new(&req.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            crate::model::version_from_dirname(&dirname)
                .map(|(v, _)| v)
                .unwrap_or(dirname)
        });

        let reindex = req.reindex.unwrap_or(false);

        let stats = self.with_indexer(|indexer| {
            if reindex {
                indexer.reindex(&req.path, &version, "auto")
            } else {
                indexer.index_directory(&req.path, &version, "auto")
            }
        });

        match stats {
            Ok(stats) => {
                // Update config with the new version
                if let Ok(mut config) = self.config.write() {
                    config.add_version(&req.path, &version, "auto");
                    if let Err(e) = config.save() {
                        tracing::warn!("Failed to save config: {e}");
                    }
                }

                let text = format!(
                    "Indexed '{}' as version '{}':\n- {} entities\n- {} edges\n- {} warnings\n- Parsers: {}",
                    req.path,
                    stats.version,
                    stats.node_count,
                    stats.edge_count,
                    stats.warning_count,
                    stats.parsers_used.join(", ")
                );
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Indexing failed: {e}"))])),
        }
    }

    /// Show current index status and statistics.
    #[tool(name = "status", description = "Show index status: total entity count, indexed versions, and entity counts by category.")]
    async fn status(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        self.with_query_engine(|qe| {
            match qe.status() {
                Ok(info) => {
                    let mut lines = vec![
                        "# datap4k-mcp Index Status\n".to_string(),
                        format!("**Total entities:** {}\n", info.entity_count),
                    ];

                    if info.versions.is_empty() {
                        lines.push("**Versions:** none indexed yet\n".to_string());
                    } else {
                        lines.push("**Versions:**".to_string());
                        for v in &info.versions {
                            let build = v
                                .build_number
                                .as_deref()
                                .map(|b| format!(" (build {b})"))
                                .unwrap_or_default();
                            lines.push(format!("- {}{build} -- {}", v.code, v.data_path));
                        }
                        lines.push(String::new());
                    }

                    if !info.category_counts.is_empty() {
                        lines.push("**Entity counts by type:**".to_string());
                        for (cat, count) in &info.category_counts {
                            lines.push(format!("- {cat}: {count}"));
                        }
                    }

                    Ok(CallToolResult::success(vec![Content::text(lines.join("\n"))]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!("Status query failed: {e}"))])),
            }
        })
    }
}
