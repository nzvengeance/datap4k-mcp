// Query engine
use anyhow::Result;

use crate::index::sqlite::VersionInfo;
use crate::index::Indexer;
use crate::model::{EntityType, Node};

/// Aggregated status information about the index.
#[derive(Debug)]
pub struct StatusInfo {
    pub entity_count: i64,
    pub versions: Vec<VersionInfo>,
    pub category_counts: Vec<(String, i64)>,
}

/// Routes queries to the appropriate backing store (SQLite or Cozo).
pub struct QueryEngine<'a> {
    indexer: &'a Indexer,
}

impl<'a> QueryEngine<'a> {
    /// Create a new query engine backed by `indexer`.
    pub fn new(indexer: &'a Indexer) -> Self {
        Self { indexer }
    }

    /// Full-text search across class name, record name, and properties.
    /// Routes to SQLite FTS5.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Node>> {
        self.indexer.sqlite.search(query, limit)
    }

    /// Look up a single entity by its UUID.
    /// Routes to SQLite.
    pub fn lookup_by_uuid(&self, uuid: &uuid::Uuid) -> Result<Option<Node>> {
        self.indexer.sqlite.lookup_by_uuid(uuid)
    }

    /// Return all entities with the given exact class name.
    /// Routes to SQLite.
    pub fn lookup_by_class_name(&self, class_name: &str) -> Result<Vec<Node>> {
        self.indexer.sqlite.lookup_by_class_name(class_name)
    }

    /// Return up to `limit` entities of the given type.
    /// Routes to SQLite.
    pub fn filter_by_type(&self, entity_type: EntityType, limit: usize) -> Result<Vec<Node>> {
        self.indexer.sqlite.filter_by_type(entity_type, limit)
    }

    /// Traverse the graph from `uuid` up to `depth` hops, returning all reachable nodes.
    ///
    /// Routes to Cozo for graph traversal, then enriches with full Node data from SQLite.
    pub fn traverse(&self, uuid: &uuid::Uuid, depth: u32) -> Result<Vec<Node>> {
        let uuid_strings = self.indexer.graph.traverse(uuid, depth)?;
        self.enrich_uuids(&uuid_strings)
    }

    /// Find the shortest path between `from` and `to` within `max_depth` hops.
    ///
    /// Routes to Cozo for pathfinding, then enriches with full Node data from SQLite.
    /// Returns `None` if no path exists within the depth limit.
    pub fn find_path(
        &self,
        from: &uuid::Uuid,
        to: &uuid::Uuid,
        max_depth: u32,
    ) -> Result<Option<Vec<Node>>> {
        let path_uuids = self.indexer.graph.find_path(from, to, max_depth)?;
        match path_uuids {
            None => Ok(None),
            Some(uuid_strings) => Ok(Some(self.enrich_uuids(&uuid_strings)?)),
        }
    }

    /// Execute arbitrary SQL and return results as string vectors.
    /// SQLite passthrough.
    pub fn raw_sql(&self, sql: &str) -> Result<Vec<Vec<String>>> {
        self.indexer.sqlite.execute_raw(sql)
    }

    /// Execute a raw Datalog/CozoScript query and return results as a JSON string.
    /// Cozo passthrough.
    pub fn raw_datalog(&self, query: &str) -> Result<String> {
        let named_rows = self.indexer.graph.execute_raw(query)?;
        // Represent the result as a JSON object with headers and rows
        let result = serde_json::json!({
            "headers": named_rows.headers,
            "rows": named_rows.rows.iter().map(|row| {
                row.iter().map(|v| format!("{v:?}")).collect::<Vec<_>>()
            }).collect::<Vec<_>>()
        });
        Ok(result.to_string())
    }

    /// Return a status summary: entity count, versions, and category counts.
    pub fn status(&self) -> Result<StatusInfo> {
        let entity_count = self.indexer.sqlite.entity_count()?;
        let versions = self.indexer.sqlite.list_versions()?;

        // Category counts across all versions
        let raw = self.indexer.sqlite.execute_raw(
            "SELECT entity_type, COUNT(*) FROM entities GROUP BY entity_type ORDER BY entity_type",
        )?;
        let category_counts: Vec<(String, i64)> = raw
            .into_iter()
            .filter_map(|row| {
                if row.len() >= 2 {
                    let count: i64 = row[1].parse().ok()?;
                    Some((row[0].clone(), count))
                } else {
                    None
                }
            })
            .collect();

        Ok(StatusInfo {
            entity_count,
            versions,
            category_counts,
        })
    }

    /// Look up a list of UUID strings in SQLite and return the full Node objects.
    ///
    /// UUIDs that cannot be parsed or are not found in SQLite are silently skipped.
    fn enrich_uuids(&self, uuid_strings: &[String]) -> Result<Vec<Node>> {
        let mut nodes = Vec::with_capacity(uuid_strings.len());
        for s in uuid_strings {
            if let Ok(uuid) = s.parse::<uuid::Uuid>() {
                if let Some(node) = self.indexer.sqlite.lookup_by_uuid(&uuid)? {
                    nodes.push(node);
                }
            }
        }
        Ok(nodes)
    }
}
