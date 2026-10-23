#!/usr/bin/env python3
"""Retain evidence of the title, game over and in-play passport rings.

As `record_inventory.py`, each case samples Display_Inventory's ring and
motion structs and the options ring's items every vsync and cuts the
ring's frames from the stores of its vsync count and input word; it also
samples the passport's page (`0x80089034`), the load choice (`0x80089038`)
and the ring's mode (`0x80089054`). The checkpoints have a formatted
memory card in slot 1 (`docs/save-evidence.md` says how they were made):

- `title`: from the disc's launcher (`.psx/card_boot`), CROSS starts Tomb
  Raider II; its first title ring opens on Lara's Home. No pokes.
- `death`: the Great Wall after loading the card's position
  (`.psx/card_wall_saved`); Lara's death count (`0x8008C502`) is poked to
  301 at one frame, which only brings the game over ring a tick later.
- `game_pages_heldout`: the same checkpoint, the passport in play turned
  to its last page and closed from there. No pokes.
"""
import argparse
import csv
import json
import subprocess

from record_backward_collision import ROOT, event
from record_inventory import INPUT_PC, RING_START_PC, RING_VSYNC_PC, field, ring_frames

# The options ring's items, by the names the tests use.
ITEMS = {'passport': 0x800888FC, 'screen': 0x80088950, 'sound': 0x800889A4, 'controls': 0x800889F8,
         'home': 0x80088A4C}
# The title runs Display_Inventory from the front end, deeper in the stack.
RINGS = {'title': (0x801FFEC8, 0x801FFE90), 'play': (0x801FFDB8, 0x801FFD80)}


def samples(ring, motion):
    out = [
        '80087fc8:i32:vsyncs', '8008aad4:x32:input', '80089028:x32:input_db', '80089054:i32:mode',
        '80089034:i32:page', '80089038:i32:choice', '80089032:i16:item_chosen',
        field(ring, 'x32', 'list'), field(ring + 0x4, 'i16', 'ring_type'), field(ring + 0x6, 'i16', 'radius'),
        field(ring + 0x8, 'i16', 'camera_pitch'), field(ring + 0xA, 'i16', 'rotating'),
        field(ring + 0xC, 'i16', 'rot_count'), field(ring + 0xE, 'i16', 'current'),
        field(ring + 0x10, 'i16', 'target'), field(ring + 0x12, 'i16', 'count'),
        field(ring + 0x16, 'i16', 'rot_adder'), field(ring + 0x2A, 'i16', 'ring_yaw'),
        field(ring + 0x34, 'i32', 'camera_y'), field(ring + 0x38, 'i32', 'camera_z'),
        field(motion, 'i16', 'motion_count'), field(motion + 0x2, 'i16', 'status'),
        field(motion + 0x4, 'i16', 'status_target'),
    ]
    for name, addr in ITEMS.items():
        out += [field(addr + 0x4, 'i16', f'{name}_object'), field(addr + 0x8, 'i16', f'{name}_frame'),
                field(addr + 0xA, 'i16', f'{name}_goal'), field(addr + 0x16, 'i16', f'{name}_pt_xrot'),
                field(addr + 0x1C, 'i16', f'{name}_x_rot'), field(addr + 0x20, 'i16', f'{name}_y_rot'),
                field(addr + 0x28, 'i32', f'{name}_ytrans'), field(addr + 0x30, 'i32', f'{name}_ztrans')]
    return out


OPEN = event(['SELECT'], 14, 10)

# name: (checkpoint, ring, frames, input, pokes)
CASES = {
    # The launcher's CROSS starts the game; at the title, Left to the
    # passport, Cross opens it on Load Game, Right to New Game, Right again
    # (the title's last page), Left back, Triangle closes it, Right turns
    # the ring back to Lara's Home.
    'title': ('.psx/card_boot/state.bin', 'title', 5400,
              [event(['CROSS'], 3606, 3600), event(['LEFT'], 4506, 4500), event(['CROSS'], 4666, 4660),
               event(['RIGHT'], 4826, 4820), event(['RIGHT'], 4906, 4900), event(['LEFT'], 4986, 4980),
               event(['TRIANGLE'], 5066, 5060), event(['RIGHT'], 5186, 5180)], []),
    # Game over: the passport opens by itself on Load Game; Right to
    # Restart Level, Right to Exit to Title, Triangle (ignored), Left, Left,
    # Left (no further), Right, and Cross on Restart Level.
    'death': ('.psx/card_wall_saved/state.bin', 'play', 760,
              [event(['RIGHT'], 126, 120), event(['RIGHT'], 206, 200), event(['TRIANGLE'], 286, 280),
               event(['LEFT'], 366, 360), event(['LEFT'], 426, 420), event(['LEFT'], 486, 480),
               event(['RIGHT'], 546, 540), event(['CROSS'], 606, 600)], ['8008C502=301:2@10']),
    # Held out: the options ring's passport in play turned to Save Game and
    # Exit to Title, Right again (no further), Triangle from the last page
    # (on to its last frame), and the ring closed.
    'game_pages_heldout': ('.psx/card_wall_saved/state.bin', 'play', 640,
                           [OPEN, event(['DOWN'], 84, 80), event(['CROSS'], 164, 160), event(['RIGHT'], 286, 280),
                            event(['RIGHT'], 346, 340), event(['RIGHT'], 406, 400), event(['TRIANGLE'], 466, 460),
                            event(['TRIANGLE'], 566, 560)], []),
}


def record(name):
    checkpoint, ring, frames, inputs, pokes = CASES[name]
    out = f'validation/save/{name}'
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames), '--input', json.dumps(inputs),
               '--watch', '80087fc8:4', '--watch', '8008aad4:4', '--out', out, '--summary']
    for poke in pokes:
        command += ['--poke', poke]
    for sample in samples(*RINGS[ring]):
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert len(report.get('pokes') or []) == len(pokes)
    writes = [json.loads(line) for line in (ROOT / out / 'writes.jsonl').read_text().splitlines()]
    rows = list(csv.DictReader((ROOT / out / 'samples.csv').open()))
    frames_out = ring_frames(writes, rows)
    # Only the samples from the ring's first frame on are kept (the title's
    # route boots the game first); ring frames name samples from there.
    first = frames_out[0]['sample']
    lines = (ROOT / out / 'samples.csv').read_text().splitlines(keepends=True)
    (ROOT / f'game/tests/data/original/passport_{name}.csv').write_text(''.join([lines[0]] + lines[1 + first:]))
    for f in frames_out:
        f['sample'] -= first
    (ROOT / f'game/tests/data/original/passport_{name}.json').write_text(json.dumps(dict(
        source='Supplied Tomb Raider II USA Rev 3 disc via psx native',
        checkpoint=checkpoint, frames=frames, input=inputs, pokes=pokes, command=command,
        first_sample=first, ring_frames=frames_out), indent=1) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('cases', nargs='*', default=list(CASES))
    for case in parser.parse_args().cases:
        record(case)
