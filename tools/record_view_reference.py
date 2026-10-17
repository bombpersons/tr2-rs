#!/usr/bin/env python3
"""Build projection reference data from retained psx RAM snapshots.

The expected projections use the original's Q14 view matrix, independently of
Bevy's camera implementation. Baked room vertices supply world landmarks.
"""
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    cases = []
    for checkpoint, level in [('wall_spawn', 'wall'), ('wall_game', 'wall'),
                              ('home_start', 'assault'), ('home_side', 'assault')]:
        raw = (ROOT / '.psx' / checkpoint / 'ram.bin').read_bytes()
        matrix = struct.unpack_from('<12i', raw, 0x8aa60)
        eye = struct.unpack_from('<3i', raw, 0x8b258)
        target = struct.unpack_from('<3i', raw, 0x8b268)
        data = json.loads((ROOT / f'game/assets/levels/{level}/level.json').read_text())
        points = {}
        for room in data['rooms']:
            for vertex in room['vertices']:
                point = [vertex[0] + room['origin'][0], vertex[1], vertex[2] + room['origin'][2]]
                delta = [point[i] - eye[i] for i in range(3)]
                view = [sum(matrix[row * 4 + i] * delta[i] for i in range(3)) / 16384 for row in range(3)]
                if not 512 < view[2] < 10000:
                    continue
                screen = [192 + 228 * view[0] / view[2], 120 + 228 * view[1] / view[2]]
                if 8 < screen[0] < 360 and 8 < screen[1] < 232:
                    points[tuple(point)] = {'room': room['id'], 'world': point, 'screen': screen}
        ordered = sorted(points.values(), key=lambda point: point['screen'])
        step = max(1, len(ordered) // 16)
        cases.append({'checkpoint': checkpoint, 'ram_sha256': hashlib.sha256(raw).hexdigest(),
                      'eye': eye, 'target': target,
                      'matrix_rows': [matrix[i:i+3] for i in range(0, 12, 4)],
                      'landmarks': ordered[::step][:16]})
    output = {'matrix_address': '0x8008aa60', 'matrix_fractional_bits': 14,
              'projection_distance': 228, 'center': [192, 120], 'cases': cases}
    (ROOT / 'game/tests/data/original/view_projection.json').write_text(json.dumps(output, indent=2) + '\n')


if __name__ == '__main__':
    main()
