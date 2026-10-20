#!/usr/bin/env python3
"""Retain PSX spider AI routes from Wall's room 42 corridor; pad input only.

The checkpoint `.psx/spider_corridor_start` places Lara one sector short of
the floor trigger that activates spiders 24-27: five frame-0 writes (her x,
y, z, yaw and room) into the settled Wall checkpoint, then 90 neutral
fields. The replays themselves do not poke memory. Cases named *_heldout
were recorded only after the spider was written and never used to refine it.
"""
import argparse
import hashlib
import json
import shutil
import subprocess

from record_backward_collision import ROOT, event
from record_tiger import CREATURE, ITEM, item

START = '.psx/spider_corridor_start'
PLACEMENT = dict(x=19968, y=-256, z=36352, yaw=-16384, room=42)
ITEMS = 0x8019C940  # item 0; 76 bytes each
LARA = ITEMS + 20 * 76
SLOTS = 0x801A29C0  # creature slots, 68 bytes each
SPIDERS = [24, 25, 26, 27]

SAMPLES = (item(LARA, 'lara', ITEM)
           + [s for n in SPIDERS for s in item(ITEMS + n * 76, f'spider{n}', ITEM)]
           + [s for k in range(4) for s in item(SLOTS + k * 68, f'slot{k}', CREATURE)]
           + ['80089d54:x32:seed'])

# name: (fields, input)
CASES = {
    # Lara walks onto the trigger and stands; the spiders drop and bite.
    'stand': (600, [event(['UP', 'R1'], 120)]),
    # Lara walks onto the trigger, then runs back east past the corridor end.
    'retreat': (600, [event(['UP', 'R1'], 120), event(['DOWN'], 170, 160), event(['UP'], 460, 172)]),
    # Held out: recorded after the spider was written.
    'long_stand_heldout': (1200, [event(['UP', 'R1'], 120)]),
    'walk_heldout': (900, [event(['UP', 'R1'], 400), event(['DOWN', 'R1'], 600, 420), event(['LEFT'], 640, 600),
                           event(['UP', 'R1'], 900, 640)]),
    # Draw and fire at the spiders as they come; dead ones burst.
    'shoot_heldout': (900, [event(['UP', 'R1'], 120), event(['TRIANGLE'], 134, 130), event(['CROSS'], 900, 180)]),
}


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def preparation():
    p = PLACEMENT
    return ['psx', 'run', '--from-state', '.psx/wall_game/state.bin', '--frames', '90',
            '--input', json.dumps([event([], 90)]),
            '--poke', f'{LARA + 0x34:08x}={p["x"]}', '--poke', f'{LARA + 0x38:08x}={p["y"]}',
            '--poke', f'{LARA + 0x3C:08x}={p["z"]}', '--poke', f'{LARA + 0x42:08x}={p["yaw"]}:2',
            '--poke', f'{LARA + 0x18:08x}={p["room"]}:2',
            '--save-state', '--out', START, '--summary']


def record(name):
    frames, inputs = CASES[name]
    checkpoint = f'{START}/state.bin'
    if not (ROOT / checkpoint).exists():
        subprocess.run(preparation(), cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    out = f'validation/spider/{name}'
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '4', '--watch', '80089d54:4',
               '--save-state', '--out', out, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    report = json.loads((ROOT / out / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert report['watch']['complete'] and not report.get('pokes')
    target = ROOT / f'game/tests/data/original/spider_{name}.csv'
    shutil.copyfile(ROOT / out / 'samples.csv', target)
    writes = [json.loads(line) for line in (ROOT / out / 'writes.jsonl').read_text().splitlines()]
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes in the replay',
        'scope': 'Spiders 24-27 activation, creature slots 0-3, mood, path target and movement per field',
        'heldout_after_implementation': name.endswith('_heldout'),
        'level': 'wall', 'command': command, 'input': inputs, 'samples': SAMPLES,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint, 'checkpoint_sha256': digest(checkpoint),
        'checkpoint_preparation': dict(command=preparation(), placement=PLACEMENT),
        'samples_sha256': digest(target.relative_to(ROOT)),
        'random_writes': [dict(frame=w['frame'], pc=w['pc'], old=w['old'], value=w['value']) for w in writes],
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', action='store_true', help='rebuild the corridor checkpoint')
    parser.add_argument('--case', choices=CASES, action='append', default=[])
    parser.add_argument('--all', action='store_true')
    args = parser.parse_args()
    if args.prepare:
        subprocess.run(preparation(), cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
    for name in CASES if args.all else args.case:
        record(name)


if __name__ == '__main__':
    main()
