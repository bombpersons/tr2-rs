#!/usr/bin/env python3
"""Exercise animated room shading, dry-room isolation and paused restoration."""
import json
from PIL import Image
from port_smoke import Client, ROOT, REFERENCES, align_reference, convert


def pixels(path):
    return list(Image.open(path).convert('RGB').get_flattened_data())


def main():
    out = ROOT/'validation/caustics/smoke'
    client = Client(ROOT/'target/debug/tomb-raider-ii', out, 'assault')
    report = {'status': 'running'}
    try:
        client.wait_loaded()
        client.call('game_step', {'frames': 1, 'buttons': []})
        align_reference(client, REFERENCES['home'])
        client.edit('Caustics', 'Caustics', {'phase': 2, 'enabled': True})
        dry = client.capture('dry-enabled')
        client.edit('Caustics', 'Caustics', {'enabled': False})
        dry_off = client.capture('dry-disabled')
        assert pixels(ROOT/dry['path']) == pixels(ROOT/dry_off['path'])
        fixture = json.loads((ROOT/'game/tests/data/original/underwater_palette_view.json').read_text())
        row = fixture['sample']
        ref = dict(level='assault', position=[row[k] for k in 'xyz'], yaw=row['yaw'], room=row['room'],
                   animation=row['animation'], absolute_animation_frame=row['animation_frame'],
                   camera=[row['camera_'+k] for k in 'xyz'], target=[row['target_'+k] for k in 'xyz'])
        align_reference(client, ref)
        client.edit('Player', 'Avatar', {'position': convert(ref['position']), 'room': ref['room']})
        baseline = client.capture('wet-disabled')
        client.edit('Caustics', 'Caustics', {'enabled': True, 'phase': 36})
        first = client.capture('wet-phase36')
        client.edit('Caustics', 'Caustics', {'phase': 52})
        second = client.capture('wet-phase52')
        a, b, c = [pixels(ROOT/item['path']) for item in [baseline, first, second]]
        changed = sum(x != y for x, y in zip(a, b))
        animated = sum(x != y for x, y in zip(b, c))
        assert changed > 20000 and animated > 20000, (changed, animated)
        hud = [y*368+x for y in range(12, 16) for x in range(39, 139)]
        assert all(a[i] == b[i] == c[i] for i in hud)
        # Shade changes must not alter topology, UVs or the projected silhouette.
        before = client.query(name='Room14', components=['RoomMeshStats'])
        client.edit('Caustics', 'Caustics', {'phase': 36})
        restored = client.capture('wet-phase36-restored')
        assert pixels(ROOT/restored['path']) == b
        after = client.query(name='Room14', components=['RoomMeshStats'])
        for key in ['triangles', 'underlay_triangles']:
            assert before['entities'][0]['components']['RoomMeshStats'][key] == after['entities'][0]['components']['RoomMeshStats'][key]
        client.edit('Caustics', 'Caustics', {'enabled': False})
        disabled = client.capture('wet-disabled-restored')
        assert pixels(ROOT/disabled['path']) == a
        bounds = (12, 70, 115, 170)
        means = {}
        for key, path in [('baseline', ROOT/baseline['path']), ('phase36', ROOT/first['path']), ('native', ROOT/fixture['screenshot'])]:
            region = list(Image.open(path).convert('RGB').crop(bounds).get_flattened_data())
            means[key] = [sum(pixel[channel] for pixel in region)/len(region) for channel in range(3)]
        report.update(status='complete', dry_unchanged=True, changed_pixels=changed, animated_pixels=animated,
                      restoration_exact=True, health_bar_unchanged=True, topology_unchanged=True,
                      crop=bounds, mean_rgb=means, screenshots=[dry, dry_off, baseline, first, second, restored, disabled])
    finally:
        client.close()
        out.mkdir(parents=True, exist_ok=True)
        (out/'report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
