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


def settled(client, before, **fields):
    """Send a command and wait for its result: a new revision or message."""
    client.edit('Editor', 'EditorCommand', fields)
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        state = status(client)
        if state['revision'] != before['revision'] or state['message'] != before['message']:
            return state
        time.sleep(0.1)
    raise RuntimeError(f'no response to {fields}')


SOLID = -32512


def sector_corners(surface, height, tilt):
    """Corner heights [x0z0, x1z0, x0z1, x1z1] of a sector's floor or ceiling (engine::editor::corners)."""
    tx, tz = tilt[0] * 256, tilt[1] * 256
    out = []
    for i in range(4):
        fx, fz = i & 1, i >> 1
        if surface == 'floor':
            d = (tx * (1 - fx) if tx > 0 else -tx * fx) + (tz * (1 - fz) if tz > 0 else -tz * fz)
        else:
            d = -(tx * fx if tx > 0 else -tx * (1 - fx)) - (tz * (1 - fz) if tz > 0 else -tz * fz)
        out.append(height + d)
    return out


def sector_faces(level, index):
    """Each quad covering one sector: (quad, x, z, 'floor'|'ceiling', corners)."""
    room = level['rooms'][index]
    vs = room['vertices']
    for i, quad in enumerate(room['quads']):
        ps = [vs[v] for v in quad[:4]]
        x0, z0 = min(p[0] for p in ps), min(p[2] for p in ps)
        if x0 % 1024 or z0 % 1024 or x0 < 0 or z0 < 0:
            continue
        corners, seen = [0] * 4, set()
        for p in ps:
            dx, dz = p[0] - x0, p[2] - z0
            if dx not in (0, 1024) or dz not in (0, 1024):
                break
            k = (dx > 0) + 2 * (dz > 0)
            seen.add(k)
            corners[k] = p[1]
        else:
            n = sum((ps[k][2] - ps[(k + 1) % 4][2]) * (ps[k][0] + ps[(k + 1) % 4][0]) for k in range(4))
            x, z = x0 // 1024, z0 // 1024
            if len(seen) == 4 and n and x < room['x_sectors'] and z < room['z_sectors']:
                yield i, x, z, 'floor' if n > 0 else 'ceiling', corners


def check_sectors(level, index):
    """Every floor and ceiling face of a room is where its sector's collision is."""
    room = level['rooms'][index]
    for _, x, z, surface, corners in sector_faces(level, index):
        sector = room['sectors'][x * room['z_sectors'] + z]
        info = level['floor_infos'][sector['floor_info']] if sector.get('floor_info') is not None else {}
        height = sector[surface]
        tilt = info.get('floor_slope' if surface == 'floor' else 'ceiling_slope', [0, 0])
        assert height != SOLID and sector_corners(surface, height, tilt) == corners, (index, x, z, surface)


def wall_candidates(level, index):
    """Wall quads one sector wide, seen from an open sector, with a solid sector behind."""
    room = level['rooms'][index]
    vs, depth = room['vertices'], room['z_sectors']
    for i, quad in enumerate(room['quads']):
        ps = [vs[v] for v in quad[:4]]
        for axis, other in ((0, 2), (2, 0)):
            if len({p[axis] for p in ps}) != 1 or ps[0][axis] % 1024:
                continue
            low, high = min(p[other] for p in ps), max(p[other] for p in ps)
            if low % 1024 or high - low != 1024:
                continue
            line, along = ps[0][axis] // 1024, low // 1024
            cells = [(line - 1, along), (line, along)] if axis == 0 else [(along, line - 1), (along, line)]
            ok = all(0 <= c[0] < room['x_sectors'] and 0 <= c[1] < depth for c in cells)
            if ok:
                floors = [room['sectors'][c[0] * depth + c[1]]['floor'] for c in cells]
                if (floors[0] == SOLID) != (floors[1] == SOLID):
                    yield i


def mesh_smoke(client, copy, level, captures):
    """Mesh mode on a real document: select, raise, paint, turn a texture,
    choose a tool, push a wall, undo, save."""
    original = copy.read_bytes()
    state = command(client, select='none', mode='mesh', mesh_room=0)
    assert state['mode'] == 'mesh', state
    rooms = range(len(level['rooms']))
    raised = None
    for index in rooms:
        for quad, x, z, surface, _ in sector_faces(level, index):
            if surface != 'floor':
                continue
            state = command(client, mesh_room=index, mesh_select=f'q{quad}', frame=True)
            assert state['mesh_selection'] == f'q{quad}' and state['mesh_room'] == index, state
            state = settled(client, state, mesh_extrude=-256)
            if state['message'].startswith('Raised'):
                raised = (index, quad, x, z)
                break
        if raised:
            break
    assert raised, 'no floor face could be raised'
    index, quad, x, z = raised
    assert state['dirty'], state
    time.sleep(2)
    captures.append(client.capture('mesh-raised'))
    state = settled(client, state, mesh_move='0,100,0')
    assert '256' in state['message'], state
    state = settled(client, state, mesh_paint=1)
    assert state['message'].startswith('Painted'), state
    state = settled(client, state, mesh_rotate=True)
    assert state['message'].startswith('Turned'), state
    # The toolbar's Paint tool with the brush that filled the face.
    state = command(client, tool='paint')
    assert state['tool'] == 'paint' and state['brush'] == 1, state
    client.edit('Editor', 'EditorCommand', {'show_textures': True})
    time.sleep(2)
    captures.append(client.capture('mesh-textures'))
    pushed = False
    for wall in list(wall_candidates(level, index))[:40]:
        state = command(client, mesh_select=f'q{wall}')
        state = settled(client, state, mesh_push=True)
        if state['message'].startswith('Pushed'):
            pushed = True
            break
    if pushed:
        time.sleep(2)
        captures.append(client.capture('mesh-pushed'))
    # Undo everything: the file is untouched again.
    undone = 0
    while state['dirty'] and undone < 6:
        state = settled(client, state, undo=True)
        undone += 1
    assert not state['dirty'] and undone == (4 if pushed else 3), (undone, state)
    state = command(client, save=True)
    assert copy.read_bytes() == original, 'mesh edits undone changed bytes'
    # Keep one raise, save it, and check what reached the file.
    state = settled(client, state, redo=True)
    state = command(client, save=True)
    assert not state['dirty'], state
    saved = json.loads(copy.read_bytes())
    room = saved['rooms'][index]
    sector = room['sectors'][x * room['z_sectors'] + z]
    assert sector['floor'] == level['rooms'][index]['sectors'][x * room['z_sectors'] + z]['floor'] - 256, sector
    check_sectors(saved, index)
    text = copy.read_text()
    compact = '\n' not in original.decode().strip()
    for other, before in enumerate(level['rooms']):
        if other != index and compact:
            assert json.dumps(before, separators=(',', ':')) in text, f'untouched room {other} changed bytes'
    state = command(client, mode='object')
    assert state['tool'] == 'move', 'painting does not carry over to object mode'
    return {'room': index, 'raised_face': quad, 'sector': [x, z], 'pushed_wall': pushed}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--level', type=Path, default=ROOT / 'engine/assets/levels/sandbox/level.json')
    parser.add_argument('--pose-placement', type=int)
    parser.add_argument('--pose-clip', type=int)
    parser.add_argument('--pose-steps', type=int, default=1)
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
        # Save As writes the untouched bytes to the new file and continues there.
        save_as = out / 'level-save-as.json'
        save_as.unlink(missing_ok=True)
        state = command(client, save_as=str(save_as))
        assert save_as.read_bytes() == original, 'untouched Save As changed bytes'
        assert Path(state['file']) == save_as.resolve(), state
        state = command(client, open=str(copy))
        assert Path(state['file']) == copy.resolve(), state
        state = command(client, select='0,1', frame=True)
        assert state['selection'] == [0, 1], state
        captures.append(client.capture('multi-select'))
        command(client, select='none')
        if source.parent.name == 'sandbox':
            command(client, select='1', frame=True)
            client.edit('Editor', 'AnimationPreview', {'active':True, 'isolate':True, 'placement':1, 'clip':0, 'frame':0.0, 'goal':1, 'playing':False})
            captures.append(client.capture('pose-closed'))
            from PIL import Image
            with Image.open(out / 'pose-closed.png') as image:
                rgb = image.convert('RGB').crop((500, 80, 1280, 650)).tobytes()
                assert sum(rgb[i] > 220 and 150 < rgb[i + 1] < 220 and rgb[i + 2] < 70 for i in range(0, len(rgb), 3)) > 20, 'selection bounding box is missing'
            for _ in range(9):
                client.edit('Editor', 'AnimationPreview', {'step':True})
                time.sleep(0.05)
            preview = next(e for e in client.query(name='Editor',components=['AnimationPreview'])['entities'] if e['name']=='Editor')['components']['AnimationPreview']
            assert (preview['clip'],preview['frame']) == (2,0.0), preview
            assert not status(client)['dirty'], 'preview changed persistent level data'
            captures.append(client.capture('pose-open'))
            from PIL import Image, ImageChops
            with Image.open(out/'pose-closed.png') as a, Image.open(out/'pose-open.png') as b:
                assert ImageChops.difference(a.convert('RGB').crop((500,80,1280,650)), b.convert('RGB').crop((500,80,1280,650))).getbbox(), 'authored pose did not change rendered geometry'
            client.edit('Editor','AnimationPreview',{'active':False,'isolate':False})
        before = json.loads(original)
        if args.pose_placement is not None:
            assert args.pose_clip is not None
            command(client,select=str(args.pose_placement),frame=True)
            client.edit('Editor','AnimationPreview',{'active':True,'isolate':True,'placement':args.pose_placement,'clip':args.pose_clip,'frame':0.0,'goal':before['animations'][args.pose_clip]['state'],'playing':False})
            captures.append(client.capture('model-start'))
            for _ in range(args.pose_steps):
                client.edit('Editor','AnimationPreview',{'step':True})
                time.sleep(0.03)
            captures.append(client.capture('model-end'))
            assert not status(client)['dirty'], 'model playback changed the document'
            client.edit('Editor','AnimationPreview',{'active':False,'isolate':False})
        x = before['placements'][1]['position'][0]
        state = command(client, pointer='/placements/1/position/0', json=str(x + 512), apply=True)
        assert state['dirty'] and state['message'] == 'Applied edit', state
        captures.append(client.capture('object-properties'))
        state = command(client, open=str(copy))
        assert state['asking_to_save'], state
        captures.append(client.capture('unsaved-changes'))
        state = command(client, cancel=True)
        assert state['dirty'], 'canceling open lost the edited document'
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
        state = command(client, select='1', frame=True)
        captures.append(client.capture('edited'))
        mesh = mesh_smoke(client, copy, saved, captures)
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
    report = {'level': str(source.relative_to(ROOT)), 'captures': captures, 'save_roundtrip': True, 'mesh': mesh}
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
