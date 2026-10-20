#!/usr/bin/env python3
"""Retain PSX tiger AI routes from Wall: pad input only, no memory pokes.

Every case starts from `.psx/tiger_pre_start`, made by pad input from the
settled Wall checkpoint (run north, turn, run into room 36). Its floor trigger
activates tiger 18 at preparation field 557, after this checkpoint, so each
route includes the creature's activation, slot initialisation and first path
search. Cases named *_heldout were recorded only after the creature rules
were written and never used to refine them.
"""
import argparse
import hashlib
import json
import shutil
import subprocess

from record_backward_collision import ROOT, event

PREPARATION = [event(['UP'], 420), event(['RIGHT'], 500, 450), event(['UP'], 550, 530)]
START = '.psx/tiger_pre_start'

TIGER = 0x8019CE98  # item 18
LARA = 0x8019CF30  # item 20
SLOT = 0x801A29C0  # creature slot 0, which the tiger takes


def item(base, prefix, fields):
    return [f'{base + offset:08x}:{fmt}:{prefix}_{name}' for offset, fmt, name in fields]


ITEM = [
    (0x00, 'i32', 'floor'), (0x04, 'x32', 'touch'), (0x0E, 'i16', 'state'), (0x10, 'i16', 'goal'),
    (0x12, 'i16', 'required'), (0x14, 'i16', 'animation'), (0x16, 'i16', 'frame'), (0x18, 'i16', 'room'),
    (0x1E, 'i16', 'speed'), (0x20, 'i16', 'fall_speed'), (0x22, 'i16', 'health'), (0x24, 'i16', 'box'),
    (0x34, 'i32', 'x'), (0x38, 'i32', 'y'), (0x3C, 'i32', 'z'),
    (0x40, 'i16', 'pitch'), (0x42, 'i16', 'yaw'), (0x44, 'i16', 'roll'), (0x48, 'x32', 'status'),
]
CREATURE = [
    (0x00, 'i16', 'head'), (0x04, 'i16', 'max_turn'), (0x06, 'i16', 'flags'), (0x08, 'i16', 'item'),
    (0x0C, 'i32', 'mood'), (0x14, 'i16', 'lot_head'), (0x16, 'i16', 'lot_tail'), (0x18, 'u16', 'search'),
    (0x22, 'i16', 'zone_count'), (0x24, 'i16', 'target_box'), (0x26, 'i16', 'required_box'),
    (0x28, 'i32', 'lot_x'), (0x2C, 'i32', 'lot_y'), (0x30, 'i32', 'lot_z'),
    (0x34, 'i32', 'target_x'), (0x38, 'i32', 'target_y'), (0x3C, 'i32', 'target_z'),
]
SAMPLES = (item(TIGER, 'tiger', ITEM) + item(SLOT, 'creature', CREATURE) + item(LARA, 'lara', ITEM)
           + ['80089d54:x32:seed'])

# name: (fields, input)
CASES = {
    # Lara stops in room 36; the tiger runs up, attacks and bites.
    'stand': (500, [event(['UP'], 30)]),
    # Lara runs back the way she came while the tiger chases.
    'flee': (500, [event(['UP'], 30), event(['DOWN'], 160, 120), event(['UP'], 330, 170)]),
    # Draw and fire: hits set the tiger's hit flag, which its mood reads.
    'shoot': (500, [event(['UP'], 30), event(['TRIANGLE'], 44, 40), event(['RIGHT'], 120, 60),
                    event(['CROSS'], 500, 120)]),
    # Held out: recorded after the creature routines and tiger were written.
    # Lara keeps still for longer while the tiger lunges and leaps.
    'long_stand_heldout': (1200, [event(['UP'], 30)]),
    # Lara walks back and forth near the tiger.
    'walk_heldout': (900, [event(['UP'], 30), event(['DOWN', 'R1'], 300, 200),
                           event(['UP', 'R1'], 500, 320), event(['LEFT'], 560, 520), event(['UP', 'R1'], 700, 580)]),
    # Lara runs a wide circle; the tiger pounces at a moving target.
    'circle_heldout': (900, [event(['UP'], 30), event(['UP', 'LEFT'], 600, 200)]),
    # Lara runs back south toward the cave and keeps going.
    'run_back_heldout': (1200, [event(['UP'], 30), event(['DOWN'], 110, 100), event(['UP'], 700, 112)]),
}


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def prepare():
    command = ['psx', 'run', '--from-state', '.psx/wall_game/state.bin', '--frames', '550',
               '--input', json.dumps(PREPARATION), '--save-state', '--out', START, '--summary']
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / START / 'report.json').read_text())
    assert report['status'] == 'complete' and not report.get('pokes')
    return command


def record(name):
    frames, inputs = CASES[name]
    checkpoint = f'{START}/state.bin'
    if not (ROOT / checkpoint).exists():
        prepare()
    out = f'validation/tiger/{name}'
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2', '--watch', '80089d54:4',
               '--save-state', '--out', out, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert report['watch']['complete'] and not report.get('pokes')
    target = ROOT / f'game/tests/data/original/tiger_{name}.csv'
    shutil.copyfile(ROOT / out / 'samples.csv', target)
    writes = [json.loads(line) for line in (ROOT / out / 'writes.jsonl').read_text().splitlines()]
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'scope': 'Tiger 18 activation, creature slot 0, mood, path target and movement per field',
        'heldout_after_implementation': name.endswith('_heldout'),
        'level': 'wall', 'command': command, 'input': inputs, 'samples': SAMPLES,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint, 'checkpoint_sha256': digest(checkpoint),
        'checkpoint_preparation': dict(source='.psx/wall_game/state.bin', fields=550, input=PREPARATION),
        'samples_sha256': digest(target.relative_to(ROOT)),
        # Every control-random store (0x80089d54), with the storing PC.
        'random_writes': [dict(frame=w['frame'], pc=w['pc'], old=w['old'], value=w['value']) for w in writes],
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', action='store_true', help='rebuild the pre-activation checkpoint')
    parser.add_argument('--case', choices=CASES, action='append', default=[])
    parser.add_argument('--all', action='store_true')
    args = parser.parse_args()
    if args.prepare:
        prepare()
    for name in CASES if args.all else args.case:
        record(name)


if __name__ == '__main__':
    main()
