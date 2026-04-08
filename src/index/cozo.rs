use std::path::Path;

use anyhow::{Context, Result};
use cozo::{DataValue, DbInstance, NamedRows, ScriptMutability};

/// Graph index backed by Cozo (a Datalog-native embedded database).
///
/// Stores entities as nodes and relationships as directed edges, enabling
/// multi-hop traversals and shortest-path queries over the game entity graph.
pub struct CozoGraph {
    db: DbInstance,
}

impl CozoGraph {
    /// Open a sled-backed Cozo database at the given path and create the schema.
    pub fn open(path: &Path) -> Result<Self> {
        let db = DbInstance::new("sled", path.to_string_lossy().as_ref(), "")
            .map_err(|e| anyhow::anyhow!("failed to open Cozo sled DB: {e}"))?;
        let graph = Self { db };
        graph.create_schema()?;
        Ok(graph)
    }

    /// Open an in-memory Cozo database. Useful for tests.
    pub fn open_in_memory() -> Result<Self> {
        let db = DbInstance::new("mem", "", "")
            .map_err(|e| anyhow::anyhow!("failed to open Cozo mem DB: {e}"))?;
        let graph = Self { db };
        graph.create_schema()?;
        Ok(graph)
    }

    /// Create the stored relations (entity and edge tables) if they don't already exist.
    fn create_schema(&self) -> Result<()> {
        // Create entity relation
        let res = self.db.run_script(
            ":create entity {uuid: String => class_name: String, entity_type: String, version: String}",
            Default::default(),
            ScriptMutability::Mutable,
        );
        // Ignore "already exists" errors
        if let Err(e) = &res {
            let msg = e.to_string();
            if !msg.contains("already exists") && !msg.contains("conflicts with an existing one") {
                return Err(anyhow::anyhow!("failed to create entity relation: {msg}"));
            }
        }

        // Create edge relation
        let res = self.db.run_script(
            ":create edge {src: String, dst: String => label: String, source_field: String}",
            Default::default(),
            ScriptMutability::Mutable,
        );
        if let Err(e) = &res {
            let msg = e.to_string();
            if !msg.contains("already exists") && !msg.contains("conflicts with an existing one") {
                return Err(anyhow::anyhow!("failed to create edge relation: {msg}"));
            }
        }

        Ok(())
    }

    /// Return the total number of entities in the graph.
    pub fn entity_count(&self) -> Result<usize> {
        let result = self.run_query("?[count(uuid)] := *entity{uuid}")?;
        if let Some(row) = result.rows.first() {
            if let Some(val) = row.first() {
                if let Some(n) = val.get_int() {
                    return Ok(n as usize);
                }
            }
        }
        Ok(0)
    }

    /// Insert a batch of entities into the graph.
    ///
    /// Each tuple is `(uuid, class_name, entity_type, version)`.
    pub fn insert_entities(&self, entities: &[(uuid::Uuid, &str, &str, &str)]) -> Result<()> {
        if entities.is_empty() {
            return Ok(());
        }

        // Build inline data rows
        let rows: Vec<String> = entities
            .iter()
            .map(|(uuid, class_name, entity_type, version)| {
                format!(
                    "[\"{}\", \"{}\", \"{}\", \"{}\"]",
                    uuid, class_name, entity_type, version
                )
            })
            .collect();

        let query = format!(
            "?[uuid, class_name, entity_type, version] <- [{}]\n\
             :put entity {{uuid => class_name, entity_type, version}}",
            rows.join(", ")
        );

        self.run_mutation(&query)
            .context("failed to insert entities")?;
        Ok(())
    }

    /// Insert a batch of edges into the graph.
    ///
    /// Each tuple is `(src_uuid, dst_uuid, label, source_field)`.
    pub fn insert_edges(&self, edges: &[(uuid::Uuid, uuid::Uuid, &str, &str)]) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }

        let rows: Vec<String> = edges
            .iter()
            .map(|(src, dst, label, source_field)| {
                format!("[\"{}\", \"{}\", \"{}\", \"{}\"]", src, dst, label, source_field)
            })
            .collect();

        let query = format!(
            "?[src, dst, label, source_field] <- [{}]\n\
             :put edge {{src, dst => label, source_field}}",
            rows.join(", ")
        );

        self.run_mutation(&query)
            .context("failed to insert edges")?;
        Ok(())
    }

    /// Traverse the graph from a starting entity, returning all UUIDs reachable
    /// within `depth` hops (excluding the start node itself).
    pub fn traverse(&self, start: &uuid::Uuid, depth: u32) -> Result<Vec<String>> {
        if depth == 0 {
            return Ok(vec![]);
        }

        // Build an iterative hop query. Each hop extends the frontier by one edge.
        // We collect all unique nodes discovered at each hop level.
        let start_str = start.to_string();
        let mut lines = Vec::new();

        // Seed: the start node (each row must be a list)
        lines.push(format!("start[uuid] <- [[\"{start_str}\"]]"));

        // First hop — follow edges in both directions
        lines.push("hop_1[dst] := start[src], *edge{src, dst}".to_string());
        lines.push("hop_1[src] := start[dst], *edge{src, dst}".to_string());

        // Subsequent hops
        for i in 2..=depth {
            lines.push(format!(
                "hop_{i}[dst] := hop_{}[src], *edge{{src, dst}}",
                i - 1
            ));
            lines.push(format!(
                "hop_{i}[src] := hop_{}[dst], *edge{{src, dst}}",
                i - 1
            ));
        }

        // Collect all reachable nodes (union of all hops), excluding the start node
        // Filter to only include nodes that exist as entities in the graph
        for i in 1..=depth {
            lines.push(format!("reachable[uuid] := hop_{i}[uuid]"));
        }

        lines.push(format!(
            "?[uuid] := reachable[uuid], uuid != \"{}\", *entity{{uuid}}",
            start_str
        ));

        let query = lines.join("\n");
        tracing::debug!("traverse query:\n{query}");
        let result = self.run_query(&query)?;

        let uuids = result
            .rows
            .iter()
            .filter_map(|row| row.first().and_then(|v| v.get_str()).map(|s| s.to_string()))
            .collect();

        Ok(uuids)
    }

    /// Find the shortest path between two entities, up to `max_depth` hops.
    ///
    /// Returns `None` if no path exists within the depth limit.
    /// Returns `Some(vec)` with the UUID strings of each node along the path
    /// (including both `from` and `to`).
    pub fn find_path(
        &self,
        from: &uuid::Uuid,
        to: &uuid::Uuid,
        max_depth: u32,
    ) -> Result<Option<Vec<String>>> {
        let from_str = from.to_string();
        let to_str = to.to_string();

        // Same node
        if from_str == to_str {
            return Ok(Some(vec![from_str]));
        }

        // Try iterative widening: check depth 1, then 2, etc.
        // At each depth, build a query that accumulates the path as a list.
        for depth in 1..=max_depth {
            let path = self.try_find_path_at_depth(&from_str, &to_str, depth)?;
            if let Some(p) = path {
                return Ok(Some(p));
            }
        }

        Ok(None)
    }

    /// Attempt to find a path of exactly `depth` hops from `from` to `to`.
    fn try_find_path_at_depth(
        &self,
        from: &str,
        to: &str,
        depth: u32,
    ) -> Result<Option<Vec<String>>> {
        let mut lines = Vec::new();

        // Seed with start node and initial path
        lines.push(format!(
            "hop_0[uuid, path] <- [[\"{from}\", [\"{from}\"]]]"
        ));

        // Build each hop level, accumulating the path
        for i in 1..=depth {
            lines.push(format!(
                "hop_{i}[dst, path] := hop_{}[src, prev_path], *edge{{src, dst}}, path = append(prev_path, dst)",
                i - 1
            ));
        }

        // Check if the target was reached at the final hop
        lines.push(format!(
            "found[path] := hop_{depth}[uuid, path], uuid = \"{to}\""
        ));

        lines.push("?[path] := found[path]".to_string());
        lines.push(":limit 1".to_string());

        let query = lines.join("\n");
        let result = self.run_query(&query)?;

        if let Some(row) = result.rows.first() {
            if let Some(DataValue::List(path_values)) = row.first() {
                let path: Vec<String> = path_values
                    .iter()
                    .filter_map(|v| v.get_str().map(|s| s.to_string()))
                    .collect();
                if !path.is_empty() {
                    return Ok(Some(path));
                }
            }
        }

        Ok(None)
    }

    /// Execute a raw CozoScript/Datalog query and return the results.
    pub fn execute_raw(&self, query: &str) -> Result<NamedRows> {
        self.db
            .run_script(query, Default::default(), ScriptMutability::Immutable)
            .map_err(|e| anyhow::anyhow!("raw query failed: {e}"))
    }

    /// Remove all entities (and their edges) for a given game version.
    pub fn drop_version(&self, version: &str) -> Result<()> {
        // First, find all entity UUIDs for this version
        let query = format!(
            "?[uuid] := *entity{{uuid, version}}, version = \"{}\"",
            version
        );
        let result = self.run_query(&query)?;

        let uuids: Vec<String> = result
            .rows
            .iter()
            .filter_map(|row| row.first().and_then(|v| v.get_str()).map(|s| s.to_string()))
            .collect();

        if uuids.is_empty() {
            return Ok(());
        }

        // Remove edges where src or dst is one of these entities
        let remove_edges = format!(
            "uuids[uuid] <- [{}]\n\
             ?[src, dst] := *edge{{src, dst}}, uuids[src]\n\
             ?[src, dst] := *edge{{src, dst}}, uuids[dst]\n\
             :rm edge {{src, dst}}",
            uuids
                .iter()
                .map(|u| format!("[\"{u}\"]"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.run_mutation(&remove_edges)
            .context("failed to remove edges for version")?;

        // Remove entities
        let remove_entities = format!(
            "?[uuid] := *entity{{uuid, version}}, version = \"{}\"\n\
             :rm entity {{uuid}}",
            version
        );
        self.run_mutation(&remove_entities)
            .context("failed to remove entities for version")?;

        Ok(())
    }

    /// Run an immutable (read) query.
    fn run_query(&self, query: &str) -> Result<NamedRows> {
        self.db
            .run_script(query, Default::default(), ScriptMutability::Immutable)
            .map_err(|e| anyhow::anyhow!("query failed: {e}"))
    }

    /// Run a mutable (write) query.
    fn run_mutation(&self, query: &str) -> Result<NamedRows> {
        self.db
            .run_script(query, Default::default(), ScriptMutability::Mutable)
            .map_err(|e| anyhow::anyhow!("mutation failed: {e}"))
    }
}
