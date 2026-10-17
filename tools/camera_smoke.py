#!/usr/bin/env python3
"""Render the authored Wall entrance shot through the standalone game's MCP."""
import json
import argparse
import time
from pathlib import Path

from port_smoke import Client, ROOT


def change_level(client, direction):
    previous = client.query()['entities'][0]['entity']
    for button in ['start', direction, 'down', 'cross']:
        client.call('game_step', {'frames': 1, 'buttons': [button]})
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        players = client.query()['entities']
        if len(players) == 1 and players[0]['entity'] != previous:
            roots = client.query('LevelWorld', ['Transform', 'GlobalTransform'])['entities']
            assert len(roots) == 1, roots
            assert roots[0]['components']['Transform']['scale'] == [1.0, 1.0, -1.0], roots
            return
        time.sleep(0.1)
    raise RuntimeError('Level did not replace its render hierarchy and player')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/camera-smoke')
    parser.add_argument('--transitions', action='store_true')
    args = parser.parse_args()
    out = args.out.resolve()
    client = Client(ROOT / 'target/debug/tomb-raider-ii', out, 'wall')
    report = {'cases': []}
    try:
        client.wait_loaded()
        for name, ticks in [('entrance', 1), ('fall', 15), ('cave', 100)]:
            client.call('game_step', {'frames': ticks, 'buttons': []})
            state = client.query(components=['Player', 'AnimationState'], resources=['CameraState'])
            camera = state['resources']['CameraState']
            if name != 'cave':
                assert camera['fixed_camera'] == 2, camera
                assert camera['position'] == [60.5, -0.375, 22.5], camera
                if name == 'entrance':
                    assert camera['target'] == [61.5, -776 / 1024, 22.5], camera
            else:
                assert camera['fixed_camera'] is None, camera
            report['cases'].append({'name': name, 'state': state, 'image': client.capture(name)})
        if args.transitions:
            # The wall's previous level is Home; Home's next level is Wall.
            for name, direction in [('home', 'left'), ('wall-return', 'right')]:
                change_level(client, direction)
                client.call('game_step', {'frames': 1, 'buttons': []})
                state = client.query(components=['Player', 'Transform', 'GlobalTransform'], resources=['CameraState'])
                if name == 'wall-return':
                    assert state['resources']['CameraState']['fixed_camera'] == 2, state
                report['cases'].append({'name': name, 'state': state, 'image': client.capture(name)})
        report['status'] = 'complete'
    finally:
        client.close()
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'report': str(out / 'report.json')}))


if __name__ == '__main__':
    main()
