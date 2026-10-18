#!/usr/bin/env python3
"""Retain PSX backward wall/step/ledge/slope evidence; no memory pokes.

Use --prepare-checkpoints once, then --case NAME. Independent validation
routes are recorded only after the controller has been implemented.
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

def event(buttons, until, frame=0):
    return dict(frame=frame, buttons=buttons, until=until)


CASES = {
    'angled_wall': ('backward_angled_start', 'assault', 420, [event(['DOWN', 'R1'], 380)]),
    'shallow_slope': ('backward_shallow_start', 'wall', 180, [event(['DOWN', 'R1'], 140)]),
    'ledge_release_heldout': ('home_hang_start', 'assault', 180, [event(['DOWN'], 62)]),
    'wall_delay_heldout': ('home_start', 'assault', 420, [event(['DOWN', 'R1'], 370, 18)]),
    'step_release_heldout': ('backward_step_down_start', 'assault', 300, [event(['DOWN', 'R1'], 170, 14)]),
    'slope_release_heldout': ('wall_game', 'wall', 140, [event(['DOWN'], 4)]),
    'wall_walk': ('home_start', 'assault', 400, [event(['DOWN', 'R1'], 350)]),
    'wall_hop': ('home_start', 'assault', 180, [event(['DOWN'], 130)]),
    'ledge_walk': ('home_hang_start', 'assault', 260, [event(['DOWN', 'R1'], 230)]),
    'ledge_hop': ('home_hang_start', 'assault', 180, [event(['DOWN'], 100)]),
    'step_up': ('backward_rolled_start', 'assault', 420, [event(['DOWN', 'R1'], 380)]),
    'step_down': ('backward_step_down_start', 'assault', 340, [event(['DOWN', 'R1'], 240)]),
    'slope_walk': ('wall_game', 'wall', 180, [event(['DOWN', 'R1'], 140)]),
    'slope_hop': ('wall_game', 'wall', 140, [event(['DOWN'], 100)]),
}


def run(command):
    subprocess.run(command, cwd=ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--case', choices=CASES)
    parser.add_argument('--prepare-checkpoints', action='store_true')
    parser.add_argument('--summarize-only', action='store_true')
    args = parser.parse_args()
    if args.prepare_checkpoints:
        for target, frames, inputs in [
            ('backward_angled_start', 100, [event(['LEFT'], 12)]),
            ('backward_rolled_start', 90, [event(['CIRCLE'], 2)]),
            ('backward_step_down_start', 240, [event(['UP', 'R1'], 160)]),
        ]:
            run(['psx', 'run', '--from-state', '.psx/home_start/state.bin',
                 '--frames', str(frames), '--input', json.dumps(inputs),
                 '--save-state', '--out', f'.psx/{target}', '--summary'])
        run(['psx', 'run', '--from-state', '.psx/wall_game/state.bin',
             '--frames', '340', '--input', json.dumps([
                 event(['RIGHT'], 56), event(['UP', 'CROSS'], 240, 60),
                 event(['UP', 'CROSS'], 300, 260)]),
             '--save-state', '--out', '.psx/backward_shallow_start', '--summary'])
        if args.case is None:
            return
    if args.case is None:
        parser.error('--case or --prepare-checkpoints is required')
    state, level, frames, inputs = CASES[args.case]
    checkpoint = f'.psx/{state}/state.bin'
    output = f'.psx/backward_collision_{args.case}'
    samples = SAMPLES.copy()
    if level == 'wall':
        # Lara's item is at 0x19cf30 in Wall, 0x17c87c in Home.
        samples = [f'{int(s[:8], 16) + 0x206b4:08x}{s[8:]}' if s.startswith('8017') else s
                   for s in samples]
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', str(frames),
               '--input', json.dumps(inputs), '--snapshot-every', '2',
               '--watch', '8008c572:2', '--max-events', '10000',
               '--save-state', '--out', output, '--summary']
    for sample in samples:
        command += ['--sample', sample]
    if not args.summarize_only:
        run(command)
    report = json.loads((ROOT / output / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    assert report['watch']['complete'] and not report.get('pokes')
    target = ROOT / f'game/tests/data/original/backward_collision_{args.case}.csv'
    shutil.copyfile(ROOT / output / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'heldout_after_implementation': args.case.endswith('_heldout'),
        'level': level, 'command': command, 'input': inputs, 'samples': samples,
        'build': report['build'], 'identity': report['identity'], 'frames': report['frames'],
        'checkpoint': checkpoint,
        'checkpoint_sha256': hashlib.sha256((ROOT / checkpoint).read_bytes()).hexdigest(),
        'samples_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'writes_sha256': hashlib.sha256((ROOT / output / 'writes.jsonl').read_bytes()).hexdigest(),
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
