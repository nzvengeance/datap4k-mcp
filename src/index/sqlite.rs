use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use rusqlite::{Connection, params};

use crate::model::{EntityType, Node};

/// Metadata about an indexed game version.
pub struct VersionInfo {
    pub id: i64,
    pub code: String,
    pub build_number: Option<String>,
    pub data_path: String,
}

/// SQLite-backed index with FTS5 full-text search.
pub struct SqliteIndex {
    conn: Connection,
}

impl SqliteIndex {
    /// Open (or create) a SQLite database at the given path and initialise the schema.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let index = Self { conn };
        index.create_schema()?;
        Ok(index)
    }

    /// Open an in-memory SQLite database. Useful for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let index = Self { conn };
        index.create_schema()?;
        Ok(index)
    }

    fn create_schema(&self) -> Result<()> {
        self.conn.execute_batch("
            CREATE TABLE IF NOT EXISTS versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                code TEXT NOT NULL UNIQUE,
                build_number TEXT,
                data_path TEXT NOT NULL,
                indexed_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version_id INTEGER REFERENCES versions(id),
                parser_name TEXT NOT NULL,
                file_count INTEGER DEFAULT 0,
                indexed_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT NOT NULL,
                class_name TEXT NOT NULL,
                record_name TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                source TEXT NOT NULL,
                source_path TEXT NOT NULL,
                game_version TEXT NOT NULL,
                properties_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE INDEX IF NOT EXISTS idx_entities_uuid ON entities(uuid);
            CREATE INDEX IF NOT EXISTS idx_entities_class_name ON entities(class_name);
            CREATE INDEX IF NOT EXISTS idx_entities_entity_type ON entities(entity_type);
            CREATE INDEX IF NOT EXISTS idx_entities_game_version ON entities(game_version);

            CREATE VIRTUAL TABLE IF NOT EXISTS entity_fts USING fts5(
                class_name, record_name, properties_text,
                content='entities', content_rowid='id',
                tokenize='porter unicode61'
            );

            CREATE TRIGGER IF NOT EXISTS entities_ai AFTER INSERT ON entities BEGIN
                INSERT INTO entity_fts(rowid, class_name, record_name, properties_text)
                VALUES (new.id, new.class_name, new.record_name, new.properties_json);
            END;
        ")?;
        Ok(())
    }

    /// Insert a batch of nodes inside a single transaction.
    pub fn insert_nodes(&self, nodes: &[Node]) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO entities (uuid, class_name, record_name, entity_type, source, source_path, game_version, properties_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        )?;

        for node in nodes {
            let props_json = serde_json::to_string(&node.properties)?;
            stmt.execute(params![
                node.id.to_string(),
                node.class_name,
                node.record_name,
                node.entity_type.as_str(),
                node.source,
                node.source_path,
                node.game_version,
                props_json,
            ])?;
        }

        Ok(())
    }

    /// Return the total number of entities stored.
    pub fn entity_count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM entities",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Full-text search across class_name, record_name, and properties.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.uuid, e.class_name, e.record_name, e.entity_type, e.source, e.source_path, e.game_version, e.properties_json
             FROM entity_fts
             JOIN entities e ON entity_fts.rowid = e.id
             WHERE entity_fts MATCH ?1
             LIMIT ?2"
        )?;

        let nodes = stmt.query_map(params![query, limit as i64], row_to_node)?
            .collect::<rusqlite::Result<Vec<Node>>>()?;

        Ok(nodes)
    }

    /// Look up a single entity by its UUID. Returns `None` if not found.
    pub fn lookup_by_uuid(&self, uuid: &uuid::Uuid) -> Result<Option<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT uuid, class_name, record_name, entity_type, source, source_path, game_version, properties_json
             FROM entities WHERE uuid = ?1 LIMIT 1"
        )?;

        let mut rows = stmt.query_map(params![uuid.to_string()], row_to_node)?;
        match rows.next() {
            Some(result) => Ok(Some(result?)),
            None => Ok(None),
        }
    }

    /// Return all entities with the given exact class_name.
    pub fn lookup_by_class_name(&self, class_name: &str) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT uuid, class_name, record_name, entity_type, source, source_path, game_version, properties_json
             FROM entities WHERE class_name = ?1"
        )?;

        let nodes = stmt.query_map(params![class_name], row_to_node)?
            .collect::<rusqlite::Result<Vec<Node>>>()?;

        Ok(nodes)
    }

    /// Return up to `limit` entities of the given type.
    pub fn filter_by_type(&self, entity_type: EntityType, limit: usize) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(
            "SELECT uuid, class_name, record_name, entity_type, source, source_path, game_version, properties_json
             FROM entities WHERE entity_type = ?1 LIMIT ?2"
        )?;

        let nodes = stmt.query_map(params![entity_type.as_str(), limit as i64], row_to_node)?
            .collect::<rusqlite::Result<Vec<Node>>>()?;

        Ok(nodes)
    }

    /// Execute arbitrary SQL and return results as string vectors.
    pub fn execute_raw(&self, sql: &str) -> Result<Vec<Vec<String>>> {
        let mut stmt = self.conn.prepare(sql)?;
        let col_count = stmt.column_count();

        let rows = stmt.query_map([], |row| {
            let mut cells = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let value: rusqlite::types::Value = row.get(i)?;
                let text = match value {
                    rusqlite::types::Value::Null => String::from("NULL"),
                    rusqlite::types::Value::Integer(n) => n.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Text(s) => s,
                    rusqlite::types::Value::Blob(b) => format!("<blob {} bytes>", b.len()),
                };
                cells.push(text);
            }
            Ok(cells)
        })?
        .collect::<rusqlite::Result<Vec<Vec<String>>>>()?;

        Ok(rows)
    }

    /// Register a game version in the versions table.
    pub fn add_version(&self, code: &str, build_number: Option<&str>, data_path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO versions (code, build_number, data_path) VALUES (?1, ?2, ?3)
             ON CONFLICT(code) DO UPDATE SET build_number=excluded.build_number, data_path=excluded.data_path",
            params![code, build_number, data_path],
        )?;
        Ok(())
    }

    /// Return all registered versions.
    pub fn list_versions(&self) -> Result<Vec<VersionInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, code, build_number, data_path FROM versions ORDER BY id"
        )?;

        let versions = stmt.query_map([], |row| {
            Ok(VersionInfo {
                id: row.get(0)?,
                code: row.get(1)?,
                build_number: row.get(2)?,
                data_path: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<VersionInfo>>>()?;

        Ok(versions)
    }

    /// Remove a version and (conceptually) all its associated data.
    pub fn drop_version(&self, version: &str) -> Result<()> {
        self.conn.execute("DELETE FROM versions WHERE code = ?1", params![version])?;
        Ok(())
    }

    /// Return entity counts grouped by entity_type for a given game version.
    pub fn category_counts(&self, version: &str) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT entity_type, COUNT(*) FROM entities WHERE game_version = ?1 GROUP BY entity_type ORDER BY entity_type"
        )?;

        let counts = stmt.query_map(params![version], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<(String, i64)>>>()?;

        Ok(counts)
    }
}

/// Convert a SQLite row to a `Node`.
///
/// Expected column order: uuid, class_name, record_name, entity_type, source, source_path,
/// game_version, properties_json.
fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<Node> {
    let uuid_str: String = row.get(0)?;
    let id = uuid::Uuid::parse_str(&uuid_str)
        .unwrap_or_else(|_| uuid::Uuid::nil());

    let entity_type_str: String = row.get(3)?;
    let entity_type = EntityType::from_str(&entity_type_str)
        .unwrap_or(EntityType::Unknown);

    let props_json: String = row.get(7)?;
    let properties: HashMap<String, serde_json::Value> =
        serde_json::from_str(&props_json).unwrap_or_default();

    Ok(Node {
        id,
        class_name: row.get(1)?,
        record_name: row.get(2)?,
        entity_type,
        source: row.get(4)?,
        source_path: row.get(5)?,
        game_version: row.get(6)?,
        properties,
    })
}
