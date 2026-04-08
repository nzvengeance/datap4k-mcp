// MCP resource helpers for DataP4kServer.

use rmcp::model::{RawResource, Resource, ResourceContents, AnnotateAble};

/// Build the list of static resources exposed by the server.
pub fn list() -> Vec<Resource> {
    vec![
        RawResource::new("p4k://versions", "versions")
            .with_description("List of indexed game versions with entity counts")
            .with_mime_type("application/json")
            .no_annotation(),
        RawResource::new("p4k://categories", "categories")
            .with_description(
                "Map of entity type → count for the first indexed version",
            )
            .with_mime_type("application/json")
            .no_annotation(),
        RawResource::new("p4k://stats", "stats")
            .with_description(
                "Summary stats: total entities, edges, and indexed versions",
            )
            .with_mime_type("application/json")
            .no_annotation(),
        RawResource::new("p4k://schema", "schema")
            .with_description("All EntityType variants recognised by the indexer")
            .with_mime_type("application/json")
            .no_annotation(),
    ]
}

/// Build text resource contents for a URI, given a pre-serialised JSON body.
pub fn text_contents(uri: &str, json: String) -> ResourceContents {
    ResourceContents::text(json, uri)
        .with_mime_type("application/json")
}
