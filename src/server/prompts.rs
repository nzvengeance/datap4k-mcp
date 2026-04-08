// MCP prompt helpers for DataP4kServer.

use rmcp::model::{
    GetPromptResult, Prompt, PromptArgument, PromptMessage, PromptMessageContent,
    PromptMessageRole,
};

// ---------------------------------------------------------------------------
// List helpers
// ---------------------------------------------------------------------------

/// Build the full list of prompts exposed by the server.
pub fn list() -> Vec<Prompt> {
    vec![
        Prompt::new(
            "investigate-item",
            Some("Structured step-by-step investigation of a game item: search, lookup, components, locations, and NPC usage"),
            Some(vec![
                PromptArgument::new("item_name")
                    .with_description("The name or class name of the item to investigate (e.g. 'Avenger Titan', 'AEGS_Avenger')")
                    .with_required(true),
            ]),
        ),
        Prompt::new(
            "compare-versions",
            Some("Compare what changed between two game patches for a given entity category"),
            Some(vec![
                PromptArgument::new("version_a")
                    .with_description("First game version code (e.g. '4.6.0-live')")
                    .with_required(true),
                PromptArgument::new("version_b")
                    .with_description("Second game version code (e.g. '4.7.0-ptu')")
                    .with_required(true),
                PromptArgument::new("category")
                    .with_description("Optional entity type to focus on (e.g. 'Ship', 'WeaponPersonal')")
                    .with_required(false),
            ]),
        ),
        Prompt::new(
            "explore-location",
            Some("Explore what can be found at a game location: shops, NPCs, loot, and connected entities"),
            Some(vec![
                PromptArgument::new("location")
                    .with_description("Location name or class name (e.g. 'Area18', 'Levski')")
                    .with_required(true),
            ]),
        ),
        Prompt::new(
            "trace-reward-chain",
            Some("Follow the mission → reward → loot chain for a given mission, tracing all downstream items"),
            Some(vec![
                PromptArgument::new("mission_name")
                    .with_description("Mission name or class name to trace (e.g. 'Delivery_Small')")
                    .with_required(true),
            ]),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Get helpers — each returns the guided message sequence for that prompt
// ---------------------------------------------------------------------------

pub fn investigate_item(item_name: &str) -> GetPromptResult {
    GetPromptResult::new(vec![
        PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(format!(
                "Investigate the item '{}' in the p4k data.",
                item_name
            )),
        ),
        PromptMessage::new(
            PromptMessageRole::Assistant,
            PromptMessageContent::text(format!(
                "I'll investigate '{}' step by step:\n\n\
                 1. Search for matching entities using `search`.\n\
                 2. Look up the full record by UUID or class name using `lookup`.\n\
                 3. Traverse its graph relationships (components, sub-entities) using `traverse`.\n\
                 4. Find where it appears in the game world using `locate`.\n\
                 5. Check which NPCs or loadouts reference it using `who_uses`.\n\n\
                 Let me start.",
                item_name
            )),
        ),
        PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(
                "Please carry out each step and summarise your findings.",
            ),
        ),
    ])
}

pub fn compare_versions(version_a: &str, version_b: &str, category: Option<&str>) -> GetPromptResult {
    let scope = category
        .map(|c| format!(" focusing on the '{}' category", c))
        .unwrap_or_default();
    GetPromptResult::new(vec![
        PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(format!(
                "Compare what changed between game versions '{}' and '{}'{} in the p4k data.",
                version_a, version_b, scope
            )),
        ),
        PromptMessage::new(
            PromptMessageRole::Assistant,
            PromptMessageContent::text(format!(
                "I'll compare '{}' and '{}'{} using the following approach:\n\n\
                 1. Check the `p4k://versions` resource to confirm both versions are indexed.\n\
                 2. Use `query` to list entities present in one version but not the other (additions/removals).\n\
                 3. Use `diff` on representative entities to surface property-level changes.\n\
                 4. Summarise: new entities, removed entities, changed properties.\n\n\
                 Starting now.",
                version_a, version_b, scope
            )),
        ),
        PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(
                "Please carry out the comparison and report the key changes.",
            ),
        ),
    ])
}

pub fn explore_location(location: &str) -> GetPromptResult {
    GetPromptResult::new(vec![
        PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(format!(
                "Explore what can be found at the location '{}' in the p4k data.",
                location
            )),
        ),
        PromptMessage::new(
            PromptMessageRole::Assistant,
            PromptMessageContent::text(format!(
                "I'll explore '{}' in these steps:\n\n\
                 1. Search for the location entity using `search`.\n\
                 2. Look up its full record and properties using `lookup`.\n\
                 3. Traverse its graph to find connected shops, NPCs, and items using `traverse`.\n\
                 4. Summarise what's available: shops and their inventory, NPCs, loot opportunities.\n\n\
                 Starting now.",
                location
            )),
        ),
        PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(
                "Please carry out the exploration and summarise what's at this location.",
            ),
        ),
    ])
}

pub fn trace_reward_chain(mission_name: &str) -> GetPromptResult {
    GetPromptResult::new(vec![
        PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(format!(
                "Trace the full reward chain for the mission '{}' in the p4k data.",
                mission_name
            )),
        ),
        PromptMessage::new(
            PromptMessageRole::Assistant,
            PromptMessageContent::text(format!(
                "I'll trace the reward chain for '{}' step by step:\n\n\
                 1. Search for the mission entity using `search`.\n\
                 2. Look up its full record to find reward pool references using `lookup`.\n\
                 3. Traverse connected reward-pool, loot-table, and item entities using `traverse` and `path`.\n\
                 4. For each reward item, look up its properties and locations using `lookup` and `locate`.\n\
                 5. Summarise the full chain: mission → reward pools → items → where those items also appear.\n\n\
                 Starting now.",
                mission_name
            )),
        ),
        PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::text(
                "Please trace the full chain and list all items reachable through this mission's rewards.",
            ),
        ),
    ])
}
