#!/usr/bin/env python3
"""Check visible room pixels and camera-away/return mesh restoration over MCP.

Build first: cargo build -p tomb-raider-ii --all-features
This is a port rendering regression, not a claim of PSX portal-clip parity.
"""
import argparse
import json
import time
from pathlib import Path

from PIL import Image
from port_smoke import Client, ROOT


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/tomb-raider-ii')
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/room-visibility')
    parser.add_argument('--levels', nargs='+', choices=['wall', 'assault'], default=['wall', 'assault'])
    args = parser.parse_args()
    report = {}
    for level, room, eye, target, ticks in [
        ('wall', 36, [62.67, -4.4, 24], [62.67, -4.4, 30], 116),
        ('assault', 44, [19.5, -1.5, 91], [19.5, -1.5, 80], 1),
    ]:
        if level not in args.levels:
            continue
        client = Client(args.binary.resolve(), args.out.resolve() / level, level)
        case = report[level] = {'captures': [], 'rooms': []}
        try:
            client.wait_loaded()
            client.call('game_step', {'frames': ticks, 'buttons': []})
            away = [2 * a - b for a, b in zip(eye, target)]
            # Repeat: empty/nonempty transitions must keep the same material,
            # geometry and pixels, even while the simulation remains paused.
            for name, look in [('forward', target), ('away', away), ('return', target), ('away-again', away), ('return-again', target)]:
                client.edit('ChaseCamera', 'CameraRig', {'manual': True, 'position': eye, 'target': look})
                time.sleep(0.15)
                state = client.query(f'Room{room}', ['Visibility', 'RoomMeshStats'])
                case['rooms'].append(state)
                triangles = state['entities'][0]['components']['RoomMeshStats']['triangles']
                assert (triangles == 0) == name.startswith('away'), (name, state)
                capture = client.capture(name, min_colors=1 if name.startswith('away') else 8)
                case['captures'].append(capture)
                if not name.startswith('away'):
                    # This centre window showed black void through Wall's
                    # first portal with the broken mesh/material lifecycle.
                    image = Image.open(ROOT / capture['path']).convert('RGB')
                    pixels = list(image.crop((145, 90, 225, 145)).get_flattened_data())
                    lit = sum(max(pixel) >= 8 for pixel in pixels)
                    case.setdefault('lit_pixels', []).append(lit)
                    assert lit > len(pixels) * 0.6, (level, name, lit)
            captures = case['captures']
            assert captures[0]['sha256'] == captures[2]['sha256'] == captures[4]['sha256'], case
        finally:
            client.close()
            args.out.mkdir(parents=True, exist_ok=True)
            (args.out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': 'complete', 'report': str(args.out / 'report.json')}))


if __name__ == '__main__':
    main()
