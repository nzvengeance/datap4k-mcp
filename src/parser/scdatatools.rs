//! Parser for scdatatools/StarFab extracted data directories.
//!
//! Handles three data sources:
//! 1. **DataCore JSON records** — `DataCore/libs/foundry/records/**/*.json`
//! 2. **Character loadouts** — `Extracted/Loadouts_Character_Converted/**/*.xml`
//! 3. **SOC Object Containers** — `Extracted/SOC_Unpacked/**/*.xml`

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::model::{Edge, EntityType, Node, ParseResult, ParseWarning};

/// Parser for directories extracted via scdatatools / StarFab.
pub struct ScdatatoolsParser;

impl super::P4kParser for ScdatatoolsParser {
    fn name(&self) -> &str {
        "scdatatools"
    }

    fn detect(&self, path: &Path) -> bool {
        path.join("DataCore/libs/foundry/records").is_dir()
    }

    fn parse(&self, path: &Path, version: &str) -> Result<ParseResult> {
        let mut result = ParseResult::new();

        // 1. DataCore JSON records
        let datacore_dir = path.join("DataCore/libs/foundry/records");
        if datacore_dir.is_dir() {
            let dc_result = parse_datacore_records(&datacore_dir, version);
            result.merge(dc_result);
        }

        // 2. Character loadouts
        let loadouts_dir = path.join("Extracted/Loadouts_Character_Converted");
        if loadouts_dir.is_dir() {
            let lo_result = parse_character_loadouts(&loadouts_dir, version);
            result.merge(lo_result);
        }

        // 3. SOC Object Containers
        let soc_dir = path.join("Extracted/SOC_Unpacked");
        if soc_dir.is_dir() {
            let soc_result = parse_soc_containers(&soc_dir, version);
            result.merge(soc_result);
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// DataCore JSON records
// ---------------------------------------------------------------------------

/// Walk `DataCore/libs/foundry/records/**/*.json` and extract nodes + edges.
fn parse_datacore_records(records_dir: &Path, version: &str) -> ParseResult {
    let mut result = ParseResult::new();

    for entry in WalkDir::new(records_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        match parse_single_datacore_record(path, records_dir, version) {
            Ok(file_result) => result.merge(file_result),
            Err(msg) => {
                result.warnings.push(ParseWarning {
                    source_path: path.display().to_string(),
                    message: msg,
                });
            }
        }
    }

    result
}

/// Parse a single DataCore JSON record file.
fn parse_single_datacore_record(
    path: &Path,
    records_dir: &Path,
    version: &str,
) -> std::result::Result<ParseResult, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read: {e}"))?;

    let root: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("invalid JSON: {e}"))?;

    let record_name = root
        .get("_RecordName_")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing _RecordName_".to_string())?;

    let record_id_str = root
        .get("_RecordId_")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing _RecordId_".to_string())?;

    let id = Uuid::parse_str(record_id_str)
        .map_err(|e| format!("invalid UUID in _RecordId_: {e}"))?;

    // Extract class_name: everything after the last dot in record_name.
    let class_name = record_name
        .rsplit('.')
        .next()
        .unwrap_or(record_name)
        .to_string();

    let record_value = root.get("_RecordValue_");

    // Determine entity type via the 3-tier classification.
    let entity_type = classify_entity(record_value, path, records_dir);

    // Relative path from the records dir for the source_path field.
    let rel_path = path
        .strip_prefix(records_dir)
        .unwrap_or(path)
        .display()
        .to_string();

    let node = Node {
        id,
        class_name,
        record_name: record_name.to_string(),
        entity_type,
        source: "scdatatools".to_string(),
        source_path: rel_path,
        game_version: version.to_string(),
        properties: HashMap::new(),
    };

    let mut result = ParseResult::new();
    result.nodes.push(node);

    // Extract edges from file:// references and _RecordId_ reference objects.
    if let Some(rv) = record_value {
        extract_edges(id, rv, "", &mut result.edges, &mut result.warnings);
    }

    Ok(result)
}

/// 3-tier entity type classification.
///
/// 1. AttachDef.Type field in `_RecordValue_.Components.SAttachableComponentParams.AttachDef.Type`
/// 2. `_RecordValue_._Type_` field
/// 3. File path heuristics
fn classify_entity(
    record_value: Option<&serde_json::Value>,
    path: &Path,
    records_dir: &Path,
) -> EntityType {
    if let Some(rv) = record_value {
        // Tier 1: AttachDef.Type
        if let Some(attach_type) = rv
            .get("Components")
            .and_then(|c| c.get("SAttachableComponentParams"))
            .and_then(|s| s.get("AttachDef"))
            .and_then(|a| a.get("Type"))
            .and_then(|t| t.as_str())
        {
            return match attach_type {
                "Ship" => EntityType::Ship,
                "Vehicle" => EntityType::Vehicle,
                "WeaponGun" => EntityType::WeaponShip,
                "Turret" | "TurretBase" => EntityType::WeaponShip,
                "WeaponPersonal" | "FPSWeapon" => EntityType::WeaponPersonal,
                "MissileRack" | "Missile" => EntityType::WeaponShip,
                "PowerPlant" | "Cooler" | "Shield" | "QuantumDrive" | "Radar"
                | "Computer" | "Battery" | "Scanner" | "Countermeasure" => EntityType::Component,
                "MiningModifier" | "SalvageHead" | "SalvageModifier" => EntityType::Component,
                "Armor" | "Helmet" | "Undersuit" | "Backpack" => EntityType::Armor,
                _ => EntityType::Unknown,
            };
        }

        // Tier 2: _Type_ field
        if let Some(type_field) = rv.get("_Type_").and_then(|t| t.as_str()) {
            return match type_field {
                "AmmoParams" => EntityType::Ammo,
                "MissionBroker" | "MissionDef" => EntityType::Mission,
                "ShopLayout" | "ShopData" => EntityType::Shop,
                "Commodity" | "CommodityDef" => EntityType::Commodity,
                "Consumable" | "ConsumableDef" => EntityType::Consumable,
                "FactionDef" | "Faction" => EntityType::Faction,
                "ReputationDef" | "ReputationReward" => EntityType::Reputation,
                "LootTable" | "LootArchetype" => EntityType::LootTable,
                "CraftingBlueprint" | "CraftingRecipe" => EntityType::CraftingBlueprint,
                "AudioDef" | "SoundDef" => EntityType::AudioDef,
                "MaterialDef" => EntityType::Material,
                "TagDatabase" | "Tag" => EntityType::Tag,
                "NPCTemplate" | "NPCCharacter" => EntityType::NPC,
                _ => {
                    // Fall through to tier 3
                    classify_by_path(path, records_dir)
                }
            };
        }
    }

    // Tier 3: file path
    classify_by_path(path, records_dir)
}

/// Classify entity type by file path patterns.
fn classify_by_path(path: &Path, records_dir: &Path) -> EntityType {
    let rel = path
        .strip_prefix(records_dir)
        .unwrap_or(path)
        .to_string_lossy();
    let rel_lower = rel.to_lowercase();

    if rel_lower.contains("/spaceships/") || rel_lower.contains("/vehicles/") {
        EntityType::Ship
    } else if rel_lower.contains("/weapons/") {
        EntityType::WeaponShip
    } else if rel_lower.contains("/ammoparams/") {
        EntityType::Ammo
    } else if rel_lower.contains("/factions/") {
        EntityType::Faction
    } else if rel_lower.contains("/missions/") {
        EntityType::Mission
    } else if rel_lower.contains("/shops/") {
        EntityType::Shop
    } else if rel_lower.contains("/loadouts/") {
        EntityType::Loadout
    } else if rel_lower.contains("/loot/") {
        EntityType::LootTable
    } else {
        EntityType::Unknown
    }
}

/// Recursively extract edges from JSON values.
///
/// Finds two kinds of references:
/// - `file://` string values → Edge with label derived from the parent field name
/// - Objects with `_RecordId_` → Edge with resolved target UUID
fn extract_edges(
    source_id: Uuid,
    value: &serde_json::Value,
    field_path: &str,
    edges: &mut Vec<Edge>,
    warnings: &mut Vec<ParseWarning>,
) {
    match value {
        serde_json::Value::String(s) => {
            if s.starts_with("file://") {
                let label = label_from_field_path(field_path);
                edges.push(Edge {
                    source_id,
                    target_id: Uuid::nil(), // unresolved file reference
                    label,
                    source_field: field_path.to_string(),
                    properties: {
                        let mut m = HashMap::new();
                        m.insert(
                            "file_ref".to_string(),
                            serde_json::Value::String(s.clone()),
                        );
                        m
                    },
                });
            }
        }
        serde_json::Value::Object(map) => {
            // Check if this is a _RecordId_ reference object.
            if let Some(ref_id_val) = map.get("_RecordId_") {
                if let Some(ref_id_str) = ref_id_val.as_str() {
                    match Uuid::parse_str(ref_id_str) {
                        Ok(target_id) => {
                            let label = label_from_field_path(field_path);
                            let mut props = HashMap::new();
                            if let Some(rn) = map.get("_RecordName_") {
                                props.insert("target_record_name".to_string(), rn.clone());
                            }
                            if let Some(rp) = map.get("_RecordPath_") {
                                props.insert("target_record_path".to_string(), rp.clone());
                            }
                            edges.push(Edge {
                                source_id,
                                target_id,
                                label,
                                source_field: field_path.to_string(),
                                properties: props,
                            });
                        }
                        Err(e) => {
                            warnings.push(ParseWarning {
                                source_path: String::new(),
                                message: format!(
                                    "invalid UUID in _RecordId_ at {field_path}: {e}"
                                ),
                            });
                        }
                    }
                    // Don't recurse into reference objects — we've handled them.
                    return;
                }
            }

            // Recurse into child fields, skipping internal `_*_` keys.
            for (key, child) in map {
                if key.starts_with('_') && key.ends_with('_') {
                    continue;
                }
                let child_path = if field_path.is_empty() {
                    key.clone()
                } else {
                    format!("{field_path}.{key}")
                };
                extract_edges(source_id, child, &child_path, edges, warnings);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let child_path = format!("{field_path}[{i}]");
                extract_edges(source_id, item, &child_path, edges, warnings);
            }
        }
        _ => {}
    }
}

/// Derive an edge label from a dotted field path.
///
/// Takes the last segment and converts it to snake_case-ish form.
/// E.g. `"Components.VehicleComponentParams.manufacturer"` → `"manufacturer"`.
fn label_from_field_path(field_path: &str) -> String {
    let last = field_path
        .rsplit('.')
        .next()
        .unwrap_or(field_path);
    // Strip array index suffix like "[0]"
    let last = if let Some(idx) = last.find('[') {
        &last[..idx]
    } else {
        last
    };
    if last.is_empty() {
        "references".to_string()
    } else {
        last.to_string()
    }
}

// ---------------------------------------------------------------------------
// Character loadouts (XML)
// ---------------------------------------------------------------------------

/// Walk `Extracted/Loadouts_Character_Converted/**/*.xml` and extract loadout nodes + edges.
fn parse_character_loadouts(loadouts_dir: &Path, version: &str) -> ParseResult {
    let mut result = ParseResult::new();

    for entry in WalkDir::new(loadouts_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }

        match parse_single_loadout(path, loadouts_dir, version) {
            Ok(file_result) => result.merge(file_result),
            Err(msg) => {
                result.warnings.push(ParseWarning {
                    source_path: path.display().to_string(),
                    message: msg,
                });
            }
        }
    }

    result
}

/// Parse a single loadout XML file using simple string matching.
///
/// Each file becomes a Loadout-typed Node. Each `itemName` attribute produces
/// an Edge with label "equips".
fn parse_single_loadout(
    path: &Path,
    loadouts_dir: &Path,
    version: &str,
) -> std::result::Result<ParseResult, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read: {e}"))?;

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let rel_path = path
        .strip_prefix(loadouts_dir)
        .unwrap_or(path)
        .display()
        .to_string();

    // Generate a deterministic UUID from the file path.
    let node_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, rel_path.as_bytes());

    let node = Node {
        id: node_id,
        class_name: file_stem.to_string(),
        record_name: file_stem.to_string(),
        entity_type: EntityType::Loadout,
        source: "scdatatools".to_string(),
        source_path: format!("Loadouts_Character_Converted/{rel_path}"),
        game_version: version.to_string(),
        properties: HashMap::new(),
    };

    let mut result = ParseResult::new();
    result.nodes.push(node);

    // Extract Item elements with itemName and portName attributes.
    extract_loadout_items(&contents, node_id, &mut result.edges);

    Ok(result)
}

/// Extract `<Item portName="..." itemName="..." ...>` patterns from loadout XML.
fn extract_loadout_items(xml: &str, source_id: Uuid, edges: &mut Vec<Edge>) {
    // Simple string-based extraction: find each `<Item ` element and pull
    // out the portName and itemName attributes.
    let mut search_from = 0;
    while let Some(item_start) = xml[search_from..].find("<Item ") {
        let abs_start = search_from + item_start;
        // Find the end of this opening tag (either `/>` or `>`).
        let tag_end = match xml[abs_start..].find('>') {
            Some(pos) => abs_start + pos,
            None => break,
        };
        let tag_str = &xml[abs_start..=tag_end];

        let port_name = extract_xml_attr(tag_str, "portName").unwrap_or_default();
        let item_name = extract_xml_attr(tag_str, "itemName").unwrap_or_default();

        if !item_name.is_empty() {
            let mut props = HashMap::new();
            if !port_name.is_empty() {
                props.insert(
                    "port_name".to_string(),
                    serde_json::Value::String(port_name),
                );
            }
            props.insert(
                "item_class_name".to_string(),
                serde_json::Value::String(item_name.clone()),
            );

            // Target is unresolved — we use a deterministic UUID from the item class name
            // so that edges to the same class_name will share a target.
            let target_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, item_name.as_bytes());

            edges.push(Edge {
                source_id,
                target_id,
                label: "equips".to_string(),
                source_field: "itemName".to_string(),
                properties: props,
            });
        }

        search_from = tag_end + 1;
    }
}

// ---------------------------------------------------------------------------
// SOC Object Containers (XML)
// ---------------------------------------------------------------------------

/// Walk `Extracted/SOC_Unpacked/**/*.xml` (skipping `*_editor.xml`) and extract location nodes.
fn parse_soc_containers(soc_dir: &Path, version: &str) -> ParseResult {
    let mut result = ParseResult::new();

    for entry in WalkDir::new(soc_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }

        // Skip editor files.
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if stem.ends_with("_editor") {
                continue;
            }
        }

        match parse_single_soc_container(path, soc_dir, version) {
            Ok(file_result) => result.merge(file_result),
            Err(msg) => {
                result.warnings.push(ParseWarning {
                    source_path: path.display().to_string(),
                    message: msg,
                });
            }
        }
    }

    result
}

/// Parse a single SOC XML file.
///
/// Each file becomes a Location-typed Node. Each `entityClass` attribute
/// produces an Edge with label "contains_entity".
fn parse_single_soc_container(
    path: &Path,
    soc_dir: &Path,
    version: &str,
) -> std::result::Result<ParseResult, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read: {e}"))?;

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let rel_path = path
        .strip_prefix(soc_dir)
        .unwrap_or(path)
        .display()
        .to_string();

    let node_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, rel_path.as_bytes());

    let node = Node {
        id: node_id,
        class_name: file_stem.to_string(),
        record_name: file_stem.to_string(),
        entity_type: EntityType::Location,
        source: "scdatatools".to_string(),
        source_path: format!("SOC_Unpacked/{rel_path}"),
        game_version: version.to_string(),
        properties: HashMap::new(),
    };

    let mut result = ParseResult::new();
    result.nodes.push(node);

    // Extract Entity elements with entityClass attributes.
    extract_soc_entities(&contents, node_id, &mut result.edges);

    Ok(result)
}

/// Extract `<Entity ... entityClass="..." ...>` patterns from SOC XML.
fn extract_soc_entities(xml: &str, source_id: Uuid, edges: &mut Vec<Edge>) {
    let mut search_from = 0;
    while let Some(entity_start) = xml[search_from..].find("<Entity ") {
        let abs_start = search_from + entity_start;
        let tag_end = match xml[abs_start..].find('>') {
            Some(pos) => abs_start + pos,
            None => break,
        };
        let tag_str = &xml[abs_start..=tag_end];

        let entity_class = extract_xml_attr(tag_str, "entityClass").unwrap_or_default();
        let entity_name = extract_xml_attr(tag_str, "name").unwrap_or_default();

        if !entity_class.is_empty() {
            let target_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, entity_class.as_bytes());
            let mut props = HashMap::new();
            if !entity_name.is_empty() {
                props.insert(
                    "entity_name".to_string(),
                    serde_json::Value::String(entity_name),
                );
            }
            props.insert(
                "entity_class".to_string(),
                serde_json::Value::String(entity_class),
            );

            edges.push(Edge {
                source_id,
                target_id,
                label: "contains_entity".to_string(),
                source_field: "entityClass".to_string(),
                properties: props,
            });
        }

        search_from = tag_end + 1;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a named XML attribute value from a tag string.
///
/// Given `<Item portName="Body" itemName="test">` and `"portName"`, returns
/// `Some("Body")`.
fn extract_xml_attr(tag: &str, attr_name: &str) -> Option<String> {
    let needle = format!("{attr_name}=\"");
    let start = tag.find(&needle)?;
    let value_start = start + needle.len();
    let remaining = &tag[value_start..];
    let end = remaining.find('"')?;
    Some(remaining[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xml_attr_basic() {
        let tag = r#"<Item portName="Body" itemName="test_item">"#;
        assert_eq!(extract_xml_attr(tag, "portName"), Some("Body".to_string()));
        assert_eq!(
            extract_xml_attr(tag, "itemName"),
            Some("test_item".to_string())
        );
        assert_eq!(extract_xml_attr(tag, "missing"), None);
    }

    #[test]
    fn test_label_from_field_path() {
        assert_eq!(label_from_field_path("manufacturer"), "manufacturer");
        assert_eq!(
            label_from_field_path("Components.VehicleComponentParams.manufacturer"),
            "manufacturer"
        );
        assert_eq!(label_from_field_path("tags[0]"), "tags");
        assert_eq!(label_from_field_path(""), "references");
    }

    #[test]
    fn test_classify_entity_attach_def() {
        let rv: serde_json::Value = serde_json::json!({
            "Components": {
                "SAttachableComponentParams": {
                    "AttachDef": { "Type": "Ship", "SubType": "Medium" }
                }
            }
        });
        let ty = classify_entity(Some(&rv), Path::new("test.json"), Path::new("."));
        assert_eq!(ty, EntityType::Ship);
    }

    #[test]
    fn test_classify_entity_type_field() {
        let rv: serde_json::Value = serde_json::json!({
            "_Type_": "AmmoParams"
        });
        let ty = classify_entity(Some(&rv), Path::new("test.json"), Path::new("."));
        assert_eq!(ty, EntityType::Ammo);
    }
}
