#!/usr/bin/env python3
"""Retain PSX traversal evidence and its exact replay/provenance, without pokes.

Prepare the first checkpoint from the existing unmodified Home start:
  psx run --from-state .psx/home_start/state.bin --frames 220 --input \
    '[{"frame":0,"buttons":["UP","CROSS"],"until":150}]' \
    --save-state --out .psx/home_three_click_start --summary

Run held-out cases only after implementing the corresponding rules.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SAMPLES = [
    '8017c8b0:i32:x', '8017c8b4:i32:y', '8017c8b8:i32:z', '8017c8be:i16:yaw',
    '8017c890:i16:animation', '8017c892:i16:animation_frame',
    '8017c88a:i16:state', '8017c88c:i16:goal', '8017c894:i16:room',
    '8017c89a:i16:speed', '8017c89c:i16:fall_speed', '8017c89e:i16:health',
    '8008c570:i16:turn_rate',
    '8008b258:i32:camera_x', '8008b25c:i32:camera_y', '8008b260:i32:camera_z',
    '8008b268:i32:target_x', '8008b26c:i32:target_y', '8008b270:i32:target_z',
]
CASES = {
    'standing_jump': ('.psx/home_side/state.bin', 180, [
        {'frame': 0, 'buttons': ['SQUARE'], 'until': 10},
    ]),
    'running_jump_validation_heldout': ('.psx/home_runway_start/state.bin', 180, [
        {'frame': 0, 'buttons': ['UP'], 'until': 26},
        {'frame': 26, 'buttons': ['UP', 'SQUARE'], 'until': 58},
    ]),
    'landing_run_heldout': ('.psx/home_side/state.bin', 120, [
        {'frame': 0, 'buttons': ['UP', 'SQUARE'], 'until': 22},
        {'frame': 22, 'buttons': ['UP'], 'until': 120},
    ]),
    'landing_cancel_heldout': ('.psx/home_side/state.bin', 240, [
        {'frame': 0, 'buttons': ['UP', 'SQUARE'], 'until': 22},
        {'frame': 96, 'buttons': ['SQUARE'], 'until': 108},
    ]),
    'running_jump': ('.psx/home_runway_start/state.bin', 200, [
        {'frame': 0, 'buttons': ['UP'], 'until': 60},
        {'frame': 60, 'buttons': ['UP', 'SQUARE'], 'until': 70},
    ]),
    'running_jump_heldout': ('.psx/home_runway_start/state.bin', 180, [
        {'frame': 0, 'buttons': ['UP'], 'until': 20},
        {'frame': 20, 'buttons': ['UP', 'SQUARE'], 'until': 60},
    ]),
    'forward_drop_heldout': ('.psx/home_hang_run_start/state.bin', 220, [
        {'frame': 0, 'buttons': ['UP', 'SQUARE', 'CROSS'], 'until': 30},
        {'frame': 30, 'buttons': ['CROSS'], 'until': 90},
    ]),
    'hang_no_action_heldout': ('.psx/home_hang_start/state.bin', 180, [
        {'frame': 0, 'buttons': ['UP', 'CROSS'], 'until': 20},
    ]),
    'forward_landing': ('.psx/home_side/state.bin', 160, [
        {'frame': 0, 'buttons': ['UP', 'SQUARE'], 'until': 22},
    ]),
    'forward_climb': ('.psx/home_hang_run_start/state.bin', 360, [
        {'frame': 0, 'buttons': ['UP', 'SQUARE', 'CROSS'], 'until': 30},
        {'frame': 30, 'buttons': ['CROSS'], 'until': 160},
        {'frame': 160, 'buttons': ['UP', 'CROSS'], 'until': 180},
    ]),
    'hang_early_climb_heldout': ('.psx/home_hang_start/state.bin', 250, [
        {'frame': 0, 'buttons': ['UP', 'CROSS'], 'until': 44},
    ]),
    'hang_early_drop_heldout': ('.psx/home_hang_start/state.bin', 180, [
        {'frame': 0, 'buttons': ['UP', 'CROSS'], 'until': 40},
        {'frame': 40, 'buttons': ['CROSS'], 'until': 44},
    ]),
    'hang_climb': ('.psx/home_hang_start/state.bin', 360, [
        {'frame': 0, 'buttons': ['UP', 'CROSS'], 'until': 38},
        {'frame': 38, 'buttons': ['CROSS'], 'until': 140},
        {'frame': 140, 'buttons': ['UP', 'CROSS'], 'until': 148},
    ]),
    'hang_drop': ('.psx/home_hang_start/state.bin', 240, [
        {'frame': 0, 'buttons': ['UP', 'CROSS'], 'until': 38},
        {'frame': 38, 'buttons': ['CROSS'], 'until': 100},
    ]),
    'three_click': ('.psx/home_three_click_start/state.bin', 220, [
        {'frame': 0, 'buttons': ['UP', 'CROSS'], 'until': 60},
    ]),
    'three_click_heldout': ('.psx/home_three_click_start/state.bin', 240, [
        {'frame': 14, 'buttons': ['UP', 'CROSS'], 'until': 34},
        {'frame': 40, 'buttons': ['RIGHT'], 'until': 100},
    ]),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=CASES)
    parser.add_argument('--prepare-checkpoints', action='store_true')
    parser.add_argument('--summarize-only', action='store_true')
    args = parser.parse_args()
    if args.prepare_checkpoints:
        recipes = [
            ('.psx/home_start/state.bin', '.psx/home_three_click_start', 220, ['UP', 'CROSS'], 150),
            ('.psx/home_start/state.bin', '.psx/home_course_probe', 500, ['UP', 'CROSS'], 500),
            ('.psx/home_course_probe/state.bin', '.psx/home_hang_start', 190, ['UP', 'CROSS'], 162),
            ('.psx/home_course_probe/state.bin', '.psx/home_hang_run_start', 200, ['UP', 'CROSS'], 132),
            ('.psx/home_side/state.bin', '.psx/home_runway_start', 380, ['DOWN', 'R1'], 300),
        ]
        for source, output, frames, buttons, until in recipes:
            subprocess.run(['psx', 'run', '--from-state', source, '--frames', str(frames),
                            '--input', json.dumps([{'frame': 0, 'buttons': buttons, 'until': until}]),
                            '--save-state', '--out', output, '--summary'], cwd=ROOT, check=True)
        if args.case is None:
            return
    if args.case is None:
        parser.error('--case or --prepare-checkpoints is required')
    checkpoint, frames, inputs = CASES[args.case]
    run = f'.psx/traversal_{args.case}'
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--watch', '8008c572:2', '--max-events', '10000',
               '--save-state', '--out', run, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    if not args.summarize_only:
        subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / run / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert report['watch']['complete']
    assert not report.get('pokes')
    target = ROOT / f'game/tests/data/original/traversal_{args.case}.csv'
    shutil.copyfile(ROOT / run / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        'used_to_refine_rules': args.case == 'running_jump_heldout',
        'command': command, 'input': inputs, 'samples': SAMPLES,
        'build': report['build'], 'identity': report['identity'],
        'frames': report['frames'], 'checkpoint': checkpoint,
        'checkpoint_sha256': hashlib.sha256((ROOT / checkpoint).read_bytes()).hexdigest(),
        'samples_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'writes_sha256': hashlib.sha256((ROOT / run / 'writes.jsonl').read_bytes()).hexdigest(),
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
