#!/usr/bin/env python3
"""Retain inventory ring evidence from ordinary PSX pad routes.

Every case starts from the settled Wall checkpoint (`.psx/use_wall_game`)
and uses no pokes. The ring runs inside Display_Inventory (0x80030F30) with
its ring and motion structs on the stack (0x801FFDB8 and 0x801FFD80 when
opened from play); items are the 0x54-byte records from 0x80087FCC.
Cases named *_heldout were recorded only after game/src/inventory was
written and never used to fit it.
"""
import argparse
import csv
import json
import shutil
import subprocess

from record_backward_collision import ROOT, event

CHECKPOINT = '.psx/use_wall_game/state.bin'
# Stores of the frame's vsync count by Display_Inventory (end of a ring
# frame; its start sets 2) and of the input word by the pad reader.
RING_VSYNC_PC, RING_START_PC, INPUT_PC = '0x00031748', '0x00031158', '0x000697C0'
RING = 0x801FFDB8
MOTION = 0x801FFD80
# The Wall start's main ring, in ring order: statistics, pistols, shotgun,
# flare, large and small medi packs.
ITEMS = {'stats': 0x80087FCC, 'pistols': 0x80088020, 'shotgun': 0x800880C8, 'flare': 0x80088074,
         'large': 0x80088560, 'small': 0x8008850C}


def field(addr, fmt, name):
    return f'{addr:08x}:{fmt}:{name}'


SAMPLES = [
    '80087fc8:i32:vsyncs', '8008aad4:x32:input', '80089028:x32:input_db',
    field(RING, 'x32', 'list'), field(RING + 0x4, 'i16', 'ring_type'), field(RING + 0x6, 'i16', 'radius'),
    field(RING + 0x8, 'i16', 'camera_pitch'), field(RING + 0xA, 'i16', 'rotating'),
    field(RING + 0xC, 'i16', 'rot_count'), field(RING + 0xE, 'i16', 'current'),
    field(RING + 0x10, 'i16', 'target'), field(RING + 0x12, 'i16', 'count'),
    field(RING + 0x16, 'i16', 'rot_adder'), field(RING + 0x2A, 'i16', 'ring_yaw'),
    field(RING + 0x34, 'i32', 'camera_y'), field(RING + 0x38, 'i32', 'camera_z'),
    field(MOTION, 'i16', 'motion_count'), field(MOTION + 0x2, 'i16', 'status'),
    field(MOTION + 0x4, 'i16', 'status_target'),
    '80089032:i16:item_chosen', '8008c4ee:i16:request_gun', '8008c4ec:i16:gun_type',
    '80088aa0:i16:main_count', '80088b34:i16:keys_count', '8019cf52:i16:health',
] + [f'{0x80088AA8 + 2 * i:08x}:i16:qty{i}' for i in range(8)]
for name, addr in ITEMS.items():
    SAMPLES += [field(addr + 0x8, 'i16', f'{name}_frame'), field(addr + 0x16, 'i16', f'{name}_pt_xrot'),
                field(addr + 0x1C, 'i16', f'{name}_x_rot'), field(addr + 0x20, 'i16', f'{name}_y_rot'),
                field(addr + 0x28, 'i32', f'{name}_ytrans'), field(addr + 0x30, 'i32', f'{name}_ztrans'),
                field(addr + 0x38, 'x32', f'{name}_meshes')]

OPEN = event(['SELECT'], 14, 10)

# name: (frames, input)
CASES = {
    # Open, rotate both ways, visit the options ring, select and put back
    # the pistols, then close.
    'browse': (900, [OPEN, event(['RIGHT'], 84, 80), event(['LEFT'], 144, 140), event(['DOWN'], 204, 200),
                     event(['UP'], 304, 300), event(['LEFT'], 384, 380), event(['CROSS'], 444, 440),
                     event(['TRIANGLE'], 604, 600), event(['TRIANGLE'], 704, 700)]),
    # Open the statistics, back out, turn the ring and close it.
    'stats': (480, [OPEN, event(['CROSS'], 64, 60), event(['TRIANGLE'], 204, 200), event(['RIGHT'], 304, 300),
                    event(['TRIANGLE'], 384, 380)]),
    # Both medi packs chosen at full health (no effect), reopening between.
    'medipacks': (760, [OPEN, event(['RIGHT'], 64, 60), event(['CROSS'], 124, 120), event(['SELECT'], 304, 300),
                         event(['RIGHT'], 374, 370), event(['RIGHT'], 414, 410), event(['CROSS'], 484, 480),
                         event(['CROSS'], 564, 560)]),
    # Held out: the options ring, the passport chosen and put back, the
    # ring closed from the options ring.
    'passport_heldout': (520, [OPEN, event(['DOWN'], 84, 80), event(['CROSS'], 164, 160),
                               event(['TRIANGLE'], 324, 320), event(['TRIANGLE'], 424, 420)]),
}


def record(name):
    frames, inputs = CASES[name]
    out = f'validation/inventory/{name}'
    command = ['psx', 'run', '--from-state', CHECKPOINT, '--frames', str(frames), '--input', json.dumps(inputs),
               '--snapshot-every', '2', '--watch', '80087fc8:4', '--watch', '8008aad4:4', '--out', out, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert not report.get('pokes')
    shutil.copyfile(ROOT / out / 'samples.csv', ROOT / f'game/tests/data/original/inventory_{name}.csv')
    writes = [json.loads(line) for line in (ROOT / out / 'writes.jsonl').read_text().splitlines()]
    samples = list(csv.DictReader((ROOT / out / 'samples.csv').open()))
    frames_out = ring_frames(writes, samples)
    (ROOT / f'game/tests/data/original/inventory_{name}.json').write_text(json.dumps(dict(
        source='Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        checkpoint=CHECKPOINT, frames=frames, input=inputs, command=command,
        ring_frames=frames_out), indent=1) + '\n')


def ring_frames(writes, samples):
    """Display_Inventory's ring frames. Each starts at a store of the vsync
    count its motions use (2 when the ring opens, 0x31158; the frame before's
    count at its end, 0x31748), reads the next input word the pad reader
    stores, and shows its result in the sample one vblank later: the state
    step of the pass whose vsync wait ended there, then the next pass's
    motions and items. The frame that reaches status 13 is the last until
    the ring opens again."""
    frames, open_ = [], False
    for i, w in enumerate(writes):
        if w['address_hex'] != '0x00087FC8' or w['pc_hex'] not in (RING_START_PC, RING_VSYNC_PC):
            continue
        if w['pc_hex'] == RING_START_PC:
            open_ = True
        if not open_:
            continue
        sample = w['run_frame'] + 1
        word = next(x['value'] for x in writes[i:] if x['address_hex'] == '0x0008AAD4' and x['pc_hex'] == INPUT_PC)
        vsyncs = 2 if w['pc_hex'] == RING_START_PC else w['value']
        frames.append(dict(sample=sample, input=word, vsyncs=vsyncs, open=w['pc_hex'] == RING_START_PC))
        if samples[sample]['status'] == '13':
            open_ = False
    return frames


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('cases', nargs='*', default=list(CASES))
    for case in parser.parse_args().cases:
        record(case)
