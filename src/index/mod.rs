// Indexer orchestration
pub mod cozo;
pub mod sqlite;

use std::path::Path;

use anyhow::{Context, Result};

use crate::config::Config;
use crate::model::EntityType;
use crate::parser;

/// Statistics returned after indexing a directory.
#[derive(Debug)]
pub struct IndexStats {
    pub version: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub warning_count: usize,
    pub parsers_used: Vec<String>,
}

/// Orchestrates parsing and storage across both the SQLite and Cozo indices.
pub struct Indexer {
    pub sqlite: sqlite::SqliteIndex,
    pub graph: cozo::CozoGraph,
}

impl Indexer {
    /// Open (or create) both backing stores under `config.index.path`.
    ///
    /// Creates the index directory if it does not already exist.
    pub fn open(config: &Config) -> Result<Self> {
        let index_path = Path::new(&config.index.path);
        std::fs::create_dir_all(index_path)
            .with_context(|| format!("failed to create index dir: {}", index_path.display()))?;

        let sqlite_path = index_path.join("entities.db");
        let cozo_path = index_path.join("graph.db");

        let sqlite = sqlite::SqliteIndex::open(&sqlite_path)
            .context("failed to open SQLite index")?;
        let graph = cozo::CozoGraph::open(&cozo_path)
            .context("failed to open Cozo graph index")?;

        Ok(Self { sqlite, graph })
    }

    /// Parse a data directory and insert the results into both stores.
    ///
    /// When `parser_name` is `"auto"`, all parsers that detect the directory
    /// are run and their results are merged. Returns an error if no parsers
    /// can handle the directory.
    pub fn index_directory(
        &self,
        data_path: &str,
        version: &str,
        parser_name: &str,
    ) -> Result<IndexStats> {
        let path = Path::new(data_path);

        // Resolve parsers
        let parsers: Vec<Box<dyn parser::P4kParser>> = if parser_name == "auto" {
            let detected = parser::detect_parsers(path);
            if detected.is_empty() {
                anyhow::bail!(
                    "no parsers could detect the directory layout at {}",
                    data_path
                );
            }
            detected
        } else {
            let all = parser::all_parsers();
            let found: Vec<_> = all.into_iter().filter(|p| p.name() == parser_name).collect();
            if found.is_empty() {
                anyhow::bail!("unknown parser: {parser_name}");
            }
            found
        };

        let parsers_used: Vec<String> = parsers.iter().map(|p| p.name().to_string()).collect();

        // Run parsers and merge results
        let mut merged = crate::model::ParseResult::new();
        for p in &parsers {
            tracing::info!("Running parser '{}' on {}", p.name(), data_path);
            let result = p.parse(path, version)
                .with_context(|| format!("parser '{}' failed on {}", p.name(), data_path))?;
            merged.merge(result);
        }

        let warning_count = merged.warnings.len();

        tracing::info!(
            "Parsed {}: {} nodes, {} edges (pre-resolution), {} warnings",
            version, merged.nodes.len(), merged.edges.len(), warning_count
        );

        // --- Edge resolution ---
        // Build lookup maps from parsed entities
        // Case-insensitive class name lookup (file:// paths are often lowercase)
        let class_name_to_uuid: std::collections::HashMap<String, uuid::Uuid> = merged
            .nodes
            .iter()
            .map(|n| (n.class_name.to_lowercase(), n.id))
            .collect();

        let known_uuids: std::collections::HashSet<uuid::Uuid> = merged
            .nodes
            .iter()
            .map(|n| n.id)
            .collect();

        let mut resolved_count = 0usize;
        let mut dropped_count = 0usize;

        // Resolve edges: match unresolved targets to real entity UUIDs
        merged.edges.retain_mut(|edge| {
            let nil = uuid::Uuid::nil();

            // Try to resolve by class name from edge properties
            let resolved = if edge.target_id == nil {
                // file:// ref edges — try to resolve path to a class name
                if let Some(serde_json::Value::String(file_ref)) = edge.properties.get("file_ref") {
                    // Extract class name from file path: last segment without extension
                    let class_name = file_ref
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .trim_end_matches(".json")
                        .to_lowercase();
                    class_name_to_uuid.get(&class_name).copied()
                } else {
                    None
                }
            } else {
                // Check if the target is a v5-hashed class name (from loadouts/SOC)
                // by looking for item_class_name or entity_class in edge properties
                let class_name = edge
                    .properties
                    .get("item_class_name")
                    .or_else(|| edge.properties.get("entity_class"))
                    .and_then(|v| v.as_str());

                if let Some(cn) = class_name {
                    let cn_lower = cn.to_lowercase();
                    let v5_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, cn.as_bytes());
                    if edge.target_id == v5_uuid {
                        // This is a v5-hashed placeholder — resolve to real entity UUID
                        class_name_to_uuid.get(&cn_lower).copied()
                    } else {
                        // Target UUID is a real _RecordId_ — check if it exists
                        if known_uuids.contains(&edge.target_id) {
                            None // already resolved, keep as-is
                        } else {
                            // Target doesn't exist — try class name resolution
                            class_name_to_uuid.get(&cn_lower).copied()
                        }
                    }
                } else {
                    // No class name in properties — check if target exists as entity
                    if known_uuids.contains(&edge.target_id) {
                        None // target exists, keep as-is
                    } else {
                        None // can't resolve, will be dropped below
                    }
                }
            };

            if let Some(real_uuid) = resolved {
                edge.target_id = real_uuid;
                resolved_count += 1;
                true
            } else if edge.target_id == nil {
                dropped_count += 1;
                false // drop unresolvable nil-target edges
            } else if !known_uuids.contains(&edge.target_id) {
                dropped_count += 1;
                false // drop edges to non-existent entities
            } else {
                true // keep edges with valid targets
            }
        });

        let node_count = merged.nodes.len();
        let edge_count = merged.edges.len();

        tracing::info!(
            "Edge resolution: {} resolved, {} dropped, {} remaining",
            resolved_count, dropped_count, edge_count
        );

        // Insert nodes into SQLite
        self.sqlite.insert_nodes(&merged.nodes)
            .context("failed to insert nodes into SQLite")?;

        // Register the version
        self.sqlite.add_version(version, None, data_path)
            .context("failed to add version to SQLite")?;

        // Insert entities into Cozo in batches of 5000
        let entity_tuples: Vec<(uuid::Uuid, &str, &str, &str)> = merged.nodes
            .iter()
            .map(|n| (n.id, n.class_name.as_str(), n.entity_type.as_str(), n.game_version.as_str()))
            .collect();

        let entity_chunks = entity_tuples.chunks(5000);
        let entity_total = entity_chunks.len();
        for (i, chunk) in entity_tuples.chunks(5000).enumerate() {
            self.graph.insert_entities(chunk)
                .context("failed to insert entities into Cozo")?;
            if (i + 1) % 5 == 0 || i + 1 == entity_total {
                tracing::info!("  graph entities: {}/{}", (i + 1) * 5000.min(entity_tuples.len() - i * 5000 + i * 5000), entity_tuples.len());
            }
        }

        // Insert edges into Cozo in batches of 5000
        let edge_tuples: Vec<(uuid::Uuid, uuid::Uuid, &str, &str)> = merged.edges
            .iter()
            .map(|e| (e.source_id, e.target_id, e.label.as_str(), e.source_field.as_str()))
            .collect();

        let edge_total_chunks = (edge_tuples.len() + 4999) / 5000;
        for (i, chunk) in edge_tuples.chunks(5000).enumerate() {
            self.graph.insert_edges(chunk)
                .context("failed to insert edges into Cozo")?;
            if (i + 1) % 10 == 0 || i + 1 == edge_total_chunks {
                tracing::info!("  graph edges: batch {}/{}", i + 1, edge_total_chunks);
            }
        }

        tracing::info!(
            "Indexed {}: {} entities stored, {} edges stored",
            version, node_count, edge_count
        );

        Ok(IndexStats {
            version: version.to_string(),
            node_count,
            edge_count,
            warning_count,
            parsers_used,
        })
    }

    /// Drop all data for `version` from both stores, then re-index.
    pub fn reindex(
        &self,
        data_path: &str,
        version: &str,
        parser_name: &str,
    ) -> Result<IndexStats> {
        tracing::info!("Dropping version {} before reindex", version);
        self.sqlite.drop_version(version)
            .context("failed to drop version from SQLite")?;
        self.graph.drop_version(version)
            .context("failed to drop version from Cozo")?;
        self.index_directory(data_path, version, parser_name)
    }

    /// Return entity counts grouped by entity type across all versions.
    pub fn category_counts_all(&self) -> Result<Vec<(EntityType, i64)>> {
        // Use a raw SQL query to get counts across all versions
        let rows = self.sqlite.execute_raw(
            "SELECT entity_type, COUNT(*) FROM entities GROUP BY entity_type ORDER BY entity_type"
        )?;

        let counts = rows
            .iter()
            .filter_map(|row| {
                if row.len() >= 2 {
                    let et: crate::model::EntityType = row[0].parse().ok()?;
                    let count: i64 = row[1].parse().ok()?;
                    Some((et, count))
                } else {
                    None
                }
            })
            .collect();

        Ok(counts)
    }
}
