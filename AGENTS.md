You have access to a tool called 'psx'. It is a specialist tool for reverse engineering playstation 1 games.

Your goal is to write a Bevy game matching as close as possible to the original playstation game in your working directory.
Extract 3d models, textures, level data, and other assets from the ROM and bake them into the project as standalone files.

Write the game using idiomatic Bevy / ECS patterns — not playstation emulation.
Use modern game architecture: components, systems, assets, scenes, and idiomatic Bevy patterns

The game should look and play like the playstation game, but the bin/cue files are an asset source only — the Rust code must not depend on it at build or runtime.

The game should be structured so it can be modified and extended after reaching feature parity.

psx provides a template Rust project with a pre-made MCP server for driving the game, taking screenshots, and comparing against the ROM. You should use this as a base.

## Reference restriction

Use the supplied disc and the `psx` tool as the sole evidence source for new
reverse-engineering work. Do not consult OpenLara, TR2Main, or other game
reimplementations. Existing code/assets predate this restriction and may contain
such references; independently verify any behavior being changed against PSX
runtime observations or binary evidence. Preserve the provenance distinction.

## Workspace

A native Bevy port of a PlayStation game, built as an engine and a game from
the start. Read the guide for the crate you are working in:

- `engine/AGENTS.md`: what belongs in the engine, the placement rules, the
  sandbox, formats people can author.
- `game/AGENTS.md`: turning reverse-engineering evidence into idiomatic Bevy
  code, tick rate, trajectory tests, verification.
- `psx-bevy/`: the template's crate. Its README documents the PS1 look,
  input, testing and the developer MCP. You may edit it when that is the
  right place for a change, but keep template updates easy to merge: see
  "Editing psx-bevy" in `docs/template.md`.

The psx toolkit's `guide` tool (topic `port`) has the porting workflow.
Routine check: `cargo test --workspace --all-features`.

Keep `editor/` current as formats and port features grow (see `editor/AGENTS.md`
and `docs/editor.md`). It depends only on the engine, uses a full-resolution
UI and ordinary 3D rendering, and preserves untouched level files byte for byte.
