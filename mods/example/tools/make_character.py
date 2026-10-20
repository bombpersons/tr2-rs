#!/usr/bin/env python3
"""Build the example mod's hand-made character into its level.

Writes model 0 (the game's player type), its meshes and its animation table
into `assets/levels/example_mod/level.json`, and the atlas the character is
coloured from. Every number here is made up for this mod: nothing comes from
the disc. The clip, state and joint numbers follow Lara's asset contract
(`docs/lara-contract.md`), the ground-locomotion subset:

- joints in Lara's order (hips, two legs, torso, two arms, head), so the
  arms, head and gun presentation land on the right bones;
- clips at the absolute numbers her controller asks for (stand 11 and 103,
  run 0/6/8, walk 1/2/3/20/21, turns 12/13/44/69, wall hits 53/54);
- every other clip number up to the contract's last one filled with a
  one-frame standing clip (state 2, ending in clip 11), so a request for an
  unsupported move shows the standing pose rather than indexing past the
  table. `game::player::contract::check` reports those as not provided.

The pickup model (type 135) keeps its own clips, moved after the
character's table.

    python3 -I mods/example/tools/make_character.py
"""
import json
import math
import re
import sys
from pathlib import Path

from PIL import Image

LEVEL_DIR = Path(__file__).resolve().parent.parent / "assets" / "levels" / "example_mod"
LEVEL = LEVEL_DIR / "level.json"
ATLAS = LEVEL_DIR / "atlas.png"

PLAYER = 0
PICKUP = 135
# One past the highest clip number the contract names
# (`game::player::contract::CLIP_COUNT`).
CLIP_COUNT = 194

# Contract states (`game::player::contract::state`).
WALK, RUN, STAND, TURN_RIGHT, TURN_LEFT, FAST_TURN = 0, 1, 2, 6, 7, 20

# Atlas: four-texel strips, one colour each. 0 and 1 are the room's.
COLOURS = [(255, 255, 255), (200, 150, 100), (230, 180, 140), (40, 130, 150), (70, 55, 80)]
FLOOR, WALL, SKIN, SHIRT, TROUSERS = range(len(COLOURS))
STRIP = 4


def write_atlas():
    image = Image.new("RGBA", (STRIP * len(COLOURS), 8))
    for i, colour in enumerate(COLOURS):
        for x in range(STRIP):
            for y in range(8):
                image.putpixel((i * STRIP + x, y), colour + (255,))
    image.save(ATLAS)


def textures():
    width = STRIP * len(COLOURS)
    out = []
    for i in range(len(COLOURS)):
        u0, u1 = (i * STRIP + 0.25) / width, (i * STRIP + STRIP - 0.25) / width
        out.append({"uv": [[u0, 0.06], [u1, 0.06], [u1, 0.94], [u0, 0.94]], "attribute": 0})
    return out


def box(x0, x1, y0, y1, z0, z1, texture, shade=0):
    """A mesh: one box, y down, around its joint."""
    v = [
        [x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1],
        [x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1],
    ]
    faces = [(0, 1, 2, 3), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    # Unlit meshes carry a shade per vertex (0 bright .. 8191 dark): darker
    # underneath, so the boxes read as solid.
    vertices = [p + [shade + (1200 if p[1] == y1 else 0)] for p in v]
    centre = [(x0 + x1) // 2, (y0 + y1) // 2, (z0 + z1) // 2]
    radius = int(math.dist(centre, v[6])) + 1
    return {
        "center": centre,
        "radius": radius,
        "vertices": vertices,
        "normals": [],
        "quads": [list(f) + [texture] for f in faces],
        "triangles": [],
    }


# The skeleton, in Lara's joint order (`contract::joint`): parent, offset
# from it (y down), and the bone's box.
LEG_X, ARM_X = 55, 105
SKELETON = [
    # 0 hips
    (None, [0, 0, 0], box(-80, 80, -50, 40, -45, 45, TROUSERS)),
    # 1-3 first leg: thigh, shin, foot
    (0, [-LEG_X, 30, 0], box(-32, 32, 0, 220, -32, 32, TROUSERS)),
    (1, [0, 220, 0], box(-27, 27, 0, 200, -27, 27, TROUSERS)),
    (2, [0, 200, 0], box(-28, 28, 0, 40, -30, 80, SKIN, 1500)),
    # 4-6 second leg
    (0, [LEG_X, 30, 0], box(-32, 32, 0, 220, -32, 32, TROUSERS)),
    (4, [0, 220, 0], box(-27, 27, 0, 200, -27, 27, TROUSERS)),
    (5, [0, 200, 0], box(-28, 28, 0, 40, -30, 80, SKIN, 1500)),
    # 7 torso
    (0, [0, -50, 0], box(-85, 85, -230, 0, -50, 50, SHIRT)),
    # 8-10 first arm: upper arm, forearm, hand
    (7, [ARM_X, -210, 0], box(-22, 22, 0, 160, -22, 22, SHIRT)),
    (8, [0, 160, 0], box(-20, 20, 0, 140, -20, 20, SKIN)),
    (9, [0, 140, 0], box(-18, 18, 0, 60, -22, 22, SKIN, 800)),
    # 11-13 second arm
    (7, [-ARM_X, -210, 0], box(-22, 22, 0, 160, -22, 22, SHIRT)),
    (11, [0, 160, 0], box(-20, 20, 0, 140, -20, 20, SKIN)),
    (12, [0, 140, 0], box(-18, 18, 0, 60, -22, 22, SKIN, 800)),
    # 14 head
    (7, [0, -235, 0], box(-60, 60, -140, 0, -60, 60, SKIN)),
]
JOINTS = len(SKELETON)
# Root height above the feet with straight legs: leg offsets plus foot.
HIP_HEIGHT = 30 + 220 + 200 + 40
BOUNDS = [-110, 110, -800, 0, -110, 110]


def angle(degrees):
    return round(degrees * 65536 / 360) % 65536


def pose(bob=0, lean=0, legs=(0, 0), knees=(0, 0), arms=(0, 0), elbows=(0, 0), turn=0, head=0):
    """A key: hips height and per-joint `[x, y, z]` turns, in degrees.

    `legs`/`arms` swing the first and second limb about x; `knees`/`elbows`
    bend the lower part; `turn` twists the hips about y.
    """
    a = [[0, 0, 0] for _ in range(JOINTS)]
    a[0] = [angle(lean), angle(turn), 0]
    a[1], a[4] = [angle(legs[0]), 0, 0], [angle(legs[1]), 0, 0]
    a[2], a[5] = [angle(-knees[0]), 0, 0], [angle(-knees[1]), 0, 0]
    a[3], a[6] = [angle(knees[0] / 2 - legs[0] / 2), 0, 0], [angle(knees[1] / 2 - legs[1] / 2), 0, 0]
    a[7] = [angle(-lean / 2), 0, 0]
    a[8], a[11] = [angle(arms[0]), 0, angle(-4)], [angle(arms[1]), 0, angle(4)]
    a[9], a[12] = [angle(elbows[0]), 0, 0], [angle(elbows[1]), 0, 0]
    a[14] = [angle(head), 0, 0]
    return {"root": [0, -HIP_HEIGHT + bob, 0], "angles": a}


STAND_POSE = pose(arms=(3, 3), elbows=(6, 6))


def stride(phase, swing, knee, arm, bob, lean):
    """A gait key at `phase` (0..1 of a two-step cycle)."""
    s = math.sin(phase * math.tau)
    c = math.cos(phase * math.tau)
    return pose(
        bob=round(bob * abs(c)),
        lean=lean,
        legs=(swing * s, -swing * s),
        knees=(knee * max(0.0, -c) + 8, knee * max(0.0, c) + 8),
        arms=(-arm * s, arm * s),
        elbows=(20 + arm / 2, 20 + arm / 2),
    )


def blend(a, b, t):
    """A key between two keys (angles taken the short way round)."""

    def mix(x, y):
        d = ((y - x + 32768) % 65536) - 32768
        return round(x + d * t) % 65536

    return {
        "root": [round(p + (q - p) * t) for p, q in zip(a["root"], b["root"])],
        "angles": [[mix(x, y) for x, y in zip(ja, jb)] for ja, jb in zip(a["angles"], b["angles"])],
    }


def units_per_frame(value):
    """A clip speed: 16.16 world units per frame."""
    return round(value * 65536)


class Table:
    """Clips at fixed numbers; frame numbers are allocated as clips are added."""

    def __init__(self):
        self.clips = {}

    def add(self, number, *, state, keys, rate, frames, next_clip, next_frame=0, speed=0.0, accel=0.0, ranges=()):
        assert number not in self.clips, number
        assert len(keys) in (math.ceil(frames / rate), math.ceil(frames / rate) + 1), (number, len(keys))
        self.clips[number] = dict(
            state=state, keys=keys, rate=rate, frames=frames, next=(next_clip, next_frame),
            speed=speed, accel=accel, ranges=list(ranges),
        )

    def build(self, count):
        """The animation list: `count` clips, fillers where none was added."""
        for number in range(count):
            if number not in self.clips:
                self.add(number, state=STAND, keys=[STAND_POSE], rate=1, frames=1, next_clip=11)
        starts, frame = {}, 0
        for number in range(count):
            starts[number] = frame
            frame += self.clips[number]["frames"]
        out = []
        for number in range(count):
            c = self.clips[number]
            start = starts[number]
            next_clip, next_frame = c["next"]
            out.append({
                "frame_rate": c["rate"],
                "frame_start": start,
                "frame_end": start + c["frames"] - 1,
                "next_animation": next_clip,
                "next_frame": starts[next_clip] + next_frame,
                "state": c["state"],
                "speed": units_per_frame(c["speed"]),
                "accel": units_per_frame(c["accel"]),
                "poses": c["keys"],
                "bounds": [BOUNDS] * len(c["keys"]),
                # A range is (goal state, first frame, last frame, clip,
                # frame), clip-relative here and absolute in the file.
                "state_changes": [
                    {
                        "state": goal,
                        "first_frame": start + first,
                        "last_frame": start + last,
                        "animation": clip,
                        "frame": starts[clip] + at,
                    }
                    for goal, first, last, clip, at in c["ranges"]
                ],
                "commands": [],
            })
        assert starts[1] == 22, "the walk loop must start at absolute frame 22"
        return out, frame


def character_clips():
    t = Table()

    def cycle(n, rate, **gait):
        keys = [stride(i / n, **gait) for i in range(n)]
        return keys + [keys[0]], n * rate

    def ease(a, b, n):
        return [blend(a, b, i / (n - 1)) for i in range(n)]

    # Everything that leaves a standing clip: walk, run and the two turns.
    def from_stand(frames):
        last = frames
        return [(WALK, 0, last, 20, 0), (RUN, 0, last, 6, 0), (TURN_RIGHT, 0, last, 12, 0), (TURN_LEFT, 0, last, 13, 0)]

    # Standing: 11 holds one frame and continues into 103, the breathing loop.
    t.add(11, state=STAND, keys=[STAND_POSE], rate=1, frames=1, next_clip=103, ranges=from_stand(1))
    breathe = [STAND_POSE, pose(bob=6, arms=(4, 4), elbows=(8, 8), head=-2), STAND_POSE]
    t.add(103, state=STAND, keys=breathe, rate=30, frames=60, next_clip=103, ranges=from_stand(60))

    # Running: start (6) into the loop (0); a stop request anywhere in the
    # loop or start enters the stop (8), which ends standing.
    # The run loop is 22 frames long, so the walk loop after it starts at
    # absolute frame 22: walk collision reads its foot windows there
    # (`contract::frame::WALK_LOOP`).
    run = dict(swing=38, knee=70, arm=40, bob=40, lean=12)
    keys, frames = cycle(11, 2, **run)
    assert frames == 22
    t.add(0, state=RUN, keys=keys, rate=2, frames=frames, next_clip=0, speed=40, ranges=[(STAND, 0, frames - 1, 8, 0)])
    start_keys = ease(STAND_POSE, keys[0], 4) + [keys[1]]
    t.add(6, state=RUN, keys=start_keys, rate=3, frames=12, next_clip=0, next_frame=2, speed=10, accel=2.5,
          ranges=[(STAND, 0, 11, 8, 0)])
    t.add(8, state=RUN, keys=ease(keys[2], STAND_POSE, 5), rate=3, frames=12, next_clip=11, speed=36, accel=-3)

    # Walking: start (20, 21) into the loop (1); stopping enters 2.
    walk = dict(swing=22, knee=35, arm=18, bob=12, lean=2)
    keys, frames = cycle(9, 4, **walk)
    assert frames == 36
    t.add(1, state=WALK, keys=keys, rate=4, frames=frames, next_clip=1, speed=14,
          ranges=[(STAND, 0, frames - 1, 2, 0)])
    t.add(20, state=WALK, keys=ease(STAND_POSE, keys[0], 3), rate=4, frames=10, next_clip=21, speed=3,
          ranges=[(STAND, 0, 9, 2, 0)])
    t.add(21, state=WALK, keys=[keys[0], keys[1], keys[2], keys[3]], rate=4, frames=16, next_clip=1, next_frame=16,
          speed=4, accel=0.6, ranges=[(STAND, 0, 15, 2, 0)])
    # The two foot stops; walking into a wall picks one by the walk loop's
    # frame (`contract::frame::WALL_STOP_*`).
    t.add(2, state=WALK, keys=ease(keys[4], STAND_POSE, 3), rate=4, frames=10, next_clip=11)
    t.add(3, state=WALK, keys=ease(keys[0], STAND_POSE, 3), rate=4, frames=10, next_clip=11)

    # Turning on the spot: little steps; the slow turns hand over to the fast
    # ones (state 20) when the controller asks.
    def shuffle(n, rate, swing):
        keys = [pose(legs=(swing * math.sin(i / n * math.tau), -swing * math.sin(i / n * math.tau)),
                     knees=(12 + 12 * max(0, math.sin(i / n * math.tau)), 12 + 12 * max(0, -math.sin(i / n * math.tau))),
                     arms=(3, 3), elbows=(8, 8)) for i in range(n)]
        return keys + [keys[0]], n * rate

    def leave_turn(frames):
        return [(WALK, 0, frames - 1, 20, 0), (RUN, 0, frames - 1, 6, 0), (STAND, 0, frames - 1, 11, 0)]

    keys, frames = shuffle(4, 4, 8)
    t.add(12, state=TURN_RIGHT, keys=keys, rate=4, frames=frames, next_clip=12,
          ranges=leave_turn(frames) + [(FAST_TURN, 0, frames - 1, 44, 0)])
    t.add(13, state=TURN_LEFT, keys=keys, rate=4, frames=frames, next_clip=13,
          ranges=leave_turn(frames) + [(FAST_TURN, 0, frames - 1, 69, 0)])
    keys, frames = shuffle(4, 3, 14)
    t.add(44, state=FAST_TURN, keys=keys, rate=3, frames=frames, next_clip=44, ranges=leave_turn(frames))
    t.add(69, state=FAST_TURN, keys=keys, rate=3, frames=frames, next_clip=69, ranges=leave_turn(frames))

    # Running into a wall: a recoil back to standing (53 early in the
    # stride, 54 late; the controller picks by the run loop's frame).
    recoil = pose(bob=-10, lean=-14, legs=(-10, 12), knees=(20, 10), arms=(-30, -30), elbows=(50, 50), head=-10)
    for number in (53, 54):
        t.add(number, state=STAND, keys=ease(recoil, STAND_POSE, 4), rate=4, frames=16, next_clip=11)
    return t


def relocate(clips, first_clip, first_frame):
    """Move a model's clips to start at `first_clip` / `first_frame`."""
    out = []
    old_first = min(c["frame_start"] for c in clips)
    for c in clips:
        c = json.loads(json.dumps(c))
        shift = first_frame - old_first
        c["frame_start"] += shift
        c["frame_end"] += shift
        c["next_frame"] += shift
        for s in c["state_changes"]:
            s["first_frame"] += shift
            s["last_frame"] += shift
            s["frame"] += shift
        out.append(c)
    return out


def dump(level):
    """Indented JSON with arrays of numbers kept on one line each."""
    text = json.dumps(level, indent=1)
    return re.sub(r"\[[-0-9.,\s]*\]", lambda m: re.sub(r"\s+", "", m.group(0)).replace(",", ", "), text)


def main():
    level = json.loads(LEVEL.read_text())
    models = {m["id"]: m for m in level["models"]}
    pickup = models[PICKUP]
    first, last = pickup["animations"]
    pickup_clips = level["animations"][first:last]
    pickup_mesh = level["meshes"][pickup["meshes"][0]]

    animations, frames = character_clips().build(CLIP_COUNT)
    moved = relocate(pickup_clips, CLIP_COUNT, frames)
    for c in moved:
        c["next_animation"] += CLIP_COUNT - first
        for s in c["state_changes"]:
            s["animation"] += CLIP_COUNT - first
    level["animations"] = animations + moved

    meshes = [bone for _, _, bone in SKELETON]
    # The pickup keeps its box and its texture slot (1).
    level["meshes"] = meshes + [pickup_mesh]
    joints = []
    for parent, offset, _ in SKELETON:
        joint = {"offset": offset}
        if parent is not None:
            joint["parent"] = parent
        joints.append(joint)
    level["models"] = [
        {"id": PLAYER, "meshes": list(range(JOINTS)), "joints": joints, "animations": [0, CLIP_COUNT]},
        {**pickup, "meshes": [JOINTS], "animations": [CLIP_COUNT, CLIP_COUNT + len(moved)]},
    ]
    for placement in level["placements"]:
        if placement["kind"] in (300, PLAYER):
            # Sector (3, 1), facing +z: two sectors short of the chime
            # trigger in sector (3, 3).
            placement.update(kind=PLAYER, position=[3584, 0, 1536], rotation=0)
    level["textures"] = textures()
    write_atlas()
    LEVEL.write_text(dump(level) + "\n")
    print(f"{LEVEL}: {len(level['animations'])} clips, {frames} character frames", file=sys.stderr)


if __name__ == "__main__":
    main()
