#!/usr/bin/env python3
"""Retain a matched room face and its actual PSX subdivision leaf packets.

Evidence is exclusively the retained psx run and decoded disc tables. The
native packet's corners independently corroborate the baked room vertices;
their earlier importer provenance is unchanged. No renderer code is executed.
"""
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    run = ROOT / '.psx/affine_gpu_near'
    gpu = (run / 'gpu_frame.json').read_bytes()
    ram = (run / 'ram.bin').read_bytes()
    capture = json.loads(gpu)
    parent = next(c for c in capture['commands'] if c['index'] == 124)
    level = json.loads((ROOT / 'game/assets/imported/levels/wall/level.json').read_text())
    raw = json.loads((ROOT / 'research/extracted/levels/wall/level.json').read_text())
    room = level['rooms'][35]
    face = room['quads'][60]
    vertices = [room['vertices'][i] for i in face[:4]]
    uv = [raw['textures'][face[4]]['uv'][i] for i in [0, 1, 3, 2]]
    eye = struct.unpack_from('<3i', ram, 0x8b258)
    target = struct.unpack_from('<3i', ram, 0x8b268)
    matrix = struct.unpack_from('<12i', ram, 0x8aa60)
    for vertex, packet in zip([vertices[i] for i in [0, 1, 3, 2]], parent['vertices']):
        world = [vertex[k] + room['origin'][k] for k in range(3)]
        view = [sum(matrix[i*4+k] * (world[k]-eye[k]) for k in range(3))/16384 for i in range(3)]
        assert max(abs(view[k]-packet['view'][k]) for k in range(3)) < 8, (view, packet)
        assert vertex[3] & 255 == packet['color'][0]
    # Match actual packets to this plane by UV knots and view coordinates.
    # The quarter-grid UV values are observed in the capture, not generated
    # by the port. A <=2-unit view tolerance accounts for GTE rounding.
    uk = [64, 79, 95, 111, 127]
    vk = [128, 143, 159, 175, 191]
    p = [v['view'] for v in parent['vertices']]
    leaves = []
    for command in capture['commands']:
        if command.get('kind') != 'poly4' or command.get('texpage') != parent['texpage'] or command.get('clut') != parent['clut']:
            continue
        corners = command['vertices']
        if any('view' not in v or v['uv'][0] not in uk or v['uv'][1] not in vk for v in corners):
            continue
        if any(max(v['uv'][k] for v in corners)-min(v['uv'][k] for v in corners) > 16 for k in range(2)):
            continue
        error = 0
        for v in corners:
            u, t = uk.index(v['uv'][0])/4, vk.index(v['uv'][1])/4
            expected = [p[0][k]+(p[1][k]-p[0][k])*u+(p[2][k]-p[0][k])*t for k in range(3)]
            error = max(error, max(abs(expected[k]-v['view'][k]) for k in range(3)))
        if error <= 2:
            leaves.append({'command': command['index'], 'vertices': corners})
    assert len(leaves) == 14
    result = {'run': '.psx/affine_gpu_near', 'gpu_sha256': hashlib.sha256(gpu).hexdigest(),
              'ram_sha256': hashlib.sha256(ram).hexdigest(), 'parent_command': 124,
              'room': 35, 'quad': 60, 'origin': room['origin'], 'vertices': vertices,
              'uv': uv, 'eye': eye, 'target': target, 'leaves': leaves}
    (ROOT / 'game/tests/data/original/subdivision.json').write_text(json.dumps(result, indent=2)+'\n')
    print(f"Retained {len(leaves)} native leaf packets from room 35 quad 60")


if __name__ == '__main__':
    main()
