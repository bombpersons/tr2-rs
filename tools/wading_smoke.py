#!/usr/bin/env python3
"""Replay PSX wading routes in the full standalone game over developer MCP.

Run after `cargo build -p tomb-raider-ii --features dev-mcp`. Each route starts
from its first recorded row, replays the same pad input and checks complete-game
poses; the camera is aligned to the recorded PSX camera for side-by-side frames.
"""
import argparse
import csv
import json
import math
import shutil
from port_smoke import Client, ROOT, align_reference, convert
from record_wading import CASES
from swimming_smoke import multiply

MODES = {0: 'Idle', 1: 'Swimming', 2: 'Surface', 4: 'Wade'}


def binary(v):
    return (round(v * 65536 / math.tau) + 32768) % 65536 - 32768


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--case', choices=CASES, action='append')
    args = p.parse_args()
    out = ROOT / 'validation/wading/smoke'
    out.mkdir(parents=True, exist_ok=True)
    report = {'status': 'running', 'cases': {}}
    level = json.loads((ROOT / 'game/assets/levels/assault/level.json').read_text())
    try:
        for name in args.case or CASES:
            rows = list(csv.DictReader((ROOT / f'game/tests/data/original/wading_{name}.csv').open()))
            meta = json.loads((ROOT / f'game/tests/data/original/wading_{name}.json').read_text())
            first = rows[0]
            mode = int(first['water_mode'])
            client = Client(ROOT / 'target/debug/tomb-raider-ii', out / name, 'assault')
            poses = report['cases'][name] = []
            try:
                client.wait_loaded()
                ref = dict(level='assault', position=[int(first[k]) for k in 'xyz'], yaw=int(first['yaw']),
                           room=int(first['room']), animation=int(first['animation']),
                           absolute_animation_frame=int(first['animation_frame']),
                           camera=[int(first['camera_' + k]) for k in 'xyz'],
                           target=[int(first['target_' + k]) for k in 'xyz'])
                align_reference(client, ref)
                client.edit('Player', 'Player', {'mode': ('PullingUp' if int(first['state']) == 55 else 'Wade' if int(first['state']) == 65 else 'Idle') if mode == 4 else MODES[mode], 'health': int(first['health'])})
                client.edit('Player', 'Motion', {'grounded': mode in [0, 4], 'velocity': [0, 0, 0],
                                                 'pitch': int(first['pitch']) * math.tau / 65536, 'turn_rate': 0,
                                                 'turning': 0, 'movement_heading': int(first['yaw']) * math.tau / 65536,
                                                 'oxygen_seconds': int(first['air']) / 30})
                client.edit('Player', 'SubmergedMotion', {'speed': int(first['swim_speed']) * 30 / 4096,
                                                          'bank': 0.0, 'air_ticks': int(first['air']), 'dive_hold': 0})
                client.edit('Player', 'Wading', {'active': mode == 4})
                client.edit('Player', 'Traversal', {'goal': int(first['goal'])})
                last = len(rows) - 5
                fields = sorted({f for f in range(6, last + 1, 30)} | {last - last % 2})
                tick = 0
                for field in fields:
                    ticks = meta['logic_ticks']
                    while tick < len(ticks) and ticks[tick]['field'] < field:
                        client.call('game_step', {'frames': 1, 'buttons': [b.lower() for b in ticks[tick]['buttons']]})
                        tick += 1
                    row = rows[field]
                    state = client.query(components=['Player', 'Motion', 'SubmergedMotion', 'AnimationState', 'Traversal', 'Wading', 'Transform'])
                    c = state['entities'][0]['components']
                    actor, water, a = c['Player'], c['SubmergedMotion'], c['AnimationState']
                    clip = level['animations'][a['animation']]
                    actual = dict(x=round(actor['position'][0] * 1024), y=round(-actor['position'][1] * 1024),
                                  z=round(actor['position'][2] * 1024), yaw=binary(actor['yaw']),
                                  animation=a['animation'], animation_frame=clip['frame_start'] + a['frame'],
                                  state=clip['state'], room=actor['room'], health=actor['health'],
                                  goal=c['Traversal']['goal'], water_mode=4 if c['Wading']['active'] else 1 if actor['mode']=='Swimming' else 2 if actor['mode']=='Surface' else 0,
                                  pitch=binary(c['Motion']['pitch']), roll=binary(water['bank']),
                                  turn_rate=c['Motion']['turn_rate'], air=water['air_ticks'])
                    actual['swim_speed'] = round(water['speed'] * 4096 / 30)
                    for key, value in actual.items():
                        assert value == int(row[key]), (name, field, key, value, row[key])
                    yaw, bank = [int(row[k])*math.tau/65536 for k in ['yaw','roll']]
                    pitch = int(row['pitch'])*math.tau/65536 if int(row['water_mode']) in [1,2] else 0
                    expected = multiply(multiply([0,math.sin(yaw/2),0,math.cos(yaw/2)],
                                                 [-math.sin(pitch/2),0,0,math.cos(pitch/2)]),
                                        [0,0,-math.sin(bank/2),math.cos(bank/2)])
                    assert abs(abs(sum(x*y for x,y in zip(expected,c['Transform']['rotation'])))-1)<1e-5
                    client.edit('ChaseCamera', 'CameraRig', {'manual': True,
                                                             'position': convert([int(row['camera_' + k]) for k in 'xyz']),
                                                             'target': convert([int(row['target_' + k]) for k in 'xyz'])})
                    capture = client.capture(f'f{field:04}', min_colors=16)
                    entry = dict(field=field, actual=actual, image=capture)
                    original = ROOT / f'validation/wading/{name}/screens/f{field + 4:06}.png'
                    if original.exists():
                        target = client.out / f'f{field:04}-original.png'
                        shutil.copyfile(original, target)
                        entry['original'] = str(target.relative_to(ROOT))
                    poses.append(entry)
            finally:
                client.close()
        report['status'] = 'complete'
    finally:
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(dict(status=report['status'], checks=sum(map(len, report['cases'].values())))))


if __name__ == '__main__':
    main()
