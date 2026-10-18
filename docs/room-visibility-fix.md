# Missing room geometry (2026-10-18)

Reported at the Great Wall entrance, immediately ahead of Lara, and on Home's
training course. Collision remained solid. Two engine presentation bugs were
found; no disc assets, game rules, level formats or template files changed.

## Mesh restoration

Camera-dependent tessellation removes `Mesh3d` when a room has no drawable
faces. Bevy 0.20 also removes that entity's extracted material binding.
Reinserting `Mesh3d` alone does not necessarily extract the material again:
material extraction watches changes to `ViewVisibility` or `MeshMaterial3d`,
and another camera can keep the visibility value unchanged. The room then has
a nonempty mesh and positive visibility stats but draws nothing.

`engine/src/render/rooms.rs` now marks the existing material changed when
restoring a missing mesh. This applies to both room surfaces and filler
underlays. Ordinary rebuilds retain their existing material change ticks and
reuse the same asset handles. Empty geometry is still not submitted.

Observed in the running port: Wall Room36 reported 436 triangles and inherited
visibility while its entrance was black. After the fix it draws the cave walls
and floor. In Home Room44 the first view could draw correctly, then became
black after turning away and returning. Both views now survive repeated turns
with identical screenshots.

## Portal traversal

The provisional nearby-room selector combined depth-first traversal, an
eight-link depth budget and one visited bit per room. A long cyclic route
could consume the budget before a shorter route was visited, suppressing the
rooms beyond it. Breadth-first traversal now visits shortest paths first and
makes the result independent of portal-list ordering. The existing distance
and depth limits remain provisional; this is not PSX portal-clipping parity.

## Regression and verification

- `engine/tests/room_visibility.rs`: a cyclic graph with a direct short path,
  reversed portal ordering, depth/distance limits and an invalid root.
- `render::rooms::tests::restoring_an_empty_mesh_refreshes_its_material_binding`:
  material invalidation on restoration, without invalidating ordinary rebuilds.
- `python3 tools/room_visibility_smoke.py`: paused Wall and Home views, two
  turn-away/return cycles each, room triangle counts and screenshot pixel
  coverage. Both fixed views are byte-identical on every return. In the
  80x55 centre window, Wall retains 3483 nonblack pixels and Home 4013.
  `--binary` and `--levels` also allow testing an older binary: the retained
  pre-fix binary fails with zero lit pixels on Wall's first view and Home's
  first return, despite nonzero room triangle counts.
- `cargo test --workspace --all-features`: 192 tests pass, including all
  existing exact gameplay trajectories and editor save/reopen tests.
- Clippy completes with existing unrelated warnings. Workspace formatting
  still reports pre-existing differences; new rendering/test code is formatted.
- The engine sandbox builds and runs over offscreen MCP; ten forward ticks
  move its walker and its static scene renders. The editor sandbox and Home
  smoke checks preserve byte-exact untouched saves and normal full-resolution
  rendering. No editor format update is required.

Artifacts and transcripts: `validation/room-visibility/`,
`validation/room-visibility-unfixed/`,
`validation/room-visibility-home-unfixed/` and
`validation/room-visibility-sandbox/`. Original PSX renderer evidence and the
decoded assets retain their existing provenance; these are Bevy lifecycle and
graph-traversal regressions, not new reverse-engineering claims.
