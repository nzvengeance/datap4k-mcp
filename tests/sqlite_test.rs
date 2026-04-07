use datap4k_mcp::index::sqlite::SqliteIndex;
use datap4k_mcp::model::*;
use std::collections::HashMap;
use tempfile::TempDir;

fn make_test_node(class_name: &str, entity_type: EntityType) -> Node {
    Node {
        id: uuid::Uuid::new_v4(),
        class_name: class_name.to_string(),
        record_name: format!("Test.{class_name}"),
        entity_type,
        source: "test".to_string(),
        source_path: format!("test/{class_name}.json"),
        game_version: "4.7.0-test".to_string(),
        properties: {
            let mut p = HashMap::new();
            p.insert("name".to_string(), serde_json::json!(class_name));
            p
        },
    }
}

#[test]
fn test_create_and_insert() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let index = SqliteIndex::open(&db_path).unwrap();
    let node = make_test_node("AEGS_Avenger_Titan", EntityType::Ship);
    index.insert_nodes(&[node]).unwrap();
    assert_eq!(index.entity_count().unwrap(), 1);
}

#[test]
fn test_fts_search() {
    let dir = TempDir::new().unwrap();
    let index = SqliteIndex::open(&dir.path().join("test.db")).unwrap();
    index.insert_nodes(&[
        make_test_node("AEGS_Avenger_Titan", EntityType::Ship),
        make_test_node("AEGS_Avenger_Stalker", EntityType::Ship),
        make_test_node("WeaponGun_S1_BEHR_Gatling", EntityType::WeaponShip),
    ]).unwrap();

    let results = index.search("avenger", 10).unwrap();
    assert_eq!(results.len(), 2);

    let results = index.search("gatling", 10).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_lookup_by_uuid() {
    let dir = TempDir::new().unwrap();
    let index = SqliteIndex::open(&dir.path().join("test.db")).unwrap();
    let node = make_test_node("AEGS_Avenger_Titan", EntityType::Ship);
    let id = node.id;
    index.insert_nodes(&[node]).unwrap();
    let found = index.lookup_by_uuid(&id).unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().class_name, "AEGS_Avenger_Titan");
}

#[test]
fn test_lookup_by_class_name() {
    let dir = TempDir::new().unwrap();
    let index = SqliteIndex::open(&dir.path().join("test.db")).unwrap();
    index.insert_nodes(&[make_test_node("AEGS_Avenger_Titan", EntityType::Ship)]).unwrap();
    let results = index.lookup_by_class_name("AEGS_Avenger_Titan").unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_filter_by_type() {
    let dir = TempDir::new().unwrap();
    let index = SqliteIndex::open(&dir.path().join("test.db")).unwrap();
    index.insert_nodes(&[
        make_test_node("AEGS_Avenger_Titan", EntityType::Ship),
        make_test_node("WeaponGun_S1", EntityType::WeaponShip),
    ]).unwrap();
    let ships = index.filter_by_type(EntityType::Ship, 100).unwrap();
    assert_eq!(ships.len(), 1);
    assert_eq!(ships[0].class_name, "AEGS_Avenger_Titan");
}

#[test]
fn test_version_management() {
    let dir = TempDir::new().unwrap();
    let index = SqliteIndex::open(&dir.path().join("test.db")).unwrap();
    index.add_version("4.7.0-live", Some("11518367"), "/data/4.7.0").unwrap();
    let versions = index.list_versions().unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].code, "4.7.0-live");
}
