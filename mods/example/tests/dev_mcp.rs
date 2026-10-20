//! The mod through its real binary: offscreen GPU and the developer MCP,
//! with a software adapter. Checks that the mod's level renders and its
//! wanderer walks.
#![cfg(feature = "dev-mcp")]

use engine::testing::mcp::Client;
use serde_json::json;
use std::process::Command;

#[test]
#[ignore = "requires a working offscreen GPU/software adapter; no display needed"]
fn the_mod_renders_and_its_wanderer_walks() {
    let output = std::env::temp_dir().join(format!("tr2-example-mod-{}", std::process::id()));
    std::fs::create_dir_all(&output).unwrap();
    println!("GPU evidence: {}", output.display());
    let mut command = Command::new(env!("CARGO_BIN_EXE_example-mod"));
    command.env("BEVY_ASSET_ROOT", env!("CARGO_MANIFEST_DIR"));
    let mut client = Client::launch(command, &output.join("mod.log"));
    client.call("game_step", json!({"frames": 10}));
    let before = client.screenshot(output.join("before.png"));
    let colors: std::collections::HashSet<_> = before.pixels().map(|p| p.0).collect();
    assert!(colors.len() > 4, "the mod's level should render: {} colours", colors.len());
    let z = |client: &mut Client| {
        client.call("game_query", json!({"name": "Wanderer", "components": ["Transform"]}))["structuredContent"]
            ["entities"][0]["components"]["Transform"]["translation"][2]
            .as_f64()
            .unwrap()
    };
    let start = z(&mut client);
    client.call("game_step", json!({"frames": 30, "buttons": ["up"]}));
    assert!(z(&mut client) > start + 0.5, "the wanderer should walk forward");
    client.screenshot(output.join("after-walk.png"));
}
