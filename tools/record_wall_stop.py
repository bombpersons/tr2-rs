#!/usr/bin/env python3
"""Record a post-implementation wall-stop hold/release/retry from the disc.

Uses only psx and the unmodified settled Wall checkpoint. The short trace
covers the first blocked run-start after the hit-wall animation finishes.
No positions, animation fields or other memory are poked.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHECKPOINT = '.psx/wall_game/state.bin'
RUN = '.psx/wall_stop_heldout'
INPUT = [
    {'frame': 0, 'buttons': ['RIGHT'], 'until': 56},
    {'frame': 60, 'buttons': ['UP', 'CROSS'], 'until': 240},
    {'frame': 260, 'buttons': ['UP', 'CROSS'], 'until': 300},
]
SAMPLES = [
    '8019cf72:i16:yaw', '8019cf64:i32:x', '8019cf68:i32:y', '8019cf6c:i32:z',
    '8019cf44:i16:animation', '8019cf46:i16:animation_frame',
    '8019cf3e:i16:state', '8019cf40:i16:goal', '8019cf4e:i16:speed', '8019cf50:i16:fall_speed',
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--psx', default='psx', help='Path to the psx executable')
    parser.add_argument('--summarize-only', action='store_true')
    args = parser.parse_args()
    command = [args.psx, 'run', '--from-state', CHECKPOINT, '--frames', '340',
               '--input', json.dumps(INPUT), '--watch', '8019cf64:12',
               '--trace', '--trace-from-frame', '216', '--trace-frames', '6',
               '--trace-range', '8003a844-8003aa08', '--trace-range', '8003a500-8003a660',
               '--trace-range', '8003c318-8003c55c', '--trace-range', '8001b018-8001b150',
               '--max-events', '20000', '--out', RUN, '--summary']
    for sample in SAMPLES:
        command += ['--sample', sample]
    if not args.summarize_only:
        subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((ROOT / RUN / 'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == 340
    assert report['trace']['lines_dropped'] == 0 and report['watch']['complete']
    target = ROOT / 'game/tests/data/original/wall_stop_heldout.csv'
    shutil.copyfile(ROOT / RUN / 'samples.csv', target)
    metadata = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'description': 'Climb, hit the wall, hold UP, release, retry UP, release again',
        'heldout_after_implementation': True,
        'command': command, 'input': INPUT, 'samples': SAMPLES,
        'build': report['build'], 'frames': report['frames'],
        'checkpoint': CHECKPOINT,
        'checkpoint_sha256': hashlib.sha256((ROOT / CHECKPOINT).read_bytes()).hexdigest(),
        'samples_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
        'trace_sha256': hashlib.sha256((ROOT / RUN / 'trace.txt').read_bytes()).hexdigest(),
        'writes_sha256': hashlib.sha256((ROOT / RUN / 'writes.jsonl').read_bytes()).hexdigest(),
    }
    target.with_suffix('.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(target.relative_to(ROOT))


if __name__ == '__main__':
    main()
