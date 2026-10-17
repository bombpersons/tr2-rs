# Working in the editor

Read ../engine/AGENTS.md and ../docs/editor.md. This crate depends on engine,
never game. Open, inspect and save engine formats; get type names through
registry metadata. Game content is data, supplied by a file path.

Use a normal full-resolution interface and ordinary Bevy 3D rendering. The
editor does not need the game's low-resolution PS1 look. Engine meshes and
posing can be reused; keep rendering adaptation here, without changing how
the game draws.

Every settled engine format must remain accessible as the port grows. Extend
validation and save/reopen coverage when adding fields or formats. An
untouched level save must reproduce the file byte for byte, including unknown
fields. Keep the hand-authored sandbox usable without disc data.

Run cargo test --workspace --all-features. For UI/preview or saving changes,
run tools/editor_smoke.py after cargo build -p editor --all-features, and look
at its screenshots. Never run write tests against source game assets: copy
them under validation/ or a temporary directory first.
