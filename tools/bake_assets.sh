#!/usr/bin/env bash
# Rebuild the game's content from your own copy of Tomb Raider II
# (PlayStation, USA Rev 3, bin/cue with one .bin per track).
#
#   tools/bake_assets.sh [path/to/game.cue]     default: the .cue in rom/
#
# Writes game/assets/{levels,screens,music}. Intermediate files go to
# research/ (gitignored). Needs: the psx toolkit (`psx` on PATH, or set PSX),
# python3 with Pillow, cargo. The baked files are copyrighted game content:
# they are gitignored and must never be committed.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cue="$(realpath "${1:-$(ls rom/*.cue | head -n1)}")"
psx="${PSX:-psx}"
work="${BAKE_WORK:-$root/research}"
out="${BAKE_OUT:-$root/game/assets}"

echo "== disc files: $cue"
"$psx" --root "$root" extract --all "$cue" -o "$work/disc" >/dev/null
echo "== levels: DATA/*.PSX -> raw tables"
python3 tools/extract_tr2.py --input "$work/disc/DATA" --output "$work/extracted/levels"
echo "== levels: raw tables -> engine format"
cargo run --quiet -p tr2-import -- --raw "$work/extracted/levels" --out "$out/levels"
echo "== screens"
python3 tools/extract_pix.py --input "$work/disc" --output "$out/screens"
echo "== music (CD audio tracks)"
python3 tools/extract_cdda.py --rom "$(dirname "$cue")" --output "$out/music"
echo "done: $out"
