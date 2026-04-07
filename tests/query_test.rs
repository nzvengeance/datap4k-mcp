use datap4k_mcp::index::Indexer;
use datap4k_mcp::query::QueryEngine;
use tempfile::TempDir;

fn setup_test_indexer() -> (TempDir, Indexer) {
    let dir = TempDir::new().unwrap();
    let config = datap4k_mcp::config::Config {
        index: datap4k_mcp::config::IndexConfig {
            path: dir.path().to_string_lossy().to_string(),
        },
        ..Default::default()
    };
    let indexer = Indexer::open(&config).unwrap();
    indexer
        .index_directory("tests/fixtures/scdatatools", "4.7.0-test", "auto")
        .unwrap();
    (dir, indexer)
}

#[test]
fn test_search_routes_to_sqlite() {
    let (_dir, indexer) = setup_test_indexer();
    let engine = QueryEngine::new(&indexer);
    let result = engine.search("avenger", 10).unwrap();
    assert!(!result.is_empty());
}

#[test]
fn test_lookup_routes_to_sqlite() {
    let (_dir, indexer) = setup_test_indexer();
    let engine = QueryEngine::new(&indexer);
    let results = engine.lookup_by_class_name("AEGS_Avenger_Titan").unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_status_returns_counts() {
    let (_dir, indexer) = setup_test_indexer();
    let engine = QueryEngine::new(&indexer);
    let status = engine.status().unwrap();
    assert!(status.entity_count > 0);
}
