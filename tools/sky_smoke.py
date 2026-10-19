#!/usr/bin/env python3
"""Check the standalone sky against retained PSX views and background contracts.

Build the dev-mcp game first. Original captures use psx only; the independent
turn view is home_start + Right[0,36),120 fields, retained in .psx/home_sky_turn.
"""
import json
import struct
from pathlib import Path
from PIL import Image, ImageChops
from port_smoke import Client, ROOT, align_reference, convert


def reference(name):
    source = ROOT / '.psx' / name
    ram = (source / 'ram.bin').read_bytes()
    def values(address, fmt):
        return list(struct.unpack_from('<' + fmt, ram, address))
    clip = values(0x17c890, 'h')[0]
    return dict(level='assault', position=values(0x17c8b0, 'iii'), yaw=values(0x17c8be, 'h')[0],
                animation=clip, absolute_animation_frame=values(0x17c892, 'h')[0], room=values(0x17c894, 'h')[0],
                camera=values(0x8b258, 'iii'), target=values(0x8b268, 'iii'),
                screenshot=str(source / 'screen.png'))


def main():
    out = ROOT / 'validation/home-sky/smoke'
    client = Client(ROOT / 'target/debug/tomb-raider-ii', out, 'assault')
    report = dict(status='running', views={})
    try:
        client.wait_loaded()
        for name in ['home_start', 'home_side', 'home_sky_turn']:
            ref = reference(name)
            align_reference(client, ref)
            client.edit('Sky', 'Sky', {'enabled': False})
            off = client.capture(name + '-off')
            client.edit('Sky', 'Sky', {'enabled': True})
            on = client.capture(name + '-on')
            a = Image.open(ROOT / off['path']).convert('RGB')
            b = Image.open(ROOT / on['path']).convert('RGB')
            original = Image.open(ref['screenshot']).convert('RGB')
            original.save(out / (name + '-original.png'))
            changed = [(i, p, q) for i, (p, q) in enumerate(zip(a.get_flattened_data(), b.get_flattened_data())) if p != q]
            # A background pass can only change the previously empty pixels.
            if name == 'home_side':
                assert len(changed) < 32, (name, len(changed))
            else:
                assert len(changed) > 300, (name, len(changed))
            assert all(p == (0, 0, 0) for _, p, _ in changed), name
            # Compare an unoccluded upper patch, outside the HUD, against PSX.
            patch = [(i, q) for i, _, q in changed if i // a.width < 36 and i % a.width < 34]
            error = None
            if name != 'home_side':
                assert len(patch) > 100, name
                reference_pixels = list(original.get_flattened_data())
                error = sum(sum(abs(q[c] - reference_pixels[i][c]) for c in range(3)) for i, q in patch) / (3 * len(patch))
                assert error < 20, (name, error)
            report['views'][name] = dict(reference=ref, off=off, on=on,
                                       changed_pixels=len(changed), upper_sky_mean_absolute_error=error)
        # Collapse the level hierarchy to isolate the background. Move the
        # camera and target together: the complete background must be identical.
        client.edit('LevelWorld', 'Transform', {'scale': [0.0, 0.0, 0.0]})
        ref = reference('home_sky_turn')
        align_reference(client, ref)
        before = client.capture('sky-only')
        offset = [100000, -50000, 100000]
        client.edit('ChaseCamera', 'CameraRig', {
            'position': convert([p + d for p, d in zip(ref['camera'], offset)]),
            'target': convert([p + d for p, d in zip(ref['target'], offset)]),
        })
        after = client.capture('sky-only-translated')
        assert ImageChops.difference(Image.open(ROOT / before['path']).convert('RGB'), Image.open(ROOT / after['path']).convert('RGB')).getbbox() is None
        report['translation_invariant'] = True
        report['status'] = 'complete'
    finally:
        client.close()
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
