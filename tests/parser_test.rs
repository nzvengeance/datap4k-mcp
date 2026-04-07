use datap4k_mcp::parser::{P4kParser, ScdatatoolsParser};
use std::path::Path;

#[test]
fn test_scdatatools_detect_valid() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures/scdatatools");
    assert!(parser.detect(path));
}

#[test]
fn test_scdatatools_detect_invalid() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures"); // no DataCore dir here
    assert!(!parser.detect(path));
}

#[test]
fn test_scdatatools_parse_datacore_records() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures/scdatatools");
    let result = parser.parse(path, "4.7.0-test").unwrap();

    // Should find at least 3 DataCore JSON records
    assert!(
        result.nodes.len() >= 3,
        "Expected at least 3 nodes, got {}",
        result.nodes.len()
    );

    // Avenger Titan should be typed as Ship
    let titan = result
        .nodes
        .iter()
        .find(|n| n.class_name.contains("Avenger_Titan"));
    assert!(titan.is_some(), "Should find Avenger Titan");
    assert_eq!(
        titan.unwrap().entity_type,
        datap4k_mcp::model::EntityType::Ship
    );

    // Weapon should be typed as WeaponShip
    let weapon = result
        .nodes
        .iter()
        .find(|n| n.class_name.contains("BEHR_Gatling"));
    assert!(weapon.is_some(), "Should find BEHR Gatling");
    assert_eq!(
        weapon.unwrap().entity_type,
        datap4k_mcp::model::EntityType::WeaponShip
    );
}

#[test]
fn test_scdatatools_parse_extracts_edges() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures/scdatatools");
    let result = parser.parse(path, "4.7.0-test").unwrap();

    assert!(
        !result.edges.is_empty(),
        "Should extract edges from file:// references"
    );
}

#[test]
fn test_scdatatools_parse_warnings_not_fatal() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures/scdatatools");
    let result = parser.parse(path, "4.7.0-test");
    assert!(result.is_ok(), "Parsing should not fail on warnings");
}

#[test]
fn test_scdatatools_parse_loadout_nodes() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures/scdatatools");
    let result = parser.parse(path, "4.7.0-test").unwrap();

    let loadout = result
        .nodes
        .iter()
        .find(|n| n.entity_type == datap4k_mcp::model::EntityType::Loadout);
    assert!(loadout.is_some(), "Should find a Loadout node from XML");

    // Loadout should have equips edges
    let equips_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.label == "equips")
        .collect();
    assert!(
        !equips_edges.is_empty(),
        "Should extract equips edges from loadout XML"
    );
}

#[test]
fn test_scdatatools_parse_location_nodes() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures/scdatatools");
    let result = parser.parse(path, "4.7.0-test").unwrap();

    let location = result
        .nodes
        .iter()
        .find(|n| n.entity_type == datap4k_mcp::model::EntityType::Location);
    assert!(
        location.is_some(),
        "Should find a Location node from SOC XML"
    );

    // Location should have contains_entity edges
    let contains_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.label == "contains_entity")
        .collect();
    assert!(
        !contains_edges.is_empty(),
        "Should extract contains_entity edges from SOC XML"
    );
}

#[test]
fn test_scdatatools_ammo_typed_correctly() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures/scdatatools");
    let result = parser.parse(path, "4.7.0-test").unwrap();

    let ammo = result
        .nodes
        .iter()
        .find(|n| n.class_name.contains("Bullet_Ballistic_Gatling_S1"));
    assert!(ammo.is_some(), "Should find ammo record");
    assert_eq!(
        ammo.unwrap().entity_type,
        datap4k_mcp::model::EntityType::Ammo
    );
}

#[test]
fn test_scdatatools_record_reference_edges() {
    let parser = ScdatatoolsParser;
    let path = Path::new("tests/fixtures/scdatatools");
    let result = parser.parse(path, "4.7.0-test").unwrap();

    // The Avenger Titan has a _RecordId_ reference in its tags array
    let tag_edge = result.edges.iter().find(|e| {
        e.target_id.to_string() == "5f12dd31-4ff9-41a5-b5e2-916270351ba9"
    });
    assert!(
        tag_edge.is_some(),
        "Should extract edge from _RecordId_ reference object"
    );
}
