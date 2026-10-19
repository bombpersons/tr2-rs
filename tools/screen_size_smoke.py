#!/usr/bin/env python3
"""Offscreen captures of the port at several screen sizes (never starts cargo).

Run after `cargo build -p tomb-raider-ii --features dev-mcp`. Each size starts
the game with a display settings file (the display menu's `display.json`),
aligns the Wall view to the original's checkpoint camera (as port_smoke.py
does), and captures the level view, the pause menu and the title screen.
Artifacts go under validation/screen-size/<size>/.

`--compare DIR` checks the native-size captures are byte-identical to an
earlier run (for example one made before a change).
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import time

from port_smoke import REFERENCES, ROOT, Client, align_reference

# The original's 368x240, the display menu's 16:9 at the same height, and 2x.
SIZES = [(368, 240), (426, 240), (736, 480)]


def run(args):
    report = {'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), 'sizes': {}}
    for width, height in [tuple(map(int, s.split('x'))) for s in args.sizes.split(',')]:
        out = (args.out / f'{width}x{height}').resolve()
        out.mkdir(parents=True, exist_ok=True)
        settings = out / 'display.json'
        settings.write_text(json.dumps({'version': 1, 'display': {'screen': [width, height], 'aspect': 'fixed', 'fullscreen': False,
                                        'look': {'vertex_snap': True, 'dither': True, 'quantize': True}}}))
        evidence = report['sizes'][f'{width}x{height}'] = {}
        extra = ['--display-settings', str(settings)]
        client = Client(args.binary.resolve(), out / 'wall', 'wall', extra_args=extra)
        try:
            client.wait_loaded()
            evidence['status'] = client.call('game_status').get('structuredContent')
            align_reference(client, REFERENCES['wall'])
            evidence['aligned'] = client.capture('aligned-original')
            client.call('game_step', {'frames': 1, 'buttons': ['start']})
            evidence['pause_menu'] = client.capture('pause-menu')
        finally:
            client.close()
        client = Client(args.binary.resolve(), out / 'title', None, extra_args=extra)
        try:
            time.sleep(2)
            client.call('game_step', {'frames': 2})
            evidence['title'] = client.capture('title', min_colors=16)
        finally:
            client.close()
        for case in ['wall', 'title']:
            log = (out / case / 'game.log').read_text()
            bad = [l for l in log.splitlines() if any(w in l for w in ['Shader compilation error', 'panicked at', 'Failed to load asset', 'Display settings were preserved'])]
            if bad:
                raise RuntimeError(f'{width}x{height} {case}: {bad[:3]}')
    if args.compare:
        native = report['sizes'].get('368x240')
        before = json.loads((args.compare / 'report.json').read_text())['sizes']['368x240']
        report['compare'] = {k: native[k]['sha256'] == before[k]['sha256'] for k in ['aligned', 'pause_menu', 'title']}
    (args.out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: {c: (v[c]['width'], v[c]['height']) for c in ['aligned', 'pause_menu', 'title']} for k, v in report['sizes'].items()}))
    if args.compare:
        print('native identical to', args.compare, report['compare'])
        if not all(report['compare'].values()):
            raise SystemExit(1)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/tomb-raider-ii')
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/screen-size')
    parser.add_argument('--sizes', default=','.join(f'{w}x{h}' for w, h in SIZES))
    parser.add_argument('--compare', type=Path)
    run(parser.parse_args())
