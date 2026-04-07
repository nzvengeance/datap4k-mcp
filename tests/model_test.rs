use datap4k_mcp::model::*;
use std::collections::HashMap;

#[test]
fn test_entity_type_display() {
    assert_eq!(EntityType::Ship.as_str(), "Ship");
    assert_eq!(EntityType::WeaponPersonal.as_str(), "WeaponPersonal");
    assert_eq!(EntityType::Unknown.as_str(), "Unknown");
}

#[test]
fn test_entity_type_from_str() {
    assert_eq!("Ship".parse::<EntityType>().unwrap(), EntityType::Ship);
    assert_eq!("Unknown".parse::<EntityType>().unwrap(), EntityType::Unknown);
    assert_eq!("FooBar".parse::<EntityType>().unwrap(), EntityType::Unknown);
}

#[test]
fn test_node_creation() {
    let node = Node {
        id: uuid::Uuid::new_v4(),
        class_name: "AEGS_Avenger_Titan".to_string(),
        record_name: "EntityClassDefinition.AEGS_Avenger_Titan".to_string(),
        entity_type: EntityType::Ship,
        source: "scdatatools".to_string(),
        source_path: "DataCore/libs/foundry/records/entities/spaceships/aegs_avenger_titan.json".to_string(),
        game_version: "4.7.0-live".to_string(),
        properties: HashMap::new(),
    };
    assert_eq!(node.class_name, "AEGS_Avenger_Titan");
    assert_eq!(node.entity_type, EntityType::Ship);
}

#[test]
fn test_edge_creation() {
    let source = uuid::Uuid::new_v4();
    let target = uuid::Uuid::new_v4();
    let edge = Edge {
        source_id: source,
        target_id: target,
        label: "has_quantum_drive".to_string(),
        source_field: "quantumDrive".to_string(),
        properties: HashMap::new(),
    };
    assert_eq!(edge.label, "has_quantum_drive");
    assert_eq!(edge.source_id, source);
}

#[test]
fn test_parse_result_aggregation() {
    let mut result = ParseResult::new();
    assert_eq!(result.nodes.len(), 0);
    assert_eq!(result.edges.len(), 0);
    assert_eq!(result.warnings.len(), 0);

    result.nodes.push(Node {
        id: uuid::Uuid::new_v4(),
        class_name: "test".to_string(),
        record_name: "test".to_string(),
        entity_type: EntityType::Unknown,
        source: "test".to_string(),
        source_path: "test".to_string(),
        game_version: "1.0".to_string(),
        properties: HashMap::new(),
    });
    result.warnings.push(ParseWarning {
        source_path: "foo.json".to_string(),
        message: "unresolved reference".to_string(),
    });

    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn test_version_from_dirname() {
    assert_eq!(
        version_from_dirname("4.7.0-live.11518367"),
        Some(("4.7.0-live".to_string(), Some("11518367".to_string())))
    );
    assert_eq!(
        version_from_dirname("4.6.0-ptu.9428532"),
        Some(("4.6.0-ptu".to_string(), Some("9428532".to_string())))
    );
    assert_eq!(version_from_dirname("random_folder"), None);
}
