#!/usr/bin/env python3
"""Retain controlled-spawn PSX low-ceiling replays and their provenance.

Only the checkpoint preparation changes the initial position/room/yaw. The
subsequent pad replays do not poke memory or change the original level/code.
"""
import argparse
import hashlib
import json
import shutil
import subprocess

from record_backward_collision import ROOT, SAMPLES, event

PREPARE = ['psx', 'run', '--from-state', '.psx/airborne_wall_start/state.bin',
           '--frames', '90', '--input', json.dumps([event([], 90)]),
           '--poke', '8017c8b0=37376', '--poke', '8017c8b4=3840',
           '--poke', '8017c8b8=91648', '--poke', '8017c8be=16384:2',
           '--poke', '8017c894=0:2', '--save-state', '--out', '.psx/ceiling_low_start', '--summary']
CASES = {
    'forward': (180, [event(['UP', 'SQUARE'], 22)]),
    'forward_delay_heldout': (200, [event(['UP', 'SQUARE'], 38, 16)]),
    'forward_release_heldout': (200, [event(['UP', 'SQUARE'], 8), event(['SQUARE'], 66, 56)]),
}


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare-checkpoint', action='store_true')
    parser.add_argument('--case', choices=CASES)
    args = parser.parse_args()
    if args.prepare_checkpoint:
        subprocess.run(PREPARE, cwd=ROOT, check=True)
    if args.case is None:
        if args.prepare_checkpoint:
            return
        parser.error('--case or --prepare-checkpoint is required')
    frames, inputs = CASES[args.case]
    checkpoint = '.psx/ceiling_low_start/state.bin'
    output = f'.psx/ceiling_{args.case}'
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--trace', '--trace-range', '8003c748-8003c760',
               '--max-events', '10000', '--watch', '8008c572:2',
               '--save-state', '--out', output, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / output / 'report.json').read_text())
    assert report['status'] == 'complete' and not report.get('pokes')
    assert report['watch']['complete'] and report['trace']['lines_dropped'] == 0
    target = ROOT / f'game/tests/data/original/ceiling_{args.case}.csv'
    shutil.copyfile(ROOT / output / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; controlled initial spawn, no replay pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        'level': 'assault', 'command': command, 'input': inputs, 'samples': SAMPLES,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint, 'checkpoint_sha256': digest(checkpoint),
        'checkpoint_preparation': PREPARE,
        'checkpoint_preparation_source_sha256': digest('.psx/airborne_wall_start/state.bin'),
        'samples_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'writes_sha256': digest(f'{output}/writes.jsonl'),
        'trace_sha256': digest(f'{output}/trace.txt'),
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
