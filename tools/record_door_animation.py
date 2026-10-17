#!/usr/bin/env python3
"""PSX-only records of Wall item 72 (type 107), at 0x8019DEA0.

Poke its activation code, not its animation. The unmodified control stays open.
The held-out run requests reopening while a close is still in progress.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
BASE = 0x8019DEA0
SAMPLES = [(0x0c, 'i16', 'kind'), (0x0e, 'i16', 'state'), (0x10, 'i16', 'goal'),
           (0x14, 'i16', 'animation'), (0x16, 'i16', 'animation_frame'), (0x28, 'x16', 'flags')]
CASES = {'control': (12, []), 'cycle': (300, ['8019dec8=0x7e00:u16@20', '8019dec8=0x4000:u16@160']),
         'heldout': (300, ['8019dec8=0x7e00:u16@0', '8019dec8=0x4000:u16@24'])}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--case', choices=CASES, default='cycle')
    parser.add_argument('--summarize-only', action='store_true')
    args = parser.parse_args()
    frames, pokes = CASES[args.case]
    run = ROOT / f'.psx/door107_{args.case}'
    command = ['psx', 'run', '--from-state', '.psx/wall_game/state.bin', '--frames', str(frames), '--out', str(run.relative_to(ROOT)), '--summary']
    for offset, fmt, name in SAMPLES: command += ['--sample', f'{BASE+offset:08x}:{fmt}:{name}']
    for poke in pokes: command += ['--poke', poke]
    if not args.summarize_only: subprocess.run(command, cwd=ROOT, check=True)
    report = json.loads((run/'report.json').read_text())
    assert report['status'] == 'complete' and report['frames']['reached'] == frames
    csv = ROOT/f'game/tests/data/original/door_{args.case}.csv'
    shutil.copyfile(run/'samples.csv', csv)
    metadata = {'source': 'Supplied USA Rev 3 disc executed with psx native; animation/state fields never poked',
                'item': 72, 'kind': 107, 'item_address': f'0x{BASE:08X}', 'samples': SAMPLES,
                'command': command, 'pokes': pokes, 'build': report['build'], 'frames': report['frames'],
                'samples_sha256': hashlib.sha256(csv.read_bytes()).hexdigest(),
                'checkpoint_sha256': hashlib.sha256((ROOT/'.psx/wall_game/state.bin').read_bytes()).hexdigest(),
                'heldout_after_implementation': args.case == 'heldout'}
    csv.with_suffix('.json').write_text(json.dumps(metadata, indent=2)+'\n')
    print(csv.relative_to(ROOT))

if __name__ == '__main__': main()
