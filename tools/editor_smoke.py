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


def click(client, at, before):
    """Click and wait for the selection to change (or a few seconds, when the
    click should change nothing): large levels draw slowly in software."""
    client.edit('Editor', 'EditorCommand', {'click': at})
    deadline = time.monotonic() + 4
    while time.monotonic() < deadline:
        state = status(client)
        if state['selection'] != before:
            return state
        time.sleep(0.1)
    return state


def click_smoke(client, level, preferred):
    """Click a framed object at the centre of the 3D view through the same
    systems as the mouse; with the Select tool, Shift-click toggles it off."""
    candidates = [i for i in [preferred, 1, 0] + list(range(len(level['placements']))) if i is not None]
    for index in dict.fromkeys(candidates[:12]):
        command(client, select=str(index), frame=True)
        time.sleep(0.5)
        command(client, select='none')
        state = click(client, '640,400', [])
        if state['selection'] == [index]:
            # The Move tool's arrows now cover the centre; Select has no handles.
            command(client, tool='select')
            state = click(client, '640,400,shift', [index])
            assert state['selection'] == [], state
            command(client, tool='move')
            return index
    raise AssertionError('clicking a framed object never selected it')


def diagnostics_smoke(client, level, captures):
    """Delete a room's portal back to a neighbour: Diagnostics reports a new
    one-sided portal (the file's own quirks stay inherited), and Fix all new
    restores a portal back."""
    state = command(client, show_diagnostics=True)
    # Earlier steps may have left new problems (the sandbox object moved
    # into the solid border is one); count from here.
    before = state['problems_new']
    pair = next(((i, k) for i, r in enumerate(level['rooms']) for k, p in enumerate(r['portals'])
                 if any(q['room'] == i for q in level['rooms'][p['room']]['portals'])), None)
    if pair is None:
        time.sleep(1)
        captures.append(client.capture('diagnostics'))
        return {'new': before, 'total': state['problems'], 'fixed': None}
    room, portal = pair
    target = level['rooms'][room]['portals'][portal]['room']
    kept = [q for q in level['rooms'][target]['portals'] if q['room'] != room]
    state = settled(client, state, pointer=f'/rooms/{target}/portals', json=json.dumps(kept), apply=True)
    deadline = time.monotonic() + 20
    while status(client)['problems_new'] == before and time.monotonic() < deadline:
        time.sleep(0.2)
    state = status(client)
    assert state['problems_new'] > before, state
    time.sleep(1)
    captures.append(client.capture('diagnostics'))
    state = settled(client, state, fix_new=True)
    assert state['message'].startswith('Fixed'), state
    deadline = time.monotonic() + 20
    while status(client)['problems_new'] != before and time.monotonic() < deadline:
        time.sleep(0.2)
    # The doorway sectors' links would stay new if the fix were missing.
    state = status(client)
    assert state['problems_new'] == before, state
    state = settled(client, state, undo=True)
    state = settled(client, state, undo=True)
    assert not state['dirty'], state
    return {'new': before, 'total': state['problems'], 'fixed': {'room': room, 'portal': portal, 'into': target}}


def single_doorway(level):
    """Two rooms joined by exactly one doorway sector each way and nothing else."""
    infos = level['floor_infos']
    def links(a, b):
        r = level['rooms'][a]
        side = [s for s in r['sectors'] if s.get('floor_info') is not None and infos[s['floor_info']].get('portal') == b]
        vertical = [s for s in r['sectors'] if b in (s.get('below'), s.get('above'))]
        return len(side), len(vertical)
    for a, room in enumerate(level['rooms']):
        for b in sorted({p['room'] for p in room['portals']}):
            if links(a, b) == (1, 0) and links(b, a) == (1, 0):
                return a, b
    return None


def connect_smoke(client, level, captures):
    """Disconnect two rooms, then reopen the doorway by selecting the wall
    that closed it and pressing Connect; the result has no new problems."""
    pair = single_doorway(level)
    if pair is None:
        return None
    a, b = pair
    state = command(client, mode='object')
    before = state['problems_new']
    state = settled(client, state, disconnect=f'{a},{b}')
    assert state['message'].startswith('Disconnected'), state
    # The closing wall is the newest face of room a (one face out, one in).
    wall = len(level['rooms'][a]['quads']) - 1
    state = command(client, mode='mesh', mesh_room=a)
    state = command(client, mesh_select=f'q{wall}', frame=True)
    state = settled(client, state, mesh_connect=True)
    assert state['message'].startswith(f'Opened a doorway into room {b}'), state
    deadline = time.monotonic() + 20
    while status(client)['problems_new'] != before and time.monotonic() < deadline:
        time.sleep(0.2)
    state = status(client)
    assert state['problems_new'] == before, state
    time.sleep(1)
    captures.append(client.capture('connected'))
    state = settled(client, state, undo=True)
    state = settled(client, state, undo=True)
    assert not state['dirty'], state
    command(client, mode='object')
    return {'rooms': [a, b]}


def hole_pair(level):
    """A room whose floor opens into a room below, with no doorway between them."""
    infos = level['floor_infos']
    for a, room in enumerate(level['rooms']):
        for b in sorted({s['below'] for s in room['sectors'] if s.get('below') is not None}):
            other = level['rooms'][b]
            side = any(s.get('floor_info') is not None and infos[s['floor_info']].get('portal') in (a, b)
                       for r, s in [(a, s) for s in other['sectors']] + [(b, s) for s in room['sectors']])
            upward = any(s.get('above') == b for s in room['sectors']) or any(s.get('below') == a for s in other['sectors'])
            if not side and not upward:
                return a, b, sum(1 for s in room['sectors'] if s.get('below') == b)
    return None


def hole_smoke(client, level, captures):
    """Close a hole by disconnecting, then reopen it by selecting the floors
    that closed it and pressing Open hole."""
    found = hole_pair(level)
    if found is None:
        return None
    a, b, count = found
    state = command(client, mode='object')
    before = state['problems_new']
    state = settled(client, state, disconnect=f'{a},{b}')
    assert state['message'].startswith('Disconnected'), state
    n = len(level['rooms'][a]['quads'])
    floors = ','.join(f'q{i}' for i in range(n, n + count))
    state = command(client, mode='mesh', mesh_room=a)
    state = command(client, mesh_select=floors)
    state = settled(client, state, mesh_hole=True)
    assert state['message'].startswith(f'Opened the floor into room {b}'), state
    deadline = time.monotonic() + 20
    while status(client)['problems_new'] != before and time.monotonic() < deadline:
        time.sleep(0.2)
    state = status(client)
    assert state['problems_new'] == before, state
    state = settled(client, state, undo=True)
    state = settled(client, state, undo=True)
    assert not state['dirty'], state
    command(client, mode='object')
    return {'rooms': [a, b], 'cells': count}


def new_room_smoke(client, level, captures):
    """Draw a 2 x 2 room on the map beside the sandbox room's open floor,
    create it, and connect it through its first wall (cutting the sandbox's
    room-wide wall down to one sector). The only new problems are the new
    room's missing navigation boxes, until navigation is rebuilt."""
    rooms = len(level['rooms'])
    state = command(client, mode='object', room=0)
    before = state['problems_new']
    state = command(client, draft_room='5,3,7,5,0,-2048')
    time.sleep(1)
    captures.append(client.capture('new-room-draft'))
    state = settled(client, state, create_room=True)
    assert state['rooms'] == rooms + 1 and state['message'].startswith(f'Created room {rooms}'), state
    # Its first wall faces in from the -x border, beside the sandbox's floor.
    state = command(client, mode='mesh', mesh_room=rooms)
    state = command(client, mesh_select='q2')
    state = settled(client, state, mesh_connect=True)
    assert state['message'].startswith('Opened a doorway into room 0'), state
    deadline = time.monotonic() + 20
    while status(client)['problems_new'] != before + 4 and time.monotonic() < deadline:
        time.sleep(0.2)
    state = status(client)
    assert state['problems_new'] == before + 4, state
    # Rebuilding navigation gives the new room (and the sandbox, which has
    # none of its own) boxes, leaving only the earlier steps' problems.
    state = settled(client, state, rebuild_nav=True)
    assert state['message'].startswith('Rebuilt navigation'), state
    deadline = time.monotonic() + 20
    while status(client)['problems_new'] != before and time.monotonic() < deadline:
        time.sleep(0.2)
    state = status(client)
    assert state['problems_new'] == before, state
    state = command(client, frame_room=True)
    time.sleep(1.5)
    captures.append(client.capture('new-room-connected'))
    for _ in range(3):
        state = settled(client, state, undo=True)
    assert state['rooms'] == rooms, state
    command(client, mode='object', room=0)
    return {'room': rooms, 'connected_to': 0}


def split_smoke(client, level, captures):
    """Split room 0 across x and merge it back, then at a height and merge
    it back: no new problems, and undo returns the file."""
    rooms = len(level['rooms'])
    r = level['rooms'][0]
    if r['x_sectors'] < 4:
        return None
    state = command(client, mode='object', room=0, show_rooms=True)
    before = state['problems_new']
    line = r['x_sectors'] // 2
    state = settled(client, state, split_room=f'x,{line}')
    assert state['rooms'] == rooms + 1, state
    deadline = time.monotonic() + 20
    while status(client)['problems_new'] != before and time.monotonic() < deadline:
        time.sleep(0.2)
    state = status(client)
    assert state['problems_new'] == before, state
    state = command(client, frame_room=True)
    time.sleep(1.5)
    captures.append(client.capture('split'))
    state = settled(client, state, merge_room=rooms)
    assert state['rooms'] == rooms and state['message'].startswith('Merged'), state
    steps = 2
    middle = ((r['top'] + r['bottom']) // 2) // 256 * 256
    state = settled(client, state, split_room=f'y,{middle}')
    if state['rooms'] == rooms + 1:
        state = settled(client, state, merge_room=rooms)
        assert state['rooms'] == rooms, state
        steps += 2
    for _ in range(steps):
        state = settled(client, state, undo=True)
    assert not state['dirty'], state
    return {'room': 0, 'line': line, 'height': middle, 'steps': steps}


def floor_faces_under_holes(level, room, above):
    """Quads of `room` that are the floor of one sector whose ceiling opens into `above`."""
    r = level['rooms'][room]
    found = []
    for i, q in enumerate(r['quads']):
        p = [r['vertices'][v][:3] for v in q[:4]]
        xs, zs = {v[0] for v in p}, {v[2] for v in p}
        if len(xs) != 2 or len(zs) != 2 or max(xs) - min(xs) != 1024 or max(zs) - min(zs) != 1024:
            continue
        if min(xs) % 1024 or min(zs) % 1024:
            continue
        # Newell's method about y: positive faces up (a floor).
        n = sum((p[k][2] - p[(k + 1) % 4][2]) * (p[k][0] + p[(k + 1) % 4][0]) for k in range(4))
        x, z = min(xs) // 1024, min(zs) // 1024
        if n > 0 and r['sectors'][x * r['z_sectors'] + z].get('above') == above:
            found.append(i)
    return found


def close_hole_smoke(client, level, captures):
    """Close part of a hole from below, by selecting the floor seen through
    it: only the new floor's navigation is missing, which a rebuild fixes."""
    pairs = sorted({(a, s['below']) for a, r in enumerate(level['rooms']) for s in r['sectors'] if s.get('below') is not None})
    found = next(((u, l, f[:4]) for u, l in pairs if (f := floor_faces_under_holes(level, l, u))), None)
    if found is None:
        return None
    upper, lower, faces = found
    state = command(client, mode='object')
    before = state['problems_new']
    state = command(client, mode='mesh', mesh_room=lower)
    state = command(client, mesh_select=','.join(f'q{i}' for i in faces), frame=True)
    state = settled(client, state, mesh_close_hole=True)
    assert state['message'].startswith(f'Closed {len(faces)} sectors of the hole to room {upper}'), state
    time.sleep(1.5)
    captures.append(client.capture('hole-closed'))
    state = settled(client, state, rebuild_nav=True)
    deadline = time.monotonic() + 20
    while status(client)['problems_new'] != before and time.monotonic() < deadline:
        time.sleep(0.2)
    state = status(client)
    assert state['problems_new'] == before, state
    state = settled(client, state, undo=True)
    state = settled(client, state, undo=True)
    assert not state['dirty'], state
    command(client, mode='object')
    return {'rooms': [upper, lower], 'closed': len(faces)}


def doorway(level):
    """A placement and the offset that carries it through one of its room's
    doorways: the centre of a border sector whose floor data leads on."""
    infos = level['floor_infos']
    for index, p in enumerate(level['placements']):
        room = level['rooms'][p['room']]
        for x in range(room['x_sectors']):
            for z in range(room['z_sectors']):
                s = room['sectors'][x * room['z_sectors'] + z]
                info = s.get('floor_info')
                target = infos[info].get('portal') if info is not None else None
                if target is None or target == p['room'] or s['floor'] == SOLID:
                    continue
                to = [room['origin'][0] + x * 1024 + 512, s['floor'], room['origin'][2] + z * 1024 + 512]
                return index, target, [to[i] - p['position'][i] for i in range(3)]
    return None


def rooms_smoke(client, level, captures):
    """The Rooms window and map, and an object carried through a doorway."""
    state = command(client, show_rooms=True, room=len(level['rooms']) - 1, select='none', frame_room=True)
    assert state['mesh_room'] == len(level['rooms']) - 1, state
    time.sleep(1)
    captures.append(client.capture('rooms-map'))
    found = doorway(level) if len(level['rooms']) > 1 else None
    result = {'rooms_window': True, 'doorway': None}
    if found:
        index, target, offset = found
        state = command(client, select=str(index))
        state = settled(client, state, move_selection=','.join(map(str, offset)))
        assert f'into room {target}' in state['message'], state
        state = settled(client, state, undo=True)
        assert not state['dirty'], state
        result['doorway'] = {'placement': index, 'into': target}
    command(client, select='none')
    return result


def flip_smoke(client, level, captures):
    """Choose an alternate room: the view flips to show it in its room's
    place, and turning Flipped off brings the original back."""
    pair = next(((b, r['alternate']) for b, r in enumerate(level['rooms']) if r.get('alternate') is not None), None)
    if pair is None:
        return None
    base, alternate = pair
    state = command(client, mode='object', select='none')
    problems = state['problems_new']
    state = command(client, show_rooms=True, room=alternate, frame_room=True)
    assert state['flipped'] and state['mesh_room'] == alternate, state
    time.sleep(1.5)
    captures.append(client.capture('flipped'))
    state = command(client, flip='off')
    assert not state['flipped'] and state['mesh_room'] == base, state
    time.sleep(1.5)
    captures.append(client.capture('unflipped'))
    assert not state['dirty'] and state['problems_new'] == problems, state
    return {'room': base, 'alternate': alternate}


def play_smoke(client, level, out):
    """Play from view, writing only the play copy: Lara's placement moves to
    the floor at the centre of the view and nothing else changes."""
    folder = out / 'play-assets'
    shutil.rmtree(folder, ignore_errors=True)
    before = status(client)
    state = settled(client, before, play=f'export:{folder}')
    if not any(m['id'] == 0 for m in level['models']):
        assert 'no Lara model' in state['message'], state
        return None
    assert state['message'].startswith('Wrote levels/'), state
    name = state['message'].split('/')[1]
    written = json.loads((folder / 'levels' / name / 'editor-play.json').read_bytes())
    lara = next(i for i, p in enumerate(level['placements']) if p['kind'] == 0)
    spot = written['placements'][lara]
    room = written['rooms'][spot['room']]
    x, z = (spot['position'][0] - room['origin'][0]) // 1024, (spot['position'][2] - room['origin'][2]) // 1024
    assert room['sectors'][x * room['z_sectors'] + z]['floor'] != SOLID, spot
    written['placements'][lara] = level['placements'][lara]
    assert written == level, 'play copy changed more than Lara'
    assert not status(client)['dirty']
    return {'level': name, 'lara': spot}


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
        clicked = click_smoke(client, json.loads(original), args.pose_placement)
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
        rooms = rooms_smoke(client, saved, captures)
        flip = flip_smoke(client, saved, captures)
        played = play_smoke(client, saved, out)
        diagnostics = diagnostics_smoke(client, saved, captures)
        connected = connect_smoke(client, saved, captures)
        holes = hole_smoke(client, saved, captures)
        built = new_room_smoke(client, saved, captures) if source.parent.name == 'sandbox' else None
        split = split_smoke(client, saved, captures)
        closed = close_hole_smoke(client, saved, captures)
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
    report = {'level': str(source.relative_to(ROOT)), 'captures': captures, 'save_roundtrip': True, 'clicked': clicked, 'rooms': rooms, 'flip': flip, 'play': played, 'diagnostics': diagnostics, 'connected': connected, 'holes': holes, 'new_room': built, 'split': split, 'close_hole': closed, 'mesh': mesh}
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
