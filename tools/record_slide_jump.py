#!/usr/bin/env python3
"""Retain controlled-spawn PSX slide-jump and mid-air twist replays.

The checkpoints are the fast-fall milestone's ramp spawns (see
record_fall_recovery.py): five frame-0 writes into the settled Home
checkpoint, then 90 neutral fields. The pad replays do not poke memory.
"""
import argparse
import hashlib
import json
import shutil
import subprocess

from record_backward_collision import ROOT, SAMPLES, event
from record_fall_recovery import SOURCE, SPAWNS, preparation

# name: (spawn, fields, input). Cases recorded after the implementation end
# in _heldout and were never used to refine the rule.
CASES = {
    'forward': ('ramp_top', 240, [event(['UP'], 40), event(['SQUARE'], 90, 72)]),
    'back': ('ramp_back', 240, [event(['DOWN'], 30), event(['SQUARE'], 84, 64)]),
    'twist_pit': ('pit', 200, [event(['UP', 'SQUARE'], 22), event(['DOWN'], 30, 22)]),
    'twist_slide_back': ('ramp_back', 240, [event(['DOWN'], 30), event(['SQUARE'], 68, 64),
                                       event(['UP', 'SQUARE'], 74, 68), event(['SQUARE'], 84, 74)]),
    'twist_roll_heldout': ('pit', 200, [event(['UP', 'SQUARE'], 22), event(['CIRCLE'], 30, 26)]),
    'twist_late_heldout': ('pit', 200, [event(['UP', 'SQUARE'], 22), event(['DOWN'], 44, 34)]),
    'back_twist_roll_heldout': ('ramp_back_near', 240, [event(['DOWN'], 20), event(['SQUARE'], 78, 30),
                                                        event(['SQUARE', 'CIRCLE'], 82, 78), event(['SQUARE'], 90, 82)]),
    'hop_pit_heldout': ('pit_edge_back', 200, [event(['DOWN'], 20)]),
    'forward_late_heldout': ('ramp_top', 240, [event(['UP'], 40), event(['SQUARE'], 90, 74)]),
    'forward_away_heldout': ('ramp_top', 240, [event(['UP'], 40), event(['DOWN', 'SQUARE'], 90, 70)]),
    'back_held_heldout': ('ramp_back_near', 240, [event(['DOWN'], 20), event(['SQUARE'], 90, 30)]),
    'back_away_heldout': ('ramp_back', 240, [event(['DOWN'], 30), event(['UP', 'SQUARE'], 84, 62)]),
}


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', choices=SPAWNS)
    parser.add_argument('--case', choices=CASES, required=True)
    args = parser.parse_args()
    spawn, frames, inputs = CASES[args.case]
    if args.prepare:
        subprocess.run(preparation(spawn), cwd=ROOT, check=True)
    checkpoint = f'.psx/fall_{spawn}_start/state.bin'
    output = f'.psx/slide_jump_{args.case}'
    # Forward-slide control 0x80039598 and back-slide control 0x80039808.
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--trace', '--trace-range', '80039598-80039830',
               '--max-events', '20000', '--watch', '8008c572:2',
               '--save-state', '--out', output, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / output / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames and not report.get('pokes')
    assert report['watch']['complete'] and report['trace']['lines_dropped'] == 0
    target = ROOT / f'game/tests/data/original/slide_jump_{args.case}.csv'
    shutil.copyfile(ROOT / output / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; controlled initial spawn, no replay pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        # The away route exposed forward flight's twist request.
        'used_to_refine_rules': not args.case.endswith('_heldout') or args.case == 'forward_away_heldout',
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
