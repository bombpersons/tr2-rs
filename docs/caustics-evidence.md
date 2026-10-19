# Wet-room vertex lighting

New evidence comes only from the supplied USA Rev3 disc through `psx`.
Reproduce with `python3 tools/record_caustics.py`, then
`python3 tools/record_caustics.py --heldout`. The held-out run was first
recorded after implementation. It explicitly sets only the visual phase to 17
at the initial boundary, exercising odd phases without changing geometry,
controllers or shading code. Its phase-0 sample precedes that poke; the first
post-tick sample is 19. All commands, pokes, code hashes and observations are
retained in `game/tests/data/original/caustics_*.json` and phase CSVs.

The ordinary room renderer calls 0x62734 before subdivision for quads at
0x61A0C and triangles at 0x61D38. Each corner's authored vertex index selects
one of 64 initial phase values at 0x8B500. Its wave offset is:

```
(vertex_phase[vertex_index % 64] - current_phase - 1) % 64
```

The signed wave byte from 0x8B4C0 is added to the corner's original shade,
clamped 0..255. The resulting shades enter the existing subdivision averages
and crack fillers. `0x6D1F4` supplies bitwise-not phase for wet rooms;
`0x6D210` clears the effect for dry rooms even when the camera is underwater.
This is separate from underwater palette selection.

The 64-byte wave and 64-byte phase table are identical in Home, Great Wall
and their later checkpoints. Initialization 0x6D178 builds them once; its draw
randomness uses the seed loaded explicitly at 0x21024, 0xD371F947. The recording
tool reconstructs all 64 phase values using the observed generator at 0x6E54C
and checks them against RAM. The resulting tables are baked as ordinary JSON
in `game/data/caustics.json`, with no runtime disc or emulation dependency.

Phase update0x6D234..0x6D244 adds elapsed display fields and wraps 64. Two
ordinary fields per game tick match the port's existing fixed schedule.
The phase continues across level changes. Unusual menu/render stalls and the
original's exact first displayed phase at every entry path are not claimed;
the reflected `Caustics` component allows explicit paused comparison.

`game/tests/caustics.rs` compares 424 distinct inputs/results from 1256 native
corner updates. Together the calibration and fresh odd-phase observations
exercise every wave-table entry and authored indices 6..111, including the
64-vertex wrap. Two 140-field recordings verify 140 simulation ticks
across phase wraps. Another test sends native corners through the actual
room policy, verifying wet-room shades and unchanged dry-room colours.

The engine adds room identity and authored corner indices to its existing
in-memory room tessellation context. Game policy supplies the effect before
its existing subdivision rule. There are no persistent format or template
changes, and ordinary editor/sandbox meshes stay static.

`python3 tools/caustics_smoke.py` uses the standalone full-game MCP renderer.
A dry Home image is byte-identical on/off. In the wet view, 58524 pixels change
with the effect and 60873 change between phases 36 and 52. Health-bar pixels
and triangle counts are unchanged. Returning to an earlier phase, or
turning the effect off, restores the previous image exactly. The wall crop's
mean RGB changes (74.88,148.98,124.25)→(70.06,139.70,116.45), towards native
(67.66,135.73,112.82). These are visual checks, not pixel parity; shading phase,
rasterization, remaining lighting differences and water-surface rendering
still need work. Screenshots were visually inspected.

Workspace all-features tests and Clippy pass with existing unrelated warnings.
The full-game independent submerged reversal replay passes 21 pose checks.
The engine sandbox runs offscreen; editor Home tread preview and untouched
save/reopen pass.
