#!/usr/bin/env python3
"""Check standalone item lighting in aligned Home and Wall views over MCP.

Build the game's dev-mcp binary first. Native reference screens are retained
under .psx/home_start and .psx/wall_game; the game never opens the disc.
"""
import argparse
import json
import statistics
from pathlib import Path

from PIL import Image
from port_smoke import Client, ROOT, REFERENCES, align_reference


def brightness(path, box):
    with Image.open(path) as image:
        rgb = image.convert('RGB').crop(box)
        return statistics.median(sum(pixel) / 3 for pixel in rgb.get_flattened_data())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/item-lighting')
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    report = {'cases': {}, 'status': 'running'}
    try:
        for case in ['home', 'wall']:
            reference = REFERENCES[case]
            client = Client(ROOT / 'target/debug/tomb-raider-ii', out / case, reference['level'])
            try:
                client.wait_loaded()
                align_reference(client, reference)
                image = client.capture('aligned', min_colors=16)
                record = report['cases'][case] = {'image': image, 'reference': reference,
                                                'state': client.query(components=['Player', 'AnimationState'])}
                if case == 'home':
                    # A skin patch on Lara's back in this fixed reference view.
                    # Before the fix its median was 5.3 vs native 127.7.
                    box = [181, 110, 195, 126]
                    port = brightness(ROOT / image['path'], box)
                    original = brightness(ROOT / reference['screenshot'], box)
                    record['skin_patch'] = {'box': box, 'port_median': port, 'original_median': original}
                    # Broad visual regression bound, not a pixel-parity claim:
                    # disallow the silhouette while tolerating pose/raster differences.
                    assert 0.5 * original <= port <= 1.75 * original, record
                log = (client.out / 'game.log').read_text()
                assert not any(s in log for s in ['panicked at', 'Shader compilation error', 'B0001', 'Failed to load asset'])
            finally:
                client.close()
        report['status'] = 'complete'
    finally:
        (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'report': str(out / 'report.json')}))


if __name__ == '__main__':
    main()
