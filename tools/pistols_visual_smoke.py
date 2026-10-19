#!/usr/bin/env python3
"""Compare drawn pistols with the original over MCP: draw, grip swap, aim, recoil.

Build first: `cargo build -p tomb-raider-ii --features dev-mcp`.

The port replays the draw_fire_holster route of tools/record_pistols.py
(Triangle at fields 10..14, Cross from field 70) from the Wall start, one
tick per two fields, and screenshots chosen ticks. The original's screens
come from a retained `psx run` of the same pad input from .psx/wall_game
(`--snapshot-every 1`, see ORIGINAL below); its screen at field f shows the
state sampled at field f - 4 (the muzzle flash counter is set at field 80
and the flash and gunfire light are on screen at fields 84..89). The game
never opens the disc; the comparison sheets are for looking at, not a
pixel-parity claim.
"""
import argparse
import csv
import json
from pathlib import Path

from PIL import Image
from port_smoke import Client, ROOT, REFERENCES, align_reference, convert

ORIGINAL = ROOT / '.psx/pistols_visual_ref'
# The retained route samples: the port's camera is placed where the
# original's was at the compared tick, so the views line up.
SAMPLES = ROOT / 'game/tests/data/original/pistols_draw_fire_holster.csv'
DISPLAY_LAG = 4
# name: port tick (original field = 2 * tick); expected left arm frame
CASES = {
    'draw_frame9': (10, 9),
    'draw_frame10': (11, 10),
    'draw_frame11': (12, 11),
    'draw_frame12': (13, 12),
    'grip_frame13': (14, 13),
    'draw_frame14': (15, 14),
    'draw_frame16': (17, 16),
    'ready_frame0': (30, 0),
    'aim_frame4': (39, 4),
    'recoil_frame24': (40, 24),
    'recoil_frame26': (42, 26),
}
DRAW_TICKS = range(5, 7)
FIRE_FROM = 35


def buttons(tick):
    held = []
    if tick in DRAW_TICKS:
        held.append('triangle')
    if tick >= FIRE_FROM:
        held.append('cross')
    return held


def sheet(port, original, out, box=None):
    a = Image.open(port).convert('RGB')
    b = Image.open(original).convert('RGB').resize(a.size, Image.NEAREST)
    if box:
        a, b = a.crop(box), b.crop(box)
        a = a.resize((a.width * 3, a.height * 3), Image.NEAREST)
        b = b.resize((b.width * 3, b.height * 3), Image.NEAREST)
    s = Image.new('RGB', (a.width * 2 + 4, a.height), (255, 0, 255))
    s.paste(a, (0, 0))
    s.paste(b, (a.width + 4, 0))
    s.save(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/pistols-visual')
    parser.add_argument('--box', type=int, nargs=4, default=[100, 90, 260, 240],
                        help='crop (port screen pixels) for the zoomed sheets')
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    report = {'cases': {}, 'status': 'running'}
    rows = list(csv.DictReader(SAMPLES.open()))
    client = Client(ROOT / 'target/debug/tomb-raider-ii', out / 'port', 'wall')
    try:
        client.wait_loaded()
        # Start where the recordings start: Lara settled at the .psx/wall_game
        # checkpoint (the level placement drops her from above).
        report['alignment'] = align_reference(client, REFERENCES['wall'])
        report['alignment']['mode'] = client.edit('Player', 'Player', {'mode': 'Idle'})
        report['alignment']['motion'] = client.edit('Player', 'Motion', {'grounded': True})
        client.edit('ChaseCamera', 'CameraRig', {'manual': False})
        wanted = {tick: name for name, (tick, _) in CASES.items()}
        for tick in range(1, max(wanted) + 1):
            client.call('game_step', {'frames': 1, 'buttons': buttons(tick - 1)})
            if tick not in wanted:
                continue
            name = wanted[tick]
            row = rows[2 * tick]
            camera = [int(row[f'camera_{a}']) for a in 'xyz']
            target = [int(row[f'target_{a}']) for a in 'xyz']
            client.edit('ChaseCamera', 'CameraRig', {'manual': True, 'position': convert(camera), 'target': convert(target)})
            state = client.query(components=['WeaponState', 'Player', 'AnimationState'])
            weapon = state['entities'][0]['components']['WeaponState']
            image = client.capture(name, min_colors=16)
            field = 2 * tick + DISPLAY_LAG
            original = ORIGINAL / f'screens/f{field:06d}.png'
            sheet(ROOT / image['path'], original, out / f'{name}_full.png')
            sheet(ROOT / image['path'], original, out / f'{name}_zoom.png', tuple(args.box))
            report['cases'][name] = {
                'tick': tick, 'original_screen': str(original.relative_to(ROOT)), 'image': image,
                'expected_left_frame': CASES[name][1], 'weapon': weapon, 'camera': camera, 'target': target,
                'player': state['entities'][0]['components'].get('Player'),
                'original_player': {k: row[k] for k in ['x', 'y', 'z', 'yaw', 'animation', 'animation_frame']},
            }
            assert weapon['left']['frame'] == CASES[name][1], (name, weapon['left'])
        log = (client.out / 'game.log').read_text()
        assert not any(s in log for s in ['panicked at', 'Shader compilation error', 'B0001', 'Failed to load asset'])
        report['status'] = 'complete'
    finally:
        client.close()
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'report': str(out / 'report.json')}))


if __name__ == '__main__':
    main()
