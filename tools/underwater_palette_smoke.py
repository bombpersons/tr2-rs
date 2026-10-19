#!/usr/bin/env python3
"""Render fresh PSX wet/dry views; check palette isolation and restoration."""
import json
import math
from pathlib import Path
from PIL import Image
from port_smoke import Client, ROOT, REFERENCES, align_reference, convert
from swimming_smoke import multiply


def pixels(path):
    return list(Image.open(path).convert('RGB').get_flattened_data())


def main():
    out = ROOT/'validation/underwater-color/smoke'
    client = Client(ROOT/'target/debug/tomb-raider-ii', out, 'assault')
    report = {'status': 'running'}
    try:
        client.wait_loaded()
        client.call('game_step', {'frames': 1, 'buttons': []})
        align_reference(client, REFERENCES['home'])
        first = client.capture('dry-enabled')
        client.edit('WaterPalette', 'WaterPalette', {'enabled': False})
        second = client.capture('dry-disabled')
        assert pixels(ROOT/first['path']) == pixels(ROOT/second['path'])
        report['dry_unchanged'] = True
        fixture = json.loads((ROOT/'game/tests/data/original/underwater_palette_view.json').read_text())
        row = fixture['sample']
        ref = dict(level='assault', position=[row[k] for k in 'xyz'], yaw=row['yaw'], room=row['room'],
                   animation=row['animation'], absolute_animation_frame=row['animation_frame'],
                   camera=[row['camera_'+k] for k in 'xyz'], target=[row['target_'+k] for k in 'xyz'])
        align_reference(client, ref)
        client.edit('Player', 'Avatar', {'position': convert(ref['position']), 'room': ref['room'], 'yaw': row['yaw']*math.tau/65536})
        client.edit('Player', 'Player', {'mode': 'Swimming'})
        yaw, pitch, bank = [row[k]*math.tau/65536 for k in ['yaw', 'pitch', 'roll']]
        rotation = multiply(multiply([0, math.sin(yaw/2), 0, math.cos(yaw/2)],
                                     [-math.sin(pitch/2), 0, 0, math.cos(pitch/2)]),
                            [0, 0, -math.sin(bank/2), math.cos(bank/2)])
        client.edit('Player', 'Transform', {'rotation': rotation})
        normal = client.capture('wet-disabled')
        client.edit('WaterPalette', 'WaterPalette', {'enabled': True})
        water = client.capture('wet-enabled')
        a, b = pixels(ROOT/normal['path']), pixels(ROOT/water['path'])
        assert all(x[1:] == y[1:] for x, y in zip(a, b)), 'Palette affected green/blue'
        assert all(x[0] >= y[0] for x, y in zip(a, b)), 'Palette increased red'
        hud = [y*368+x for y in range(12, 16) for x in range(39, 139)]
        assert all(a[i] == b[i] for i in hud), 'Palette affected health bar'
        changed = sum(x != y for x, y in zip(a, b))
        assert changed > 20000, changed
        native = Image.open(ROOT/fixture['screenshot']).convert('RGB')
        bounds = (12, 70, 115, 170)
        means = {}
        for key, image in [('normal', Image.open(ROOT/normal['path'])), ('water', Image.open(ROOT/water['path'])), ('native', native)]:
            region = list(image.convert('RGB').crop(bounds).get_flattened_data())
            means[key] = [sum(p[c] for p in region)/len(region) for c in range(3)]
        assert abs(means['water'][0]-means['native'][0]) < abs(means['normal'][0]-means['native'][0])/4, means
        client.edit('WaterPalette', 'WaterPalette', {'enabled': False})
        restored = client.capture('wet-restored')
        assert pixels(ROOT/restored['path']) == a
        report.update(status='complete', changed_pixels=changed, green_blue_unchanged=True,
                      hud_unchanged=True, restoration_exact=True, crop=bounds, mean_rgb=means,
                      native=fixture['screenshot'], screenshots=[first, second, normal, water, restored])
    finally:
        client.close()
        out.mkdir(parents=True, exist_ok=True)
        (out/'report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
