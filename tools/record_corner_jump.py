#!/usr/bin/env python3
"""Retain controlled-spawn PSX airborne side/corner contact replays.

Each case's checkpoint changes only Lara's initial position, room and yaw
(five frame-0 writes into the settled Home checkpoint, then 90 neutral fields).
The subsequent pad replays do not poke memory or change the level/code.
"""
import argparse
import hashlib
import json
import shutil
import subprocess

from record_backward_collision import ROOT, SAMPLES, event

SOURCE = '.psx/home_start/state.bin'
# name: (x, y, z, yaw, room). Room0 is the low (1024) corridor along +x;
# room7 is the tall (1536) hall with a north wall at z82944.
SPAWNS = {
    'low_left': (37376, 3840, 91900, 13384, 0),
    'low_right': (37376, 3840, 91400, 19384, 0),
    'open_left': (26112, -1280, 82600, 12384, 7),
    'open_right': (26112, -1280, 78100, 20384, 7),
}
# name: (spawn, fields, input). Cases recorded after the implementation end
# in _heldout and were never used to refine the rule.
CASES = {
    'low_left': ('low_left', 180, [event(['UP', 'SQUARE'], 22)]),
    'open_left': ('open_left', 180, [event(['UP', 'SQUARE'], 22)]),
    'open_reach': ('open_left', 200, [event(['UP', 'SQUARE', 'CROSS'], 22), event(['CROSS'], 100, 22)]),
    'open_right_heldout': ('open_right', 180, [event(['UP', 'SQUARE'], 22)]),
    'low_right_delay_heldout': ('low_right', 200, [event(['UP', 'SQUARE'], 38, 16)]),
    'open_reach_release_heldout': ('open_left', 240, [event(['UP', 'SQUARE', 'CROSS'], 30, 8),
                                                      event(['CROSS'], 50, 30), event(['UP'], 168, 136)]),
    'open_right_reach_validation_heldout': ('open_right', 260, [event(['UP', 'SQUARE', 'CROSS'], 26, 4),
                                                                event(['CROSS'], 60, 26), event(['UP'], 200, 140)]),
}


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def preparation(spawn):
    x, y, z, yaw, room = SPAWNS[spawn]
    return ['psx', 'run', '--from-state', SOURCE, '--frames', '90', '--input', json.dumps([event([], 90)]),
            '--poke', f'8017c8b0={x}', '--poke', f'8017c8b4={y}', '--poke', f'8017c8b8={z}',
            '--poke', f'8017c8be={yaw}:2', '--poke', f'8017c894={room}:2',
            '--save-state', '--out', f'.psx/corner_{spawn}_start', '--summary']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', choices=SPAWNS)
    parser.add_argument('--case', choices=CASES)
    args = parser.parse_args()
    if args.prepare:
        subprocess.run(preparation(args.prepare), cwd=ROOT, check=True)
    if args.case is None:
        if args.prepare:
            return
        parser.error('--case or --prepare is required')
    spawn, frames, inputs = CASES[args.case]
    checkpoint = f'.psx/corner_{spawn}_start/state.bin'
    output = f'.psx/corner_{args.case}'
    # Both airborne deflection routines: forward jump 0x8003C5F4 and reach 0x8003C7F4.
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--trace', '--trace-range', '8003c5f4-8003c918',
               '--max-events', '20000', '--watch', '8008c572:2',
               '--save-state', '--out', output, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / output / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames and not report.get('pokes')
    assert report['watch']['complete'] and report['trace']['lines_dropped'] == 0
    target = ROOT / f'game/tests/data/original/corner_{args.case}.csv'
    shutil.copyfile(ROOT / output / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; controlled initial spawn, no replay pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        # The reach release route exposed the run-stop prediction defect.
        'used_to_refine_rules': not args.case.endswith('_heldout') or args.case == 'open_reach_release_heldout',
        'level': 'assault', 'command': command, 'input': inputs, 'samples': SAMPLES,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint, 'checkpoint_sha256': digest(checkpoint),
        'checkpoint_preparation': preparation(spawn),
        'checkpoint_preparation_source_sha256': digest(SOURCE),
        'samples_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'writes_sha256': digest(f'{output}/writes.jsonl'),
        'trace_sha256': digest(f'{output}/trace.txt'),
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
