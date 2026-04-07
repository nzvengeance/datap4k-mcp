# Tools Reference

datap4k-mcp exposes 11 tools over the MCP protocol. All tools communicate over stdio using JSON-RPC. Parameters marked **required** must be provided; all others are optional.

---

## search

Full-text search across entity class names, record names, and properties. Uses SQLite FTS5 under the hood — FTS5 query syntax is supported (prefix queries, boolean operators, phrase queries).

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | yes | — | Search query. FTS5 syntax: `"Gatling"`, `"laser*"`, `"type:WeaponShip gun"` |
| `limit` | integer | no | 20 | Maximum results to return. Capped at 100. |

**Example request**
```json
{
  "name": "search",
  "arguments": {
    "query": "Gatling gun",
    "limit": 5
  }
}
```

**Example response**
```
Found 5 results for 'Gatling gun':

[WeaponShip] WEAP_BEHR_Gatling_S1 — uuid: 3f2a1b4c-... (source: scdatatools)
[WeaponShip] WEAP_BEHR_Gatling_S2 — uuid: 4d3e2f5a-... (source: scdatatools)
[WeaponShip] WEAP_KLWE_Gatling_S3 — uuid: 7c8b9d0e-... (source: scdatatools)
[Component] COMP_BEHR_GatlingBarrel_S1 — uuid: 1a2b3c4d-... (source: scdatatools)
[Ammo] AMMO_BEHR_Gatling_S1 — uuid: 5e6f7a8b-... (source: scdatatools)
```

---

## lookup

Look up a specific entity by its UUID or exact class name. Returns full property details.

Provide either `uuid` or `class_name` — not both.

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `uuid` | string | one of | — | Entity UUID (e.g. `"3f2a1b4c-0000-0000-0000-000000000000"`) |
| `class_name` | string | one of | — | Exact class name (e.g. `"WEAP_BEHR_Gatling_S1"`) |

**Example request**
```json
{
  "name": "lookup",
  "arguments": {
    "class_name": "WEAP_BEHR_Gatling_S1"
  }
}
```

**Example response**
```
Found 1 entities:

## [WeaponShip] WEAP_BEHR_Gatling_S1
- **UUID:** 3f2a1b4c-0000-0000-0000-000000000000
- **Record:** entities/weapons/ship/behr/WEAP_BEHR_Gatling_S1.json
- **Source:** scdatatools (Data/Libs/Foundry/Records/entities/weapons/ship/behr/WEAP_BEHR_Gatling_S1.json)
- **Version:** 4.7.0-live

```json
{
  "size": 1,
  "damage_physical": 142.5,
  "fire_rate": 550,
  "ammo_class": "AMMO_BEHR_Gatling_S1",
  "manufacturer": "BEHR"
}
```
```

---

## traverse

Walk the entity relationship graph starting from a UUID. Returns all entities reachable within the specified depth. Useful for understanding what components, ammo types, or sub-entities are attached to an item.

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `uuid` | string | yes | — | UUID of the starting entity |
| `depth` | integer | no | 2 | Maximum traversal depth. Capped at 5. |

**Example request**
```json
{
  "name": "traverse",
  "arguments": {
    "uuid": "3f2a1b4c-0000-0000-0000-000000000000",
    "depth": 2
  }
}
```

**Example response**
```
Traversal from 3f2a1b4c-0000-0000-0000-000000000000 (depth 2): 4 nodes found

[WeaponShip] WEAP_BEHR_Gatling_S1 — uuid: 3f2a1b4c-... (source: scdatatools)
[Ammo] AMMO_BEHR_Gatling_S1 — uuid: 5e6f7a8b-... (source: scdatatools)
[Component] COMP_BEHR_GatlingBarrel_S1 — uuid: 1a2b3c4d-... (source: scdatatools)
[Material] MAT_Metal_GunBarrel — uuid: 9f0a1b2c-... (source: scdatatools)
```

---

## path

Find the shortest path between two entities in the graph. Returns the chain of nodes connecting them. Useful for discovering indirect relationships (e.g. which faction controls a shop that sells an item).

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `from` | string | yes | — | UUID of the starting entity |
| `to` | string | yes | — | UUID of the target entity |
| `max_depth` | integer | no | 5 | Maximum search depth. Capped at 10. |

**Example request**
```json
{
  "name": "path",
  "arguments": {
    "from": "3f2a1b4c-0000-0000-0000-000000000000",
    "to": "bb00cc11-0000-0000-0000-000000000000",
    "max_depth": 6
  }
}
```

**Example response**
```
Path from 3f2a1b4c-... to bb00cc11-... (3 hops):

[WeaponShip] WEAP_BEHR_Gatling_S1 — uuid: 3f2a1b4c-... (source: scdatatools)
[Ship] VEHICLE_AEGS_Gladius — uuid: 7d8e9f0a-... (source: scdatatools)
[Location] Crusader Industries Showroom — uuid: cc22dd33-... (source: scdatatools)
[Shop] New Deal Ship Shop — uuid: bb00cc11-... (source: scdatatools)
```

If no path exists within `max_depth`, the response explains that no path was found.

---

## diff

Compare an entity's properties between two indexed game versions. Shows properties that were added, removed, or changed. The entity can be specified as a UUID or class name.

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `entity` | string | yes | — | Entity UUID or exact class name |
| `version_a` | string | yes | — | First version code (e.g. `"4.6.0-live"`) |
| `version_b` | string | yes | — | Second version code (e.g. `"4.7.0-live"`) |

**Example request**
```json
{
  "name": "diff",
  "arguments": {
    "entity": "WEAP_BEHR_Gatling_S1",
    "version_a": "4.6.0-live",
    "version_b": "4.7.0-live"
  }
}
```

**Example response**
```
# Diff: WEAP_BEHR_Gatling_S1 (4.6.0-live vs 4.7.0-live)

3 properties differ:

  ~ damage_physical: 130.0 -> 142.5
  ~ fire_rate: 500 -> 550
  + heat_per_shot: 8.2
```

Lines prefixed with `~` are changed values, `+` are new in version_b, `-` are removed.

---

## query

Execute a raw SQL query against the SQLite entity index. The main tables are:

- `entities` — all indexed entities (`id`, `class_name`, `record_name`, `entity_type`, `source`, `source_path`, `game_version`)
- `edges` — relationships between entities (`source_id`, `target_id`, `label`, `source_field`)
- `entity_properties` — key/value properties per entity (`entity_id`, `key`, `value`)

SELECT queries are recommended. The database is shared with the server process.

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `sql` | string | yes | — | SQL query to execute |

**Example request**
```json
{
  "name": "query",
  "arguments": {
    "sql": "SELECT class_name, game_version FROM entities WHERE entity_type = 'WeaponShip' ORDER BY class_name LIMIT 10"
  }
}
```

**Example response**
```
10 rows returned:

WEAP_AEGS_CannonGun_S2 | 4.7.0-live
WEAP_AEGS_CannonGun_S3 | 4.7.0-live
WEAP_AEGS_CannonGun_S4 | 4.7.0-live
WEAP_BEHR_Gatling_S1 | 4.7.0-live
WEAP_BEHR_Gatling_S2 | 4.7.0-live
...
```

---

## graph_query

Execute a raw [CozoScript](https://docs.cozodb.org/en/latest/tutorial.html) / Datalog query against the Cozo graph index. Returns results as JSON.

Cozo nodes and edges are stored as relations:
- `entity[uuid, class_name, entity_type, game_version]`
- `edge[src, dst, label]`

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | yes | — | CozoScript query string |

**Example request**
```json
{
  "name": "graph_query",
  "arguments": {
    "query": "?[class_name] := entity[uuid, class_name, 'Ship', _], edge[uuid, _, 'has_weapon']"
  }
}
```

**Example response**
```json
{
  "headers": ["class_name"],
  "rows": [
    ["VEHICLE_AEGS_Gladius"],
    ["VEHICLE_AEGS_Avenger_Titan"],
    ["VEHICLE_CRUS_Prospector"]
  ]
}
```

---

## locate

Find where an entity is located in the game world. Traverses the graph up to depth 3 from the entity and collects any `Location` or `Shop` type nodes. Accepts a UUID or class name.

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `entity` | string | yes | — | Entity UUID or class name to locate |

**Example request**
```json
{
  "name": "locate",
  "arguments": {
    "entity": "WEAP_BEHR_Arclight_Pistol"
  }
}
```

**Example response**
```
Found 3 locations for 'WEAP_BEHR_Arclight_Pistol':

[Shop] New Deal Ship Shop — uuid: bb00cc11-... (source: scdatatools)
[Location] Port Olisar — uuid: dd44ee55-... (source: scdatatools)
[Shop] Centermass — uuid: ff66aa77-... (source: scdatatools)
```

---

## who_uses

Find which NPCs, loadouts, or ships reference a given item. Traverses the graph from the item and collects any `NPC`, `Loadout`, `Ship`, or `Vehicle` type nodes. Accepts a UUID or class name.

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `item` | string | yes | — | Item UUID or class name |

**Example request**
```json
{
  "name": "who_uses",
  "arguments": {
    "item": "WEAP_BEHR_Arclight_Pistol"
  }
}
```

**Example response**
```
Found 4 entities that use 'WEAP_BEHR_Arclight_Pistol':

[NPC] NPC_Security_Armistice_Light — uuid: 11223344-... (source: scdatatools)
[Loadout] LOAD_CivSec_Light_01 — uuid: 55667788-... (source: scdatatools)
[NPC] NPC_Mercenary_Light_01 — uuid: 99aabbcc-... (source: scdatatools)
[Loadout] LOAD_Merc_Light_02 — uuid: ddeeff00-... (source: scdatatools)
```

---

## index

Index an extracted p4k data directory. Parses all game data files and adds them to the SQLite and Cozo indexes. The version code is auto-detected from the directory name (e.g. `4.7.0-live.11518367` → `4.7.0-live`).

Use `reindex: true` to drop and replace existing data for a version. Without it, a version that was already indexed is a no-op.

**Parameters**

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `path` | string | yes | — | Absolute path to extracted p4k data directory |
| `version` | string | no | auto | Version code (e.g. `"4.7.0-live"`). Auto-detected from directory name if omitted. |
| `reindex` | boolean | no | false | Drop existing data for this version before re-indexing |

**Example request**
```json
{
  "name": "index",
  "arguments": {
    "path": "/mnt/e/SC/Data p4k/4.7.0-live.11518367",
    "reindex": false
  }
}
```

**Example response**
```
Indexed '/mnt/e/SC/Data p4k/4.7.0-live.11518367' as version '4.7.0-live':
- 94821 entities
- 318443 edges
- 12 warnings
- Parsers: scdatatools
```

After indexing, the version is saved to `~/.datap4k-mcp/config.toml` so it is available on the next server start.

---

## status

Show current index status: total entity count, all indexed versions with their data paths, and a breakdown of entity counts by type.

**Parameters**

None.

**Example request**
```json
{
  "name": "status",
  "arguments": {}
}
```

**Example response**
```
# datap4k-mcp Index Status

**Total entities:** 189203

**Versions:**
- 4.7.0-live (build 11518367) -- /mnt/e/SC/Data p4k/4.7.0-live.11518367
- 4.6.0-live (build 9428532) -- /mnt/e/SC/Data p4k/4.6.0-live.9428532

**Entity counts by type:**
- Component: 42187
- WeaponShip: 12043
- WeaponPersonal: 8921
- Ship: 452
- NPC: 7834
- Loadout: 6120
- Location: 1289
- Shop: 318
- Ammo: 4201
- Armor: 3891
- Consumable: 2104
- CraftingBlueprint: 677
- Faction: 31
- Mission: 2559
- LootTable: 884
- Commodity: 203
- Material: 1941
- AudioDef: 28743
- Tag: 11201
- Unknown: 54601
```

---

## Entity Format

All tools that return entity lists use one of two formats:

**One-line summary** (search, traverse, path, locate, who_uses):
```
[EntityType] ClassName — uuid: UUID (source: parser)
```

**Detailed** (lookup, diff):
```markdown
## [EntityType] ClassName
- **UUID:** uuid
- **Record:** relative/path/to/source/file.json
- **Source:** parser (full/path/to/source/file.json)
- **Version:** 4.7.0-live

{properties as JSON}
```

## Entity Types

| Type | Description |
|------|-------------|
| `Ship` | Flyable spacecraft |
| `Vehicle` | Ground and non-ship vehicles |
| `WeaponPersonal` | FPS weapons (rifles, pistols, knives) |
| `WeaponShip` | Ship-mounted weapons |
| `Component` | Ship components (shields, drives, coolers, etc.) |
| `Ammo` | Ammunition definitions |
| `Armor` | FPS armour pieces |
| `Consumable` | Consumable items (stims, medpens) |
| `Commodity` | Trade commodities |
| `Mission` | Mission definitions |
| `Location` | In-world locations (stations, planets, landing zones) |
| `Shop` | In-game shops and kiosks |
| `NPC` | Non-player character definitions |
| `Loadout` | NPC or ship equipment loadouts |
| `CraftingBlueprint` | Crafting recipe blueprints |
| `Faction` | Game factions |
| `Reputation` | Reputation tiers and perks |
| `LootTable` | Loot pool definitions |
| `AudioDef` | Audio asset definitions |
| `Material` | Render material definitions |
| `Tag` | Game data tags |
| `Unknown` | Entities that did not match a known type |
