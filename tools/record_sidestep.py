#!/usr/bin/env python3
"""Record PSX-only sidestep controls and authored stops, without memory pokes.

The held-out routes are recorded after implementation. --summarize-only can
retain runs made through the psx MCP with the same arguments.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHECKPOINT = '.psx/home_side/state.bin'
SAMPLES = [
    '8017c8be:i16:yaw', '8017c8b0:i32:x', '8017c8b4:i32:y', '8017c8b8:i32:z',
    '8017c890:i16:animation', '8017c892:i16:animation_frame',
    '8017c88a:i16:state', '8017c88c:i16:goal', '8017c89a:i16:speed',
    '8017c89c:i16:fall_speed', '8008c570:i16:turn_rate',
]
CAMERA_SAMPLES = [
    '8008b258:i32:camera_x', '8008b25c:i32:camera_y', '8008b260:i32:camera_z',
    '8008b268:i32:target_x', '8008b26c:i32:target_y', '8008b270:i32:target_z',
]
CASES = {
    'left_clear': (100, [{'frame': 0, 'buttons': ['R2', 'LEFT'], 'until': 20}]),
    'right_base': (140, [{'frame': 0, 'buttons': ['R2', 'RIGHT'], 'until': 80}]),
    'walk_left': (140, [{'frame': 0, 'buttons': ['R1', 'LEFT'], 'until': 80}]),
    'walk_right': (140, [{'frame': 0, 'buttons': ['R1', 'RIGHT'], 'until': 80}]),
    'left': (140, [{'frame': 0, 'buttons': ['L2'], 'until': 80}]),
    'right': (140, [{'frame': 0, 'buttons': ['R2'], 'until': 80}]),
    'steer_heldout': (100, [
        {'frame': 0, 'buttons': ['R2', 'LEFT', 'UP'], 'until': 24},
        {'frame': 24, 'buttons': ['R2', 'LEFT'], 'until': 30},
    ]),
    'rehold_heldout': (160, [
        {'frame': 0, 'buttons': ['R2', 'RIGHT', 'DOWN'], 'until': 6},
        {'frame': 6, 'buttons': ['R2', 'RIGHT'], 'until': 10},
        {'frame': 14, 'buttons': ['R2', 'RIGHT'], 'until': 80},
        {'frame': 80, 'buttons': ['R2', 'LEFT'], 'until': 100},
    ]),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=CASES, required=True)
    parser.add_argument('--psx', default='psx')
    parser.add_argument('--camera', action='store_true', help='Also retain native camera positions for visual comparison')
    parser.add_argument('--summarize-only', action='store_true')
    args = parser.parse_args()
    frames, inputs = CASES[args.case]
    run = f'.psx/sidestep_{args.case}'
    command = [args.psx, 'run', '--from-state', CHECKPOINT, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '1',
               '--watch', '8008c572:2', '--max-events', '10000', '--out', run, '--summary']
    samples = SAMPLES + (CAMERA_SAMPLES if args.camera else [])
    for sample in samples:
        command += ['--sample', sample]
    if not args.summarize_only:
        subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / run / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert report['watch']['complete']
    target = ROOT / f'game/tests/data/original/sidestep_{args.case}.csv'
    shutil.copyfile(ROOT / run / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        'command': command, 'input': inputs, 'samples': samples,
        'build': report['build'], 'frames': report['frames'], 'checkpoint': CHECKPOINT,
        'checkpoint_sha256': hashlib.sha256((ROOT / CHECKPOINT).read_bytes()).hexdigest(),
        'samples_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'writes_sha256': hashlib.sha256((ROOT / run / 'writes.jsonl').read_bytes()).hexdigest(),
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
