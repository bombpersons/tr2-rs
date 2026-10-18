# Port verification and completion

Agreed 2026-10-18. Baseline committed and pushed as `5bedba0` before starting.
Use only the supplied disc and `psx` for new reverse-engineering evidence.
Existing inherited behavior is provisional until independently checked.

Check an item only after implementing it, retaining PSX evidence, adding
meaningful regression coverage and completing the applicable workspace,
sandbox, editor and visual checks. Link the evidence and tests when checking
it off. Record partial progress below without checking the parent item.

## Traversal

- [ ] Jump, grab and hang: original grab conditions, alignment, animation and
  collision behavior; climb from a hang and release to drop. Include a complete
  recorded route and an independently recorded validation route.
- [ ] Three-click climbs: entry conditions, authored timing and final offset.
- [ ] Backward movement against walls, steps, drops and slopes.
- [ ] Landing recovery: original animation dispatch and input recovery windows.

## Water

- [ ] Swimming: acceleration, braking, turning, pitch, collision and animation.
- [ ] Water transitions: entry, surface, submerge and climb out; oxygen behavior.

## Combat and interactions

- [ ] Pistols: draw/holster, firing cadence, targeting and shot/damage timing.
- [ ] Tiger and spider behavior: attack selection, contact and damage windows.
- [ ] Pickups: alignment, animation and collection timing.
- [ ] Switches and key/puzzle use: alignment, animation and trigger timing.
- [ ] Doors: collision and floor/portal changes during opening/closing,
  including occupancy override.

## Appearance

- [ ] Distance darkening: original room/object depth shading and boundaries.
- [ ] Distant texture LOD: original >=8192 room policy and texture selection.
- [ ] Home sky: extract and render with the original placement and behavior.

## Progress

- 2026-10-18: Baseline `cargo test --workspace --all-features` passed before
  checkpoint commit/push. GPU/MCP tests are opt-in and were not run in that check.
  Ledge traversal is the first investigation.
