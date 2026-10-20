#!/usr/bin/env python3
"""Retain controlled-spawn PSX side-jump, running-twist and hop-reach replays.

The checkpoints are record_fall_recovery.py's spawns around room5's raised
block: five frame-0 writes into the settled Home checkpoint, then 90 neutral
fields. The pad replays do not poke memory.
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
    'left_pit': ('pit_edge_side', 200, [event(['LEFT', 'SQUARE'], 20)]),
    'right_block': ('pit_edge_side', 120, [event(['RIGHT', 'SQUARE'], 20)]),
    'right_wall': ('pit_floor_side', 120, [event(['RIGHT', 'SQUARE'], 20)]),
    'run_twist': ('pit_floor_run', 200, [event(['UP'], 40), event(['UP', 'SQUARE'], 46, 40), event(['DOWN'], 60, 46)]),
    'hop_reach': ('pit_edge_back', 200, [event(['DOWN'], 20), event(['DOWN', 'CROSS'], 30, 20), event(['CROSS'], 200, 30)]),
    'right_pit_heldout': ('pit_edge_side_right', 200, [event(['RIGHT', 'SQUARE'], 20)]),
    'left_wall_heldout': ('pit_floor_side_left', 120, [event(['LEFT', 'SQUARE'], 20)]),
    'left_over_right_heldout': ('pit_edge_side', 200, [event(['LEFT', 'RIGHT', 'SQUARE'], 20)]),
    'forward_over_right_heldout': ('pit_floor_side', 140, [event(['UP', 'RIGHT', 'SQUARE'], 20)]),
    'wall_then_back_heldout': ('pit_floor_side', 140, [event(['RIGHT', 'DOWN', 'SQUARE'], 20)]),
    'standing_back_heldout': ('pit_edge_back', 200, [event(['DOWN', 'SQUARE'], 20)]),
    'side_twist_heldout': ('pit_edge_side', 200, [event(['LEFT', 'SQUARE'], 20), event(['RIGHT'], 40, 24)]),
    'run_twist_roll_heldout': ('pit_floor_run', 200, [event(['UP'], 40), event(['UP', 'SQUARE'], 46, 40),
                                                      event(['CIRCLE'], 60, 48)]),
    'run_turn_jump_heldout': ('pit_floor_run', 160, [event(['UP'], 36), event(['UP', 'LEFT', 'SQUARE'], 46, 36)]),
    'hop_reach_late_heldout': ('pit_edge_back', 200, [event(['DOWN'], 20), event(['CROSS'], 200, 40)]),
    'walk_jump_heldout': ('pit_floor_run', 120, [event(['UP', 'R1'], 20), event(['UP', 'R1', 'SQUARE'], 40, 20)]),
}

# Held-out routes that then exposed a rule: the stop handler's Jump skipping
# its turn, and reach control's fast-fall request.
REFINED = {'forward_over_right_heldout', 'hop_reach_late_heldout'}


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', action='store_true', help="rebuild the case's spawn checkpoint first")
    parser.add_argument('--case', choices=CASES, required=True)
    args = parser.parse_args()
    spawn, frames, inputs = CASES[args.case]
    if args.prepare:
        subprocess.run(preparation(spawn), cwd=ROOT, check=True)
    checkpoint = f'.psx/fall_{spawn}_start/state.bin'
    output = f'.psx/side_jump_{args.case}'
    # Compression control 0x800390C8 and side-jump controls 0x80039638/9C.
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--trace', '--trace-range', '800390c8-80039270',
               '--max-events', '20000', '--watch', '8008c572:2',
               '--save-state', '--out', output, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / output / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames and not report.get('pokes')
    assert report['watch']['complete'] and report['trace']['lines_dropped'] == 0
    target = ROOT / f'game/tests/data/original/side_jump_{args.case}.csv'
    shutil.copyfile(ROOT / output / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; controlled initial spawn, no replay pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        'used_to_refine_rules': not args.case.endswith('_heldout') or args.case in REFINED,
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
