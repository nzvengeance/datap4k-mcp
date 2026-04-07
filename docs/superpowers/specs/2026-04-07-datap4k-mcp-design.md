# datap4k-mcp Design Spec

**Date:** 2026-04-07
**Status:** Approved
**Repo:** `nzvengeance/datap4k-mcp`
**Location:** `/home/gavin/my_other_repos/datap4k-mcp`

## Overview

A Rust MCP (Model Context Protocol) server that gives LLMs structured, queryable access to extracted Star Citizen game data (p4k). Indexes 217+ DataCore entity categories, 274K+ Server Object Container files, NPC loadouts, localization strings, and materials into a hybrid SQLite + Cozo storage layer. Supports keyword search, entity lookup, graph traversal, path discovery between entities, and cross-version diffing.

**Target audience:** The Star Citizen datamining community — anyone with extracted p4k data who wants to query it through an LLM.

## Architecture

```
+-----------------------------------------------------+
|                   datap4k-mcp                        |
|                                                      |
|  +----------------+  +----------------+              |
|  | scdatatools    |  | unp4k          |  <- Parser   |
|  | Parser (JSON)  |  | Parser (XML)   |    Plugins   |
|  +-------+--------+  +-------+--------+              |
|          |                    |                       |
|          v                    v                       |
|  +-------------------------------+                   |
|  |     Common Entity Model       | <- Normalized     |
|  |  (nodes, edges, properties)   |    representation |
|  +--------------+----------------+                   |
|                 |                                     |
|         +-------+-------+                            |
|         v               v                            |
|  +------------+  +------------+                      |
|  |  SQLite    |  |   Cozo     | <- Dual storage      |
|  |  (search,  |  |  (graph,   |                      |
|  |   lookup,  |  |   paths,   |                      |
|  |   bulk)    |  |   links)   |                      |
|  +-----+------+  +-----+------+                      |
|        |               |                             |
|        v               v                             |
|  +-------------------------------+                   |
|  |      Query Engine             | <- Routes queries |
|  |  (decides SQLite vs Cozo)     |    to right store |
|  +--------------+----------------+                   |
|                 |                                     |
|                 v                                     |
|  +-------------------------------+                   |
|  |      MCP Server Layer         | <- Tools,         |
|  |  (stdio transport, init)      |    Resources,     |
|  +-------------------------------+    Prompts        |
+-----------------------------------------------------+
```

### Key architectural decisions

- **Plugin parser system:** Each extraction format (scdatatools, unp4k) is a parser plugin that normalizes data into the Common Entity Model. Multiple parsers can be active simultaneously — a user can index both their StarFab extract and their unp4k extract for the same version and compare what each surfaces.
- **Hybrid storage:** SQLite for search, lookup, and bulk queries (FTS5 full-text search). Cozo (embedded Datalog graph DB backed by SQLite) for relationship traversal, path discovery, and connection finding. The Query Engine routes to the right store based on query type.
- **Persistent index:** Built once per game version, persisted to disk. First run indexes everything; subsequent runs are instant. New game versions are indexed on demand.
- **LLM-guided init:** No config file editing required. The MCP init flow guides the user through setup conversationally.

## Common Entity Model

The normalized representation that both parsers produce and both storage engines consume.

### Node (Entity)

```rust
struct Node {
    id: Uuid,                          // from game data, or generated
    class_name: String,                // e.g., "AEGS_Avenger_Titan"
    record_name: String,               // e.g., "EntityClassDefinition.AEGS_Avenger_Titan"
    entity_type: EntityType,           // Ship, Weapon, Component, etc.
    source: String,                    // "scdatatools" | "unp4k"
    source_path: String,              // original file path relative to data dir
    game_version: String,             // "4.7.0-live"
    properties: Map<String, Value>,   // all typed properties from the record
}
```

### Edge (Relationship)

```rust
struct Edge {
    source_id: Uuid,
    target_id: Uuid,
    label: String,             // "has_component", "mounts_weapon", "references", "child_of", etc.
    source_field: String,      // which field in the source record created this edge
    properties: Map<String, Value>,  // edge metadata (port name, slot size, etc.)
}
```

### EntityType enum

Coarse categories for filtering and display:

`Ship`, `Vehicle`, `WeaponPersonal`, `WeaponShip`, `Component`, `Ammo`, `Armor`, `Consumable`, `Commodity`, `Mission`, `Location`, `Shop`, `NPC`, `Loadout`, `CraftingBlueprint`, `Faction`, `Reputation`, `LootTable`, `AudioDef`, `Material`, `Tag`, `Unknown`

Parser plugins map format-specific structures into these types. When both parsers produce a node for the same game UUID, they coexist with different `source` tags — the query engine can merge or compare them.

### Edge label discovery

Parsers don't need to know all possible relationship types upfront:
- Any `file://` reference in DataCore JSON becomes an edge
- Any `<EntityRef>` or `itemName` in XML becomes an edge
- The label is inferred from the parent field name (e.g., a field called `quantumDrive` pointing to another record becomes a `has_quantum_drive` edge)

## Parser Plugin System

### Plugin trait

```rust
trait P4kParser: Send + Sync {
    fn name(&self) -> &str;                              // "scdatatools" | "unp4k"
    fn detect(&self, path: &Path) -> bool;                // can this parser handle this directory?
    fn parse(&self, path: &Path, version: &str) -> Result<ParseResult>;
}

struct ParseResult {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    warnings: Vec<ParseWarning>,  // unresolvable refs, unknown types, etc.
}
```

### scdatatools parser

Handles the output from scdatatools / StarFab (StarFab uses scdatatools under the hood):

- `DataCore/libs/foundry/records/**/*.json` — 57K+ DataForge entity records as JSON
- `Extracted/XML/**/*.xml` — 12K+ CryXmlB-converted XMLs (entity definitions, configs, libs)
- `Extracted/SOC_Unpacked/**/*.xml` — 274K+ Server Object Container files (world layout)
- `Extracted/Loadouts_Character_Converted/**/*.xml` — 2,854 NPC loadout hierarchies
- `Extracted/XML/Data/Localization/` — Localization strings (multi-language)
- `Extracted/CharDefs/**/*.cdf` — Character/object definitions
- `Extracted/Materials/**/*.mtl` — Material definitions
- `Extracted/Config/` — Engine/game configuration
- `file://` references between DataCore records become edges

### unp4k parser

Handles the output from unp4k / unforge:

- DataForge records serialized as XML (different structure from scdatatools JSON)
- CryXmlB files extracted as raw XML
- Same SOC / loadout / localization files (same source data, same output structure for non-DataCore files)

### Auto-detection

On init, the server walks the provided data directory. Each registered parser's `detect()` is called. All matching parsers run and tag their output with their `source` name.

### Warnings over failures

If a parser can't resolve a reference or hits an unknown record type, it emits a warning and continues. Partial coverage beats crashing on one malformed file out of 120K+.

## Storage Layer

### SQLite (search, lookup, bulk queries)

| Table | Purpose |
|-------|---------|
| `entities` | id, uuid, class_name, record_name, entity_type, source, source_path, game_version, properties_json |
| `locations` | SOC data — object containers, entity placements, spatial positions |
| `loadouts` | Character loadout hierarchy (NPC -> body -> items -> sub-items, with portName) |
| `localization` | String key -> translated text (multi-language) |
| `materials` | Material definitions |
| `entity_fts` | FTS5 virtual table over class_name, record_name, localization text |
| `versions` | id, code, build_number, indexed_at, data_path |
| `sources` | id, version_id, parser_name, file_count, indexed_at |

### Cozo (graph traversal, path discovery)

| Relation | Schema | What it connects |
|----------|--------|-----------------|
| `entity` | uuid, class_name, entity_type, version | All entities as nodes |
| `edge` | source_uuid, target_uuid, label, source_field | DataCore references between entities |
| `edge_props` | source_uuid, target_uuid, key, value | Properties on edges |
| `located_at` | entity_uuid, location_uuid, container_path | Entity -> SOC location (where in the world?) |
| `equipped_by` | item_uuid, npc_uuid, port_name | Item -> NPC loadout (who uses this?) |
| `contains` | parent_uuid, child_uuid, container_type | SOC container hierarchy (station -> room -> rack -> item) |

### Query routing

| Query type | Routes to | Example |
|-----------|-----------|---------|
| Keyword search | SQLite FTS5 | `search "avenger titan"` |
| UUID/class_name lookup | SQLite | `lookup uuid:97648869-...` |
| Bulk filter | SQLite | `query "SELECT * FROM entities WHERE entity_type = 'Ship'"` |
| Relationship traversal | Cozo | `traverse uuid:... depth:3` |
| Path discovery | Cozo (BFS) | `path from:uuid1 to:uuid2` |
| World location | Cozo | `locate uuid:...` (follows `located_at` edges) |
| NPC usage | Cozo | `who_uses uuid:...` (follows `equipped_by` edges) |
| Version diff | SQLite | `diff version:4.6 version:4.7 entity:uuid` |

### Graph power: connecting disconnected things

The graph enables multi-hop discovery that flat search can't do. Example: "What rewards does the CRU-L1 security faction give for bounty missions?"

```
faction -> reputation -> awardservice -> missiondata -> missiongiver -> located_at (SOC)
```

All 217 DataCore categories become graph nodes. Every reference between them becomes an edge. The LLM asks natural questions, the MCP routes to the right storage engine.

## MCP Interface

### Tools (11)

| Tool | Purpose | Routes to |
|------|---------|-----------|
| `search` | Keyword search across all entities, localization strings | SQLite FTS5 |
| `lookup` | Get full record by UUID, class_name, or record path | SQLite |
| `traverse` | Follow relationships from an entity, configurable depth | Cozo |
| `path` | Find how two entities are connected (shortest path) | Cozo BFS |
| `diff` | Compare entity between two game versions | SQLite |
| `query` | Raw SQL against the SQLite index | SQLite |
| `graph_query` | Raw Datalog against the Cozo graph | Cozo |
| `locate` | Where in the game world is this entity? | Cozo |
| `who_uses` | Which NPCs/loadouts reference this item? | Cozo |
| `index` | Trigger indexing of a data directory (or re-index) | Indexer |
| `status` | Show indexed versions, entity counts, index health | SQLite |

### Resources (4)

| URI | What it returns |
|-----|-----------------|
| `p4k://versions` | List of indexed game versions with stats |
| `p4k://{version}/categories` | All 217+ DataCore categories with entity counts |
| `p4k://{version}/stats` | Total entities, edges, locations, loadouts, languages |
| `p4k://{version}/schema` | Entity types and their property shapes |

### Prompts (4)

| Prompt | Purpose |
|--------|---------|
| `investigate-item` | Structured investigation: lookup -> relationships -> locations -> NPC usage -> version history |
| `compare-versions` | Guided diff: what changed between two patches for a category or specific entity |
| `explore-location` | What's at a specific game location: entities, NPCs, loot, shops |
| `trace-reward-chain` | Follow a mission -> reward -> loot table -> items chain end to end |

### Server description

> Query and explore extracted Star Citizen game data (p4k). Search 217+ entity categories, traverse relationships between items/ships/NPCs/locations/missions, compare game versions, and discover connections across the entire game database.

## Init Flow & Configuration

### First run experience

1. LLM connects to datap4k-mcp for the first time
2. Server detects no config exists -> returns a setup prompt
3. LLM asks user: "Where is your extracted p4k data?"
4. User provides path (e.g., `E:\SC Data\4.7.0-live.11518367` or "a new patch dropped and I just extracted 4.7.0-live.11518367")
5. Server auto-detects format via parser `detect()` functions
6. Server begins indexing -> streams progress updates ("Indexing DataCore... 57,833 records", "Indexing SOC... 274,187 files")
7. Index persisted to `~/.datap4k-mcp/index/` (SQLite + Cozo databases)
8. Server is ready for queries

### Config file (`~/.datap4k-mcp/config.toml`)

```toml
[sources]

[[sources.versions]]
path = "E:\\SC Data\\4.7.0-live.11518367"
version = "4.7.0-live"
parser = "auto"  # or "scdatatools" / "unp4k"

[[sources.versions]]
path = "E:\\SC Data\\4.6.0-live.9428532"
version = "4.6.0-live"
parser = "auto"

[index]
path = "~/.datap4k-mcp/index"

[server]
log_level = "info"
```

### Adding versions

User says "I've got 4.8 data at E:\SC Data\4.8.0" -> `index` tool auto-detects version from directory name pattern (`version-channel.build`), indexes, adds to config. Both versions queryable, `diff` works across them.

### Re-indexing

When the same version directory is updated (e.g., re-extracted with newer tool version), user says "re-index 4.7" -> server drops the old index for that version and rebuilds.

## Data Sources Summary

| Source | Files | Format | What it contains |
|--------|-------|--------|-----------------|
| DataCore | 57,833 | JSON | Entity records — items, ships, weapons, missions, factions, reputation, crafting, loot, shops, audio, and 200+ more categories |
| SOC_Unpacked | 274,187 | XML + .soc | Server Object Containers — game world layout, entity placement, spatial positions |
| XML | 12,098 | XML | CryXmlB-converted configs, localization, libs, scripts |
| Config | 9,710 | Various | Engine/game configuration |
| SOC_Raw | 9,442 | Binary | Raw SOC data |
| Loadouts_Character | 2,854 | XML | NPC loadout hierarchies — who wears/carries what |
| CharDefs | 1,625 | .cdf | Character/object definitions |
| Materials | 25,122 | .mtl | Material definitions |
| DCB_Raw | 1 | .dcb | Raw DataForge binary (Game2.dcb) — alternative direct parse source |
| VoiceLines | ~11 | JSON + WAV | Extracted voice data |

**Total:** ~390K+ files per game version, ~174GB on disk.

## Documentation Plan

| Document | Audience | Purpose |
|----------|----------|---------|
| `README.md` | Everyone | What it is, quick start, example LLM conversations |
| `docs/installation.md` | End users | Pre-built binary install, cargo install, MCP client config |
| `docs/supported-formats.md` | End users | Supported extraction tools, how to prepare p4k data |
| `docs/tools-reference.md` | LLM + users | Every tool: description, parameters, example I/O |
| `docs/resources-reference.md` | LLM + users | Every resource URI with schema |
| `docs/prompts-reference.md` | LLM + users | Every prompt template with use cases |
| `docs/architecture.md` | Contributors | Plugin system, storage, query routing, adding parsers |
| `docs/contributing.md` | Contributors | Dev setup, building, testing, PR process |
| `CHANGELOG.md` | Everyone | Version history |

Tools/resources/prompts reference docs are written for both human and LLM readability — clear parameter descriptions, concrete examples, expected output shapes.

## Distribution

- **Pre-built binaries** via GitHub Releases: Windows x86_64 (.exe), macOS aarch64 + x86_64, Linux x86_64. Single static binaries, no runtime dependencies.
- **cargo install:** `cargo install datap4k-mcp`
- **Release cadence:** Tag-driven. Push version tag -> CI builds all platforms -> GitHub Release with checksums.
- **MCP client config:** `{ "mcpServers": { "datap4k": { "command": "datap4k-mcp" } } }`

## Rust Dependencies (Expected)

| Crate | Purpose |
|-------|---------|
| `rmcp` or `mcp-server` | MCP protocol server (stdio transport) |
| `rusqlite` | SQLite with FTS5 |
| `cozo` | Embedded Datalog graph DB |
| `serde` / `serde_json` | JSON serialization |
| `quick-xml` | XML parsing (CryXmlB-converted files, SOC, loadouts) |
| `tokio` | Async runtime |
| `clap` | CLI argument parsing |
| `toml` | Config file parsing |
| `uuid` | UUID handling |
| `walkdir` | Recursive directory traversal |
| `indicatif` | Progress bars during indexing |
| `tracing` | Structured logging |

## Future Considerations (Not in v1)

- **Direct p4k parsing:** Read Data.p4k and Game2.dcb directly instead of requiring pre-extraction. Would eliminate the extraction step entirely.
- **Live re-index on file change:** Watch the data directory and re-index when files change (e.g., user is actively extracting).
- **Remote index sharing:** Publish pre-built indexes so users don't need local p4k extracts at all.
- **Web UI:** Browser-based graph visualization of entity relationships.
