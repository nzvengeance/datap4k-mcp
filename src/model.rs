use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// All entity types that can be extracted from Star Citizen p4k data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    Ship,
    Vehicle,
    WeaponPersonal,
    WeaponShip,
    Component,
    Ammo,
    Armor,
    Consumable,
    Commodity,
    Mission,
    Location,
    Shop,
    NPC,
    Loadout,
    CraftingBlueprint,
    Faction,
    Reputation,
    LootTable,
    AudioDef,
    Material,
    Tag,
    Unknown,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Ship => "Ship",
            EntityType::Vehicle => "Vehicle",
            EntityType::WeaponPersonal => "WeaponPersonal",
            EntityType::WeaponShip => "WeaponShip",
            EntityType::Component => "Component",
            EntityType::Ammo => "Ammo",
            EntityType::Armor => "Armor",
            EntityType::Consumable => "Consumable",
            EntityType::Commodity => "Commodity",
            EntityType::Mission => "Mission",
            EntityType::Location => "Location",
            EntityType::Shop => "Shop",
            EntityType::NPC => "NPC",
            EntityType::Loadout => "Loadout",
            EntityType::CraftingBlueprint => "CraftingBlueprint",
            EntityType::Faction => "Faction",
            EntityType::Reputation => "Reputation",
            EntityType::LootTable => "LootTable",
            EntityType::AudioDef => "AudioDef",
            EntityType::Material => "Material",
            EntityType::Tag => "Tag",
            EntityType::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for EntityType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = match s {
            "Ship" => EntityType::Ship,
            "Vehicle" => EntityType::Vehicle,
            "WeaponPersonal" => EntityType::WeaponPersonal,
            "WeaponShip" => EntityType::WeaponShip,
            "Component" => EntityType::Component,
            "Ammo" => EntityType::Ammo,
            "Armor" => EntityType::Armor,
            "Consumable" => EntityType::Consumable,
            "Commodity" => EntityType::Commodity,
            "Mission" => EntityType::Mission,
            "Location" => EntityType::Location,
            "Shop" => EntityType::Shop,
            "NPC" => EntityType::NPC,
            "Loadout" => EntityType::Loadout,
            "CraftingBlueprint" => EntityType::CraftingBlueprint,
            "Faction" => EntityType::Faction,
            "Reputation" => EntityType::Reputation,
            "LootTable" => EntityType::LootTable,
            "AudioDef" => EntityType::AudioDef,
            "Material" => EntityType::Material,
            "Tag" => EntityType::Tag,
            _ => EntityType::Unknown,
        };
        Ok(t)
    }
}

/// A single entity node in the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub class_name: String,
    pub record_name: String,
    pub entity_type: EntityType,
    pub source: String,
    pub source_path: String,
    pub game_version: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// A directed relationship between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub label: String,
    pub source_field: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// A non-fatal warning encountered during parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseWarning {
    pub source_path: String,
    pub message: String,
}

/// Accumulated output from one or more parse passes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseResult {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub warnings: Vec<ParseWarning>,
}

impl ParseResult {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consume `other` and append all its contents into `self`.
    pub fn merge(&mut self, other: ParseResult) {
        self.nodes.extend(other.nodes);
        self.edges.extend(other.edges);
        self.warnings.extend(other.warnings);
    }
}

/// Extract version code and optional build number from a p4k data directory name.
///
/// Examples:
/// - `"4.7.0-live.11518367"` → `Some(("4.7.0-live", Some("11518367")))`
/// - `"4.6.0-ptu.9428532"` → `Some(("4.6.0-ptu", Some("9428532")))`
/// - `"random_folder"` → `None`
pub fn version_from_dirname(dirname: &str) -> Option<(String, Option<String>)> {
    // Match patterns like: 4.7.0-live.11518367  or  4.7.0-live  or  4.6.0-ptu.9428532
    let re = Regex::new(r"^(\d+\.\d+\.\d+-[a-z]+)(?:\.(\d+))?$").unwrap();
    let caps = re.captures(dirname)?;
    let version = caps.get(1)?.as_str().to_string();
    let build = caps.get(2).map(|m| m.as_str().to_string());
    Some((version, build))
}
