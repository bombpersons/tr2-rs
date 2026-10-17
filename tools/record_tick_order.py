#!/usr/bin/env python3
"""Record and summarize the supplied disc's tick ordering with psx only.

The first run is a baseline; no input or memory pokes. --summarize-only
processes the already captured trace. The JSON retains caller addresses and
absolute fields; tests project only the phases the port currently implements.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUN = '.psx/tick_order_port'
CALL = {
    0x1dc54: 'items', 0x1dcd0: 'active_effects', 0x1dce8: 'player_control',
    0x383e0: 'player_animation', 0x38400: 'object_collision',
    0x38438: 'state_collision', 0x38444: 'room_update', 0x3844c: 'weapons',
    0x38458: 'triggers', 0x1dcf0: 'secondary_motion', 0x1dcf8: 'camera', 0x1dd00: 'sound',
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--summarize-only', action='store_true')
    parser.add_argument('--heldout', action='store_true')
    args = parser.parse_args()
    run = '.psx/tick_order_heldout' if args.heldout else RUN
    checkpoint = '.psx/home_side/state.bin' if args.heldout else '.psx/wall_game/state.bin'
    frames = '6' if args.heldout else '4'
    command = ['psx', 'run', '--from-state', checkpoint, '--frames', frames,
               '--trace', '--trace-range', '0x8001DC00-0x8001DD08',
               '--trace-range', '0x80038194-0x800384D0', '--max-events', '100000', '--out', run, '--summary']
    if args.heldout:
        command += ['--replay', 'game/tests/data/original/tick_order_turn.input.json']
    if not args.summarize_only:
        subprocess.run(command, cwd=ROOT, check=True)
    trace = ROOT / run / 'trace.txt'
    ticks = {}
    for line in trace.read_text().splitlines():
        parts = line.split()
        if len(parts) < 5 or not parts[0].isdigit():
            continue
        caller = int(parts[2], 16)
        if caller in CALL and parts[4] in ('jal', 'jalr'):
            ticks.setdefault(int(parts[0]), []).append({'caller': f'0x{caller | 0x80000000:08X}', 'phase': CALL[caller]})
    report = json.loads((ROOT / run / 'report.json').read_text())
    assert report['status'] == 'complete' and report['trace']['lines_dropped'] == 0
    output = {
        'source': 'Supplied Tomb Raider II USA Rev 3 disc via psx native; no pokes',
        'command': command,
        'checkpoint_sha256': hashlib.sha256((ROOT / checkpoint).read_bytes()).hexdigest(),
        'trace_sha256': hashlib.sha256(trace.read_bytes()).hexdigest(),
        'frames': report['frames'],
        'input': report['input']['events'],
        'build': report['build'],
        'recorded_after_schedule_change': args.heldout,
        'ticks': [{'field': field, 'calls': calls} for field, calls in ticks.items()],
    }
    path = ROOT / 'game/tests/data/original' / ('tick_order_heldout.json' if args.heldout else 'tick_order.json')
    path.write_text(json.dumps(output, indent=2) + '\n')
    print(f'{path.relative_to(ROOT)}: {len(ticks)} recorded ticks')


if __name__ == '__main__':
    main()
