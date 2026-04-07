use datap4k_mcp::config::Config;
use datap4k_mcp::index::Indexer;
use datap4k_mcp::query::QueryEngine;
use tempfile::TempDir;

#[test]
fn test_end_to_end_parse_index_query() {
    let dir = TempDir::new().unwrap();
    let config = Config {
        index: datap4k_mcp::config::IndexConfig {
            path: dir.path().to_string_lossy().to_string(),
        },
        ..Default::default()
    };

    // Index test fixtures
    let indexer = Indexer::open(&config).unwrap();
    let stats = indexer
        .index_directory("tests/fixtures/scdatatools", "4.7.0-test", "auto")
        .unwrap();

    assert!(stats.node_count > 0, "Should index some entities");
    assert!(stats.edge_count > 0, "Should extract some edges");

    // Query via engine
    let engine = QueryEngine::new(&indexer);

    // Search
    let results = engine.search("avenger", 10).unwrap();
    assert!(!results.is_empty(), "Should find Avenger Titan via search");

    // Lookup by class_name
    let results = engine.lookup_by_class_name("AEGS_Avenger_Titan").unwrap();
    assert!(!results.is_empty(), "Should find Avenger Titan by class_name");
    let titan = &results[0];
    assert_eq!(titan.entity_type, datap4k_mcp::model::EntityType::Ship);

    // Status
    let status = engine.status().unwrap();
    assert!(status.entity_count > 0);
    assert!(!status.versions.is_empty());
}
