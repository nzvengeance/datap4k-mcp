# Installation Guide

## Prerequisites

- A Star Citizen p4k extraction tool: [scdatatools](https://gitlab.com/scmodding/frameworks/scdatatools) or [StarFab](https://gitlab.com/scmodding/tools/starfab)
- An extracted p4k data directory (JSON files under a `Data/` subdirectory)

## Installing the Binary

### Option 1: Pre-built Binary (Recommended)

1. Go to [GitHub Releases](https://github.com/nzvengeance/datap4k-mcp/releases)
2. Download the archive for your platform:
   - `datap4k-mcp-x86_64-unknown-linux-gnu.tar.gz` — Linux x86_64
   - `datap4k-mcp-x86_64-pc-windows-msvc.zip` — Windows x86_64
   - `datap4k-mcp-aarch64-apple-darwin.tar.gz` — macOS Apple Silicon
   - `datap4k-mcp-x86_64-apple-darwin.tar.gz` — macOS Intel
3. Extract and place `datap4k-mcp` (or `datap4k-mcp.exe`) somewhere on your `PATH`

Verify the install:
```bash
datap4k-mcp --version
```

### Option 2: cargo install

Requires Rust 1.75+. If you don't have Rust, install it from [rustup.rs](https://rustup.rs/).

```bash
cargo install datap4k-mcp
```

The binary is installed to `~/.cargo/bin/`, which is on `PATH` automatically after a Rust install.

## Configuring Your MCP Client

### Claude Code

Edit `~/.claude/settings.json`:

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

Restart Claude Code after saving. The server starts on demand via stdio.

### Cursor

Edit `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (per-project):

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

### VS Code Copilot

Edit `.vscode/mcp.json` in your workspace (or the user-level settings):

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

## First Run Walkthrough

### Step 1: Extract p4k data

Use scdatatools or StarFab to extract a game version. The resulting directory should look like:

```
4.7.0-live.11518367/
└── Data/
    ├── Libs/
    │   └── Foundry/
    │       └── Records/
    │           ├── entities/
    │           ├── vehicles/
    │           └── ...
    └── ...
```

The directory name (`4.7.0-live.11518367`) is parsed automatically to detect the version code (`4.7.0-live`) and build number (`11518367`).

### Step 2: Index the data

You can index from your AI client or from the command line.

**From a chat conversation:**
> "Index my 4.7.0 data at /mnt/e/SC/Data p4k/4.7.0-live.11518367"

The server will call the `index` tool, parse the directory, and report how many entities and edges were found.

**From the command line:**
```bash
datap4k-mcp index /mnt/e/SC/Data\ p4k/4.7.0-live.11518367
# Output: Indexed 4.7.0-live: 94821 entities, 318443 edges, 12 warnings
```

### Step 3: Query

Once indexed, all 11 tools are available. Try:

- "Search for Gatling guns"
- "Look up entity VEHICLE_AEGS_Gladius"
- "What changed in the Sabre between 4.6.0 and 4.7.0?"
- "Where can I find the Arclight pistol?"

### Configuration File

The config lives at `~/.datap4k-mcp/config.toml` and is updated automatically when you index a new version. You can edit it manually:

```toml
[index]
path = "/home/user/.datap4k-mcp/index"

[server]
log_level = "info"

[[sources.versions]]
path = "/mnt/e/SC/Data p4k/4.7.0-live.11518367"
version = "4.7.0-live"
parser = "auto"

[[sources.versions]]
path = "/mnt/e/SC/Data p4k/4.6.0-live.9428532"
version = "4.6.0-live"
parser = "auto"
```

Valid `log_level` values: `trace`, `debug`, `info`, `warn`, `error`. Logs are written to stderr (never stdout — stdout is reserved for the MCP protocol).
