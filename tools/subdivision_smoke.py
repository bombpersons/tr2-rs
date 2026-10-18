#!/usr/bin/env python3
"""Compare room subdivision switches on paused, aligned PSX reference views.

Build first: cargo build -p tomb-raider-ii --all-features
Only retained psx RAM/captures supply the reference cameras. Outputs and the
exact MCP transcript are kept under validation/subdivision/.
"""
import json
import struct
import time
from pathlib import Path
from port_smoke import Client, ROOT, REFERENCES, align_reference, convert


def main():
    out = ROOT / 'validation/subdivision'
    client = Client(ROOT / 'target/debug/tomb-raider-ii', out, 'wall')
    report = {}
    try:
        client.wait_loaded()
        align_reference(client, REFERENCES['wall'])
        for case in ['near', 'far']:
            raw = (ROOT / f'.psx/affine_gpu_{case}/ram.bin').read_bytes()
            eye = struct.unpack_from('<3i', raw, 0x8b258)
            target = struct.unpack_from('<3i', raw, 0x8b268)
            client.edit('ChaseCamera', 'CameraRig', {'manual': True, 'position': convert(eye), 'target': convert(target)})
            result = report[case] = {'eye': eye, 'target': target, 'captures': {}}
            for mode, enabled, fillers in [('off', False, False), ('split', True, False), ('filled', True, True)]:
                client.edit('RoomSubdivision', 'RoomSubdivision', {'enabled': enabled, 'fillers': fillers})
                time.sleep(0.15)
                result['captures'][mode] = client.capture(f'{case}-{mode}', min_colors=16)
                result[mode] = client.query(name='Room35', components=['RoomMeshStats', 'Transform'])
            for capture in result['captures'].values():
                assert (capture['width'], capture['height']) == (368, 240)
            counts = [result[mode]['entities'][0]['components']['RoomMeshStats'] for mode in ['off', 'split', 'filled']]
            assert counts[1]['triangles'] != counts[0]['triangles'], counts
            assert counts[1]['underlay_triangles'] == 0 and counts[2]['underlay_triangles'] > 0, counts
        logs = (out / 'game.log').read_text()
        assert not any(s in logs for s in ['panicked at', 'Shader compilation error', 'B0001', 'Failed to load asset']), logs
    finally:
        client.close()
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({case: result['captures'] for case, result in report.items()}, indent=2))


if __name__ == '__main__':
    main()
