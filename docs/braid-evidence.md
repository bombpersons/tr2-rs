# Braid and screenshot alignment: PSX-only investigation

## Display latency (the "30 px offset")

The original's `--snapshot-every` screens lag its sampled game state by about
four fields (two logic ticks): in `.psx/climb_visual` Lara is still running
in screens 79 and 81 although RAM shows the grab at field 79; the climb pose
first appears in screen 83. Comparing original screen N+4 with the port's
tick at field N, Lara's position, pose and orientation line up (field 61
position within a few pixels). The ~30 px offset was this latency, not a
port error. `tools/climb_smoke.py` captures the port at field N; compare it
with original screen N+4.

## Braid

- Draw `0x800305ac`: six iterations over a 20-byte array at `0x8008c5c8`
  (x, y, z int; x rotation +12, y rotation +14): translate absolute, RotY,
  RotX, draw braid mesh j (object 2, 6 meshes, joint offsets 50/54/54/48/36).
  Found by pairing mesh-draw `0x80062f1c` calls with the braid mesh addresses.
- InitialiseHair `0x8002f594`: every segment pitch -16384 (hanging), first
  flag set; velocities at `0x8008c660` (12-byte entries) cleared.
- HairControl `0x8002f67c` (per tick):
  - spheres from Lara's bone chain: hips (mesh 0), torso (7), head (14),
    right upper arm (8, radius*3/2) and left upper arm (joint 11 but mesh 8's
    centre and radius, as the code loads mesh pointer 8 twice);
  - root = head matrix * (0, -23, -55); on the first tick the chain is laid
    out from the root with the stored angles;
  - floor = height under the root (Lara's floor if the root is below it);
    rooms flagged 0x20 add wind along +z, a random walk clamped to 0..7;
  - per point 1..6: pos += 3*vel/4; dry: y += 10, clamp to floor else add
    wind; water: clamp to surface/floor; push out of each sphere to its
    radius; yaw = atan(dz, dx), pitch = -atan(horizontal, dy) from the
    previous point; rebuild at the joint length (point 6 reuses joint 5);
    vel = new - old.

The port (`presentation/braid.rs`) follows these rules in floating point on
the game tick; its RNG and trig are not the original's. Water handling and
Lara's head/torso look rotations (zero here) are not ported. Rendered frames
show the braid hanging and swinging like the original's. The grey object over
Lara's right shoulder in the original (a mesh-swap model, likely a holstered
weapon) is not yet drawn by the port.
