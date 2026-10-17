#!/usr/bin/env python3
"""Render the climb_two_click route in the port for side-by-side comparison.

Replays the original's input (RIGHT fields [0,56), UP+CROSS from 60) from the
settled Wall start and captures the screen after the ticks matching the
original fields in FIELDS (field = 1 + 2 * ticks). Original frames come from
`.psx/climb_visual/screens` (psx run --snapshot-every 1 from wall_game).
The original screens lag game state by ~4 fields: compare port field N with
original screen N+4 (docs/braid-evidence.md).
Run after `cargo build -p tomb-raider-ii --features dev-mcp`.
"""
import argparse
import json
from pathlib import Path

from port_smoke import Client, ROOT

FIELDS = [41, 61, 79, 107, 135, 151, 189, 219]
START = [64176, 5376, 26090]


def buttons(tick):
    field = 2 * tick
    return ['right'] if field < 56 else ['up', 'cross'] if field >= 60 else []


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/climb-smoke')
    args = parser.parse_args()
    client = Client(ROOT / 'target/debug/tomb-raider-ii', args.out.resolve(), 'wall')
    report = {'cases': []}
    try:
        client.wait_loaded()
        # Let the authored entry fall and slides finish, as the original did.
        for _ in range(40):
            client.call('game_step', {'frames': 10, 'buttons': []})
            p = client.query(components=['Player'])['entities'][0]['components']['Player']
            if p['mode'] == 'Idle':
                break
        client.call('game_step', {'frames': 60, 'buttons': []})
        p = client.query(components=['Player'])['entities'][0]['components']['Player']
        report['settled'] = p
        position = [round(p['position'][0] * 1024), round(-p['position'][1] * 1024), round(p['position'][2] * 1024)]
        assert position == START, f'port settled at {position}, original {START}'
        tick = 0
        for field in FIELDS:
            target = (field - 1) // 2
            while tick < target:
                client.call('game_step', {'frames': 1, 'buttons': buttons(tick)})
                tick += 1
            state = client.query(components=['Player', 'AnimationState'], resources=['CameraState'])
            report['cases'].append({'field': field, 'state': state, 'image': client.capture(f'f{field:03}', min_colors=16)})
        report['status'] = 'complete'
    finally:
        client.close()
        (args.out / 'report.json').write_text(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
