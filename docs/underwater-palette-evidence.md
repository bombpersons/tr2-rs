# Underwater palette

Evidence is the supplied USA Rev3 disc through `psx`, independently of any
inherited reimplementation references. Reproduce with
`python3 tools/record_underwater_palette.py`; compact observations and exact
commands are in `game/tests/data/original/underwater_palette*.json`.

The native renderer uses a second set of CLUTs, rather than colouring the
screen or multiplying vertex red. Both Home checkpoints contain 8192 normal
palette entries at VRAM x960..1023/y384..511 and their underwater counterparts
128 rows above. Every pair in both captures satisfies:

```
water RGB555 = (normal & ~31) | ((normal & 31) >> 1)
```

Green, blue and STP remain byte-exact. Observed entries cover all 32 red
levels. The fixture retains a representative pair for each level, capture and
palette-pair hashes, checkpoint identity and source-code range hashes. The
Rust test checks the atlas conversion against these observations.

The room/object setup at 0x24FD0..0x25000 and 0x251BC..0x251EC tests room flag 1:
wet rooms call 0x6D1F4, forcing the underwater palette; dry rooms call 0x6D210
with the camera-water value from 0x8C250. Camera setup 0x23F14..0x23F20 sets
that value from its room flag. Lara's renderer makes the same choice at
0x24060..0x240AC. The room renderer 0x617B0..0x61804 selects a packed CLUT
adjustment of 0x20000000, applied at 0x61954, corresponding to 128 palette rows.
Traces retain actual wet/dry calls. Caustics use another global and are a
separate effect, subsequently ported in [wet-room vertex lighting](caustics-evidence.md).

The game derives one ordinary Bevy `Image` from the baked atlas: recover its
five-bit red, halve it, then expand it using the same floor(r*255/31) as the
existing importer. Alpha is preserved. The engine's reusable `TextureOverride`
component changes individual mesh textures, caches shared material variants,
and preserves live source lighting. Removing the override restores the source
material. Game policy follows room parents through the scene hierarchy, so
rooms, underlays, objects and bones receive the palette while HUD and sky
retain their textures. No level format, template or ROM dependency was added.

`python3 tools/underwater_palette_smoke.py` checks a freshly recorded swim view
captured after implementation. It verifies 86616 changed pixels, unchanged
green/blue everywhere, unchanged health-bar pixels and exact restoration of
the previous image. A dry Home comparison is byte-identical with the effect
on/off. In the fixed wall crop, mean red changes 156.57→74.88; native is 67.66.
Remaining shade/geometry differences include caustics, ported in the subsequent change; this is not a
claim of pixel parity. The images were inspected alongside the native view.
The existing 21-pose full-game swimming steering replay remains exact.

Workspace all-features tests and Clippy pass with existing unrelated warnings.
The engine sandbox runs offscreen; editor Home Lara stroke preview and an
untouched save pass. The engine test also verifies shared overrides, isolation
from ordinary meshes, live lighting and restoration after removal.
