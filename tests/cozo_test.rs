use datap4k_mcp::index::cozo::CozoGraph;

fn test_uuid(n: u8) -> uuid::Uuid {
    uuid::Uuid::from_bytes([n, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, n])
}

#[test]
fn test_create_graph() {
    let graph = CozoGraph::open_in_memory().unwrap();
    assert_eq!(graph.entity_count().unwrap(), 0);
}

#[test]
fn test_insert_and_traverse() {
    let graph = CozoGraph::open_in_memory().unwrap();

    let ship_id = test_uuid(1);
    let weapon_id = test_uuid(2);
    let ammo_id = test_uuid(3);

    graph.insert_entities(&[
        (ship_id, "AEGS_Avenger_Titan", "Ship", "4.7.0"),
        (weapon_id, "WeaponGun_S1_BEHR", "WeaponShip", "4.7.0"),
        (ammo_id, "Bullet_Gatling_S1", "Ammo", "4.7.0"),
    ]).unwrap();

    graph.insert_edges(&[
        (ship_id, weapon_id, "has_weapon", "weapon_slot_1"),
        (weapon_id, ammo_id, "has_ammo", "ammoParamsRecord"),
    ]).unwrap();

    // Traverse 1 hop from ship
    let neighbors = graph.traverse(&ship_id, 1).unwrap();
    assert_eq!(neighbors.len(), 1, "Ship should have 1 direct neighbor (weapon)");

    // Traverse 2 hops from ship — should reach ammo
    let neighbors = graph.traverse(&ship_id, 2).unwrap();
    assert_eq!(neighbors.len(), 2, "Ship should reach weapon + ammo in 2 hops");
}

#[test]
fn test_find_path() {
    let graph = CozoGraph::open_in_memory().unwrap();

    let a = test_uuid(1);
    let b = test_uuid(2);
    let c = test_uuid(3);

    graph.insert_entities(&[
        (a, "EntityA", "Ship", "4.7.0"),
        (b, "EntityB", "Component", "4.7.0"),
        (c, "EntityC", "Ammo", "4.7.0"),
    ]).unwrap();

    graph.insert_edges(&[
        (a, b, "has_component", "field1"),
        (b, c, "uses_ammo", "field2"),
    ]).unwrap();

    let path = graph.find_path(&a, &c, 5).unwrap();
    assert!(path.is_some(), "Should find path A -> B -> C");
    let path = path.unwrap();
    assert_eq!(path.len(), 3, "Path should have 3 nodes: A, B, C");
}

#[test]
fn test_no_path() {
    let graph = CozoGraph::open_in_memory().unwrap();

    let a = test_uuid(1);
    let b = test_uuid(2);

    graph.insert_entities(&[
        (a, "EntityA", "Ship", "4.7.0"),
        (b, "EntityB", "Component", "4.7.0"),
    ]).unwrap();

    let path = graph.find_path(&a, &b, 5).unwrap();
    assert!(path.is_none(), "Should find no path between disconnected nodes");
}
