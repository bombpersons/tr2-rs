#!/usr/bin/env python3
"""Exercise the real editor over the template MCP, with a disposable sandbox copy.

Build first: cargo build -p editor --all-features
Optional real content: --level game/assets/levels/wall/level.json
The original file is never edited. Evidence is kept in validation/editor-smoke.
"""
import argparse
import json
import shutil
import time
from pathlib import Path
from port_smoke import Client, ROOT


def status(client):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        result = client.query(name='Editor', components=['EditorStatus'])
        state = next(e for e in result['entities'] if e['name'] == 'Editor')['components']['EditorStatus']
        if state['file']:
            return state
        time.sleep(0.05)
    raise RuntimeError('Editor did not open its document')


def command(client, **fields):
    client.edit('Editor', 'EditorCommand', fields)
    time.sleep(0.15)
    return status(client)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--level', type=Path, default=ROOT / 'engine/assets/levels/sandbox/level.json')
    args = parser.parse_args()
    source = args.level.resolve()
    out = ROOT / 'validation/editor-smoke' / source.parent.name
    out.mkdir(parents=True, exist_ok=True)
    copy = out / 'level.json'
    original = source.read_bytes()
    copy.write_bytes(original)
    shutil.copyfile(source.parent / 'atlas.png', out / 'atlas.png')
    client = Client(ROOT / 'target/debug/editor', out, str(copy))
    captures = []
    try:
        state = status(client)
        assert not state['dirty'], state
        command(client, save=True)
        assert copy.read_bytes() == original, 'untouched save changed bytes'
        captures.append(client.capture('opened'))
        before = json.loads(original)
        x = before['placements'][1]['position'][0]
        state = command(client, pointer='/placements/1/position/0', json=str(x + 512), apply=True)
        assert state['dirty'] and state['message'] == 'Applied edit', state
        state = command(client, undo=True)
        assert not state['dirty'], state
        state = command(client, save=True)
        assert copy.read_bytes() == original, 'undo/save changed bytes'
        state = command(client, redo=True)
        assert state['dirty'], state
        state = command(client, pointer='/placements/1/room', json='999999', apply=True)
        assert 'out of range' in state['message'], state
        state = command(client, save=True)
        assert not state['dirty'], state
        saved = json.loads(copy.read_bytes())
        before['placements'][1]['position'][0] += 512
        assert saved == before, 'save lost unrelated data'
        state = command(client, pointer='/placements/1', navigate=True, frame=True)
        captures.append(client.capture('edited'))
    finally:
        client.close()
    # A new process proves that the saved engine file, rather than transient
    # in-memory state, is sufficient to open and show the edited content.
    client = Client(ROOT / 'target/debug/editor', out / 'reopened', str(copy))
    try:
        state = status(client)
        assert not state['dirty'], state
        captures.append(client.capture('reopened'))
    finally:
        client.close()
    assert source.read_bytes() == original
    from PIL import Image
    for capture in captures:
        with Image.open(ROOT / capture['path']) as image:
            assert image.size == (1280, 800), 'editor must render at full resolution'
            preview = image.convert('RGB').crop((500, 80, 1280, 800))
            assert len(preview.getcolors(preview.width * preview.height)) > 32, '3D preview is blank or a fallback material'
    report = {'level': str(source.relative_to(ROOT)), 'captures': captures, 'save_roundtrip': True}
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
