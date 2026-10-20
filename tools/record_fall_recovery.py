#!/usr/bin/env python3
"""Retain controlled-spawn PSX fast-fall and slide-edge landing replays.

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
# name: (x, y, z, yaw, room). Room5's raised block (floor1024) ends at a
# 2048-unit pit; room36's flat top (floor0) overlooks room49's steep ramp,
# which ends about1280 above room91's ground. (The mirrored room48 ramp over
# room16 stalls the native core's display and logic on its slide-off tick.)
SPAWNS = {
    'pit': (19968, 1024, 79360, 0, 5),
    'ramp_top': (31232, 0, 88576, 16384, 36),
    'ramp_back': (31488, 0, 88576, -16384, 36),
    'pit_angled': (19712, 1024, 79200, 2500, 5),
    'ramp_back_near': (31360, 0, 88320, -16384, 36),
}
# name: (spawn, fields, input). Cases recorded after the implementation end
# in _heldout and were never used to refine the rule.
CASES = {
    'pit_jump': ('pit', 200, [event(['UP', 'SQUARE'], 22)]),
    'ramp_run': ('ramp_top', 240, [event(['UP'], 40)]),
    'ramp_hop_back': ('ramp_back', 240, [event(['DOWN'], 30)]),
    'pit_angled_heldout': ('pit_angled', 220, [event(['UP', 'SQUARE'], 30, 8), event(['UP'], 200, 150)]),
    'ramp_run_late_heldout': ('ramp_top', 260, [event(['UP'], 60, 20), event(['UP'], 240, 190)]),
    'ramp_back_near_heldout': ('ramp_back_near', 240, [event(['DOWN'], 20)]),
}


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def preparation(spawn):
    x, y, z, yaw, room = SPAWNS[spawn]
    return ['psx', 'run', '--from-state', SOURCE, '--frames', '90', '--input', json.dumps([event([], 90)]),
            '--poke', f'8017c8b0={x}', '--poke', f'8017c8b4={y}', '--poke', f'8017c8b8={z}',
            '--poke', f'8017c8be={yaw}:2', '--poke', f'8017c894={room}:2',
            '--save-state', '--out', f'.psx/fall_{spawn}_start', '--summary']


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
    checkpoint = f'.psx/fall_{spawn}_start/state.bin'
    output = f'.psx/fall_{args.case}'
    # Slide collision 0x8003C364 and fast-fall collision 0x8003AEEC.
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--trace', '--trace-range', '8003c364-8003c4c4',
               '--max-events', '20000', '--watch', '8008c572:2',
               '--save-state', '--out', output, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / output / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames and not report.get('pokes')
    assert report['watch']['complete'] and report['trace']['lines_dropped'] == 0
    target = ROOT / f'game/tests/data/original/fall_{args.case}.csv'
    shutil.copyfile(ROOT / output / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; controlled initial spawn, no replay pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        'used_to_refine_rules': not args.case.endswith('_heldout'),
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
