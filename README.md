# datap4k-mcp

An [MCP](https://modelcontextprotocol.io/) server for querying extracted Star Citizen p4k game data. Point it at a directory of JSON/XML files produced by scdatatools or StarFab and immediately search, traverse, and diff entities across game versions — all from any MCP-capable AI client.

The index is a hybrid of SQLite (full-text search, raw SQL) and Cozo (graph traversal, path-finding, Datalog queries). Data is parsed once and persisted; re-indexing a version replaces only that version's data.

## Quick Start

### 1. Install

**Pre-built binary** — download from [GitHub Releases](https://github.com/nzvengeance/datap4k-mcp/releases) and place it on your `PATH`.

**From source:**
```bash
cargo install datap4k-mcp
```

### 2. Configure your MCP client

Add the server to your client's MCP configuration. For **Claude Code** (`~/.claude/settings.json`):

```json
{
  "mcpServers": {
    "datap4k": {
      "command": "datap4k-mcp",
      "args": ["serve"]
    }
  }
}
```

For **Cursor** (`~/.cursor/mcp.json`) or **VS Code Copilot** (`.vscode/mcp.json`):
```json
{
  "servers": {
    "datap4k": {
      "type": "stdio",
      "command": "datap4k-mcp",
      "args": ["serve"]
    }
  }
}
```

### 3. Index your data

Start a conversation and tell the server where your extracted files live:

> "I just extracted 4.7.0, index it at /mnt/e/SC/Data p4k/4.7.0-live.11518367"

The server auto-detects the version code from the directory name and picks the right parser. You can also run it from the command line without starting the server:

```bash
datap4k-mcp index /mnt/e/SC/Data\ p4k/4.7.0-live.11518367
```

## Features

| Tool | Description |
|------|-------------|
| `search` | Full-text search across entity names and properties (FTS5 syntax) |
| `lookup` | Look up a single entity by UUID or exact class name |
| `traverse` | Walk the entity graph from a starting UUID (configurable depth) |
| `path` | Find the shortest path between two entities by UUID |
| `diff` | Compare an entity's properties between two game versions |
| `query` | Execute a raw SQL query against the SQLite entity index |
| `graph_query` | Execute a CozoScript/Datalog query against the graph index |
| `locate` | Find where an entity can be found in the game world |
| `who_uses` | Find which NPCs, loadouts, or ships reference an item |
| `index` | Index a p4k data directory (triggerable from chat) |
| `status` | Show index status: entity counts, indexed versions |

## Supported Formats

| Tool | Format | Status |
|------|--------|--------|
| [scdatatools](https://gitlab.com/scmodding/frameworks/scdatatools) / [StarFab](https://gitlab.com/scmodding/tools/starfab) | JSON | Supported |
| [unp4k](https://github.com/dolkensp/unp4k) | XML | Coming soon |

The parser is auto-detected from the directory layout. Pass `parser: "scdatatools"` or `parser: "unp4k"` to `index` if you want to override.

## Architecture

```
p4k data directory
        │
        ▼
┌─────────────────┐
│  Parser plugins  │  detect() → parse()
│  scdatatools     │  scdatatools: JSON files under Data/
│  unp4k (soon)   │  unp4k: XML files, flat layout
└────────┬────────┘
         │ ParseResult (nodes + edges)
         ▼
┌─────────────────────────────────────┐
│           Indexer                   │
│  ┌─────────────┐  ┌──────────────┐  │
│  │   SQLite     │  │    Cozo      │  │
│  │  (FTS5, SQL) │  │  (graph,     │  │
│  │  entities    │  │   Datalog)   │  │
│  └─────────────┘  └──────────────┘  │
└────────────────────┬────────────────┘
                     │
                     ▼
          ┌──────────────────┐
          │   QueryEngine    │
          └────────┬─────────┘
                   │
                   ▼
          ┌──────────────────┐
          │   MCP Server     │  11 tools over stdio
          │  DataP4kServer   │
          └──────────────────┘
```

Index data lives at `~/.datap4k-mcp/index/` and config at `~/.datap4k-mcp/config.toml`. Both are created automatically on first run.

## Building from Source

Requires Rust 1.75+.

```bash
git clone https://github.com/nzvengeance/datap4k-mcp
cd datap4k-mcp
cargo build --release
# binary at: target/release/datap4k-mcp
```

Run the test suite:
```bash
cargo test
```

## License

MIT — see [LICENSE](LICENSE).
